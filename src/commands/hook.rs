use crate::cli::{HookArgs, HookCommand, HookExecArgs, HookInstallArgs};
use crate::commands::add;
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{
    Evidence, ItemStatus, LogEvent, Severity, compute_id, format_timestamp, resolve_agent,
};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const HOOK_INPUT_LIMIT: u64 = 1024 * 1024;
/// A failed command becomes the cut's text verbatim. Long debugging one-liners are noise, not
/// friction, so the hook declines them rather than filing an entry nobody will read.
const HOOK_COMMAND_LIMIT: usize = 500;
// Read-only interrogation commands whose non-zero exit is an expected answer.
const PROBE_COMMANDS: &[&str] = &[
    "grep", "rg", "ls", "find", "tail", "head", "cat", "stat", "test", "[", "which", "curl", "gh",
];
const CLAUDE_CODE_COMMAND_SUFFIX: &str = "hook exec claude-code";
static TEMP_FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Serialize)]
pub struct HookInstallData {
    pub changed: bool,
    pub settings_path: String,
    pub command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookInstallOutcome {
    Created,
    Amended,
    Unchanged,
}

impl HookInstallOutcome {
    fn changed(self) -> bool {
        !matches!(self, Self::Unchanged)
    }

    fn action(self) -> Option<&'static str> {
        match self {
            Self::Created => Some("created"),
            Self::Amended => Some("amended"),
            Self::Unchanged => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ClaudeCodePayload {
    hook_event_name: Option<String>,
    tool_name: Option<String>,
    tool_input: Option<ClaudeCodeToolInput>,
    error: Option<String>,
    is_interrupt: Option<bool>,
    cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeCodeToolInput {
    command: Option<String>,
}

enum ClaudeCodePayloadRead {
    Payload(ClaudeCodePayload),
    TooLarge,
    NotJson,
}

enum HookExecOutcome {
    StdinUnreadable,
    StdinTooLarge,
    PayloadNotJson,
    UnexpectedEvent(Option<String>),
    NonBashTool(Option<String>),
    Interrupted,
    MissingCommand,
    CommandTooLong(usize),
    CompoundCommand,
    CommandRejected(String),
    ProbeCommand(String),
    MissingLog(PathBuf),
    DuplicateOpenCommand(String),
    Filed(String),
    ClockUnavailable(String),
    RuntimeError(String),
}

impl HookExecOutcome {
    fn explain_message(&self) -> String {
        match self {
            Self::StdinUnreadable => "hook exec: stdin could not be read; skipped".into(),
            Self::StdinTooLarge => {
                "hook exec: stdin exceeds the 1048576-byte limit; skipped".into()
            }
            Self::PayloadNotJson => "hook exec: stdin is not valid JSON; skipped".into(),
            Self::UnexpectedEvent(value) => format!(
                "hook exec: hook_event_name was {}; expected \"PostToolUseFailure\"; skipped",
                hook_explain_value(value.as_deref())
            ),
            Self::NonBashTool(value) => format!(
                "hook exec: tool_name was {}; expected \"Bash\"; skipped",
                hook_explain_value(value.as_deref())
            ),
            Self::Interrupted => "hook exec: is_interrupt is true; skipped".into(),
            Self::MissingCommand => {
                "hook exec: tool_input.command is missing or empty; skipped".into()
            }
            Self::CommandTooLong(bytes) => format!(
                "hook exec: tool_input.command is {bytes} bytes; exceeds the {HOOK_COMMAND_LIMIT}-byte limit; skipped"
            ),
            Self::CompoundCommand => {
                "hook exec: tool_input.command is not a simple command (chain, substitution, or unterminated quote); its exit does not name the friction; skipped".into()
            }
            Self::CommandRejected(reason) => {
                format!("hook exec: tool_input.command failed cut validation ({reason:?}); skipped")
            }
            Self::ProbeCommand(program) => format!(
                "hook exec: {program} is a read-only probe; non-zero exit is an expected answer; skipped"
            ),
            Self::MissingLog(path) => {
                format!("hook exec: resolved log file {path:?} is not an existing file; skipped")
            }
            Self::DuplicateOpenCommand(id) => {
                format!("hook exec: duplicate open command matches cut {id}; skipped")
            }
            Self::Filed(id) => format!("hook exec: filed cut {id}"),
            Self::ClockUnavailable(reason) => {
                format!("hook exec: clock could not be resolved ({reason:?}); skipped")
            }
            Self::RuntimeError(reason) => {
                format!("hook exec: internal hook error ({reason:?}); skipped")
            }
        }
    }
}

fn hook_explain_value(value: Option<&str>) -> String {
    value
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| "<missing>".into())
}

pub(crate) fn leading_program(command: &str) -> Option<&str> {
    command
        .split_whitespace()
        .find(|word| !is_environment_assignment(word))
        .map(|word| {
            Path::new(word)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(word)
        })
}

/// Reports whether the command chains, substitutes, or ends inside a quote. A chain's non-zero
/// exit names neither the failing step nor the friction, so the hook declines it. Like
/// `leading_program` this does not parse the shell: bare `&`, heredocs, `$'...'`, and nested
/// substitution are deliberately not recognized, and an ambiguous scan resolves toward skipping.
pub(crate) fn is_compound_command(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut index = 0;
    let mut in_single = false;
    let mut in_double = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_single {
            in_single = byte != b'\'';
        } else if in_double {
            match byte {
                // POSIX honours backslash escapes inside double quotes only.
                b'\\' => index += 1,
                b'"' => in_double = false,
                _ => {}
            }
        } else {
            match byte {
                b'\'' => in_single = true,
                b'"' => in_double = true,
                b'|' | b';' | b'\n' | b'`' => return true,
                b'&' if bytes.get(index + 1) == Some(&b'&') => return true,
                b'$' if bytes.get(index + 1) == Some(&b'(') => return true,
                _ => {}
            }
        }
        index += 1;
    }
    // An unterminated quote (or a trailing backslash inside one) leaves the scan
    // unable to say where the quoted span ends, which is exactly the ambiguity
    // r29 resolves toward skipping.
    in_single || in_double
}

fn is_environment_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(byte) if byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

pub fn run(args: HookArgs, file: Option<PathBuf>, pretty: bool, now: Timestamp) -> AppResult<i32> {
    match args.command {
        HookCommand::Install(args) => install(args, pretty),
        HookCommand::Exec(args) => {
            let _ = exec(args, file, now);
            Ok(0)
        }
    }
}

pub fn exec(_args: HookExecArgs, file: Option<PathBuf>, now: Timestamp) -> AppResult<()> {
    let outcome = exec_claude_code(file, now)
        .unwrap_or_else(|error| HookExecOutcome::RuntimeError(error.message));
    write_hook_explanation(&outcome);
    Ok(())
}

/// Explains a failure that happens before `exec` can run (currently only clock resolution),
/// so `BLOTTER_HOOK_EXPLAIN=1` never exits 0 without naming the gate.
pub fn explain_clock_failure(reason: &str) {
    write_hook_explanation(&HookExecOutcome::ClockUnavailable(reason.to_string()));
}

fn exec_claude_code(file: Option<PathBuf>, now: Timestamp) -> AppResult<HookExecOutcome> {
    let payload = match read_claude_code_payload() {
        Ok(ClaudeCodePayloadRead::Payload(payload)) => payload,
        Ok(ClaudeCodePayloadRead::TooLarge) => return Ok(HookExecOutcome::StdinTooLarge),
        Ok(ClaudeCodePayloadRead::NotJson) => return Ok(HookExecOutcome::PayloadNotJson),
        Err(_) => return Ok(HookExecOutcome::StdinUnreadable),
    };
    if payload.hook_event_name.as_deref() != Some("PostToolUseFailure") {
        return Ok(HookExecOutcome::UnexpectedEvent(payload.hook_event_name));
    }
    if payload.tool_name.as_deref() != Some("Bash") {
        return Ok(HookExecOutcome::NonBashTool(payload.tool_name));
    }
    if payload.is_interrupt == Some(true) {
        return Ok(HookExecOutcome::Interrupted);
    }
    let Some(raw_command) = payload
        .tool_input
        .and_then(|input| input.command)
        .filter(|command| !command.trim().is_empty())
    else {
        return Ok(HookExecOutcome::MissingCommand);
    };
    if raw_command.len() > HOOK_COMMAND_LIMIT {
        return Ok(HookExecOutcome::CommandTooLong(raw_command.len()));
    }
    if is_compound_command(&raw_command) {
        return Ok(HookExecOutcome::CompoundCommand);
    }
    if let Some(program) = leading_program(&raw_command)
        && PROBE_COMMANDS.contains(&program)
    {
        return Ok(HookExecOutcome::ProbeCommand(program.to_string()));
    }

    let cwd = payload_working_dir(payload.cwd)?;
    let home = store::home_dir(&cwd);
    let command = add::redact_evidence(&raw_command, home.as_deref());
    // The raw command is gated above; the stored machine-captured text is
    // validated only after full evidence redaction.
    if let Err(error) = add::validate_text(&command, "cut") {
        return Ok(HookExecOutcome::CommandRejected(error.message));
    }
    let resolved = store::discover_from(&cwd, file)?;
    if !resolved.path.is_file() {
        return Ok(HookExecOutcome::MissingLog(resolved.path));
    }

    let (agent, _) = resolve_agent(None);
    let ts = format_timestamp(now);
    let tags = vec!["auto".into(), "claude-code".into()];
    let id = compute_id(&ts, &agent, &command, Severity::Minor, &tags);
    let record = LogEvent::Cut {
        id: id.clone(),
        ts,
        agent,
        text: command.clone(),
        tags,
        severity: Severity::Minor,
        cwd: store::record_cwd(&cwd, resolved.cwd_repo(), home.as_deref()),
        source: Some("hook".into()),
        evidence: Some(Evidence {
            cmd: Some(add::redact_evidence(&raw_command, home.as_deref())),
            exit: None,
            stderr: None,
            note: payload
                .error
                .as_deref()
                .map(|error| add::redact_and_truncate(error, 1024, home.as_deref())),
        }),
    };

    store::with_exclusive(&resolved.path, false, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        let duplicate_open_command = store::fold_bytes(&bytes)
            .items
            .iter()
            .find(|item| {
                item.kind == "cut" && item.status == ItemStatus::Open && item.text == command
            })
            .map(|item| item.id.clone());
        if let Some(existing_id) = duplicate_open_command {
            return Ok(HookExecOutcome::DuplicateOpenCommand(existing_id));
        }
        store::append_json(log, &resolved.path, &bytes, &record)?;
        Ok(HookExecOutcome::Filed(id.clone()))
    })
}

fn read_claude_code_payload() -> AppResult<ClaudeCodePayloadRead> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take(HOOK_INPUT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::from_io(error, Path::new("stdin")))?;
    if bytes.len() > HOOK_INPUT_LIMIT as usize {
        return Ok(ClaudeCodePayloadRead::TooLarge);
    }
    match serde_json::from_slice(&bytes) {
        Ok(payload) => Ok(ClaudeCodePayloadRead::Payload(payload)),
        Err(_) => Ok(ClaudeCodePayloadRead::NotJson),
    }
}

fn write_hook_explanation(outcome: &HookExecOutcome) {
    if std::env::var("BLOTTER_HOOK_EXPLAIN").is_ok_and(|value| value == "1") {
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "{}", outcome.explain_message());
    }
}

fn payload_working_dir(payload_cwd: Option<String>) -> AppResult<PathBuf> {
    let process_cwd =
        std::env::current_dir().map_err(|error| AppError::from_io(error, Path::new(".")))?;
    let Some(payload_cwd) = payload_cwd.filter(|cwd| !cwd.is_empty()) else {
        return Ok(process_cwd);
    };
    let cwd = PathBuf::from(payload_cwd);
    Ok(if cwd.is_absolute() {
        cwd
    } else {
        process_cwd.join(cwd)
    })
}

fn install(args: HookInstallArgs, pretty: bool) -> AppResult<i32> {
    let settings_path = settings_path(&args)?;
    let command = claude_code_command()?;
    let mut settings = read_settings(&settings_path)?;
    let outcome = insert_claude_code_hook(&mut settings, &command)?;
    let changed = outcome.changed();
    if changed && !args.dry_run {
        write_settings_atomically(&settings_path, &settings)?;
    }

    let mut meta = Meta::new();
    if args.dry_run {
        meta.warnings.push("dry run; settings not written".into());
    }
    if let Some(action) = outcome.action() {
        let message = if args.dry_run {
            format!("dry run; hook would be {action}")
        } else {
            format!("hook {action}")
        };
        meta.warnings.push(message);
    }
    output::write_success(
        HookInstallData {
            changed,
            settings_path: settings_path.to_string_lossy().into_owned(),
            command,
        },
        pretty,
        meta,
    )
    .map_err(|error| AppError::from_io(error, Path::new("stdout")))?;
    Ok(0)
}

fn settings_path(args: &HookInstallArgs) -> AppResult<PathBuf> {
    if args.global {
        let home = std::env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                AppError::config(
                    "cannot resolve the home directory for global Claude Code settings",
                    "Set HOME or use `blotter hook install claude-code --settings PATH`.",
                )
            })?;
        return Ok(home.join(".claude/settings.json"));
    }

    let cwd = std::env::current_dir().map_err(|error| AppError::from_io(error, Path::new(".")))?;
    if let Some(settings) = args.settings.as_ref() {
        return Ok(if settings.is_absolute() {
            settings.clone()
        } else {
            cwd.join(settings)
        });
    }
    Ok(store::find_repo_root(&cwd)
        .unwrap_or(cwd)
        .join(".claude/settings.json"))
}

fn claude_code_command() -> AppResult<String> {
    let executable = std::env::current_exe()
        .map_err(|error| AppError::from_io(error, Path::new("current executable")))?;
    Ok(format!(
        "{} {CLAUDE_CODE_COMMAND_SUFFIX}",
        executable.display()
    ))
}

fn read_settings(path: &Path) -> AppResult<Value> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(json!({})),
        Err(error) => return Err(AppError::from_io(error, path)),
    };
    let settings = serde_json::from_slice::<Value>(&bytes).map_err(|error| {
        AppError::invalid_input(
            format!(
                "Claude Code settings are not valid JSON: {} ({error})",
                path.display()
            ),
            "Fix the JSON in the settings file, then rerun `blotter hook install claude-code`.",
        )
    })?;
    if settings.is_object() {
        Ok(settings)
    } else {
        Err(AppError::invalid_input(
            format!(
                "Claude Code settings must be a JSON object: {}",
                path.display()
            ),
            "Replace the settings file with a JSON object, then rerun `blotter hook install claude-code`.",
        ))
    }
}

fn insert_claude_code_hook(settings: &mut Value, command: &str) -> AppResult<HookInstallOutcome> {
    let root = settings
        .as_object_mut()
        .expect("read_settings guarantees object roots");
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks.as_object_mut().ok_or_else(|| {
        AppError::invalid_input(
            "Claude Code settings field 'hooks' must be a JSON object",
            "Fix the hooks field to be an object, then rerun `blotter hook install claude-code`.",
        )
    })?;
    let post_tool_use_failure = hooks
        .entry("PostToolUseFailure")
        .or_insert_with(|| Value::Array(Vec::new()));
    let entries = post_tool_use_failure.as_array_mut().ok_or_else(|| {
        AppError::invalid_input(
            "Claude Code settings hooks.PostToolUseFailure must be a JSON array",
            "Fix hooks.PostToolUseFailure to be an array, then rerun `blotter hook install claude-code`.",
        )
    })?;
    let current_executable = claude_code_hook_executable(command)
        .expect("claude_code_command always includes the Claude Code command suffix");
    let mut managed_hook_found = false;
    let mut outcome = HookInstallOutcome::Unchanged;
    for entry in entries.iter_mut() {
        let Some(hooks) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
            continue;
        };
        for hook in hooks {
            let Some(existing_command) = hook.get("command").and_then(Value::as_str) else {
                continue;
            };
            let Some(existing_executable) = claude_code_hook_executable(existing_command) else {
                continue;
            };
            managed_hook_found = true;
            if existing_executable != current_executable {
                *hook
                    .get_mut("command")
                    .expect("command was just read from this hook") = Value::String(command.into());
                outcome = HookInstallOutcome::Amended;
            }
        }
    }
    if managed_hook_found {
        return Ok(outcome);
    }

    entries.push(json!({
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": command}],
    }));
    Ok(HookInstallOutcome::Created)
}

fn claude_code_hook_executable(command: &str) -> Option<&str> {
    command
        .strip_suffix(CLAUDE_CODE_COMMAND_SUFFIX)
        .map(str::trim_end)
}

fn write_settings_atomically(path: &Path, settings: &Value) -> AppResult<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::invalid_input(
            format!("settings path has no parent directory: {}", path.display()),
            "Pass a settings file path with a parent directory.",
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| AppError::from_io(error, parent))?;
    let previous_permissions = fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions());
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");
    let (temporary_path, mut temporary_file) = create_temp_file(parent, filename)?;
    if let Some(permissions) = previous_permissions
        && let Err(error) = temporary_file.set_permissions(permissions)
    {
        drop(temporary_file);
        let _ = fs::remove_file(&temporary_path);
        return Err(AppError::from_io(error, &temporary_path));
    }

    let mut bytes = serde_json::to_vec_pretty(settings)
        .map_err(|error| AppError::internal(error.to_string()))?;
    bytes.push(b'\n');
    if let Err(error) = temporary_file.write_all(&bytes) {
        drop(temporary_file);
        let _ = fs::remove_file(&temporary_path);
        return Err(AppError::from_io(error, &temporary_path));
    }
    if let Err(error) = temporary_file.sync_all() {
        drop(temporary_file);
        let _ = fs::remove_file(&temporary_path);
        return Err(AppError::from_io(error, &temporary_path));
    }
    drop(temporary_file);
    if let Err(error) = fs::rename(&temporary_path, path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(AppError::from_io(error, path));
    }
    Ok(())
}

fn create_temp_file(parent: &Path, filename: &str) -> AppResult<(PathBuf, fs::File)> {
    for attempt in 0..100 {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".{filename}.blotter-{}-{sequence}-{attempt}.tmp",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(AppError::from_io(error, &path)),
        }
    }
    Err(AppError::from_io(
        std::io::Error::new(
            ErrorKind::AlreadyExists,
            "could not allocate a unique temporary settings file",
        ),
        parent,
    ))
}
