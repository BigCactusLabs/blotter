use crate::cli::AddArgs;
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::redact::{evidence_delimiter, rewrite_home_paths};
use crate::store;
use crate::{Evidence, LogEvent, compute_id, format_timestamp, resolve_agent_checked};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[cfg(not(unix))]
use std::fs::File;
#[cfg(unix)]
use std::fs::OpenOptions;
use std::io::{IsTerminal, Read};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const STDERR_INPUT_LIMIT: u64 = 1024 * 1024;
const STDIN_INPUT_LIMIT: u64 = 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct AddData {
    pub changed: bool,
    pub record: LogEvent,
}

pub fn run(args: AddArgs, file: Option<PathBuf>, pretty: bool, now: Timestamp) -> AppResult<i32> {
    let resolved = store::discover(file)?;
    let cwd = std::env::current_dir()
        .map_err(|error| AppError::from_io(error, std::path::Path::new(".")))?;
    let home = store::home_dir(&cwd);
    let evidence = build_evidence(&args, home.as_deref())?;
    let text = rewrite_home_paths(&read_text(args.text, "cut", "add")?, home.as_deref());
    validate_text(&text, "cut")?;
    let (agent, source) = resolve_agent_checked(args.agent, true)?;
    let mut tags = args.tags;
    tags.sort();
    tags.dedup();
    let mut warnings = resolved.warnings.clone();
    let ts = format_timestamp(now);
    let supplied_evidence = evidence.is_some();
    let resolution_text =
        text.trim_start().starts_with("RESOLUTION") || text.trim_start().starts_with("RESOLVED");
    let record = LogEvent::Cut {
        id: compute_id(&ts, &agent, &text, args.severity, &tags),
        ts,
        agent,
        text,
        tags,
        severity: args.severity,
        cwd: store::record_cwd(&cwd, resolved.cwd_repo(), home.as_deref()),
        source: None,
        evidence,
    };
    if resolution_text {
        warnings.push(
            "resolution_text: this looks like a resolution; use `blotter resolve <id>` for an existing cut".into(),
        );
    }

    let (changed, record) = store::append_unique(&resolved.path, record, args.dry_run)?;
    if args.dry_run {
        warnings.push("dry run; no record appended".into());
    } else if !changed {
        warnings.push(
            if supplied_evidence {
                "duplicate_cut: existing record returned; later evidence was not stored"
            } else {
                "duplicate cut; existing record returned"
            }
            .into(),
        );
    }
    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.agent_source = Some(source.into());
    meta.warnings = warnings;
    output::write_success(AddData { changed, record }, pretty, meta)
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(0)
}

fn build_evidence(args: &AddArgs, home: Option<&Path>) -> AppResult<Option<Evidence>> {
    let stderr = args.stderr_file.as_deref().map(read_stderr).transpose()?;
    if args.cmd.is_none() && args.exit_code.is_none() && stderr.is_none() && args.evidence.is_none()
    {
        return Ok(None);
    }
    Ok(Some(Evidence {
        cmd: args
            .cmd
            .as_deref()
            .map(|value| redact_evidence(value, home)),
        exit: args.exit_code,
        stderr: stderr.map(|value| redact_and_truncate(&value, 4096, home)),
        note: args
            .evidence
            .as_deref()
            .map(|value| redact_evidence(value, home)),
    }))
}

fn read_stderr(path: &std::path::Path) -> AppResult<String> {
    // Opening first makes the handle, rather than a path lookup, the object we validate.
    // OpenOptions follows symlinks, preserving the accepted symlink-to-regular-file policy.
    #[cfg(unix)]
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| AppError::from_evidence_file(error, path))?;
    #[cfg(not(unix))]
    let mut file = File::open(path).map_err(|error| AppError::from_evidence_file(error, path))?;
    let metadata = file
        .metadata()
        .map_err(|error| AppError::from_evidence_file(error, path))?;
    if !metadata.is_file() {
        return Err(AppError::invalid_input(
            format!(
                "stderr evidence path is not a regular file: {}",
                path.display()
            ),
            "Pass a regular UTF-8 file to --stderr-file PATH; FIFOs and devices are not accepted.",
        ));
    }
    if metadata.len() > STDERR_INPUT_LIMIT {
        return Err(AppError::invalid_input(
            format!(
                "stderr evidence file exceeds the {}-byte read limit: {}",
                STDERR_INPUT_LIMIT,
                path.display()
            ),
            "Pass a smaller stderr file to --stderr-file PATH; stored sanitized stderr is capped at 4096 bytes.",
        ));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(STDERR_INPUT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::from_evidence_file(error, path))?;
    if bytes.len() > STDERR_INPUT_LIMIT as usize {
        return Err(AppError::invalid_input(
            format!(
                "stderr evidence file exceeds the {}-byte read limit: {}",
                STDERR_INPUT_LIMIT,
                path.display()
            ),
            "Pass a smaller stderr file to --stderr-file PATH; stored sanitized stderr is capped at 4096 bytes.",
        ));
    }
    String::from_utf8(bytes).map_err(|_| {
        AppError::invalid_input(
            format!("stderr file is not valid UTF-8: {}", path.display()),
            "Pass a UTF-8 stderr file with --stderr-file PATH.",
        )
    })
}

pub(crate) fn redact_and_truncate(value: &str, max_bytes: usize, home: Option<&Path>) -> String {
    truncate_utf8(&redact_evidence(value, home), max_bytes)
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

const SENSITIVE_KEYS: &str = "accesskey apikey authorization authtoken bearer clientsecret dbpassword key passwd password secret token";

fn word(s: &str, i: usize) -> bool {
    s.as_bytes()
        .get(i)
        .is_some_and(|b| b.is_ascii_alphanumeric())
}

fn assignment_value_span(input: &str, end: usize) -> Option<(usize, usize)> {
    let rest = input[end..].trim_start_matches('"').trim_start();
    let separator = rest.chars().next().filter(|c| matches!(c, '=' | ':'))?;
    let rest = rest[separator.len_utf8()..].trim_start();
    let rest = rest.trim_start_matches(['"', '\'']);
    let start = input.len() - rest.len();
    let end = rest
        .find(|character: char| evidence_delimiter(character))
        .map_or(input.len(), |offset| start + offset);
    (start < end).then_some((start, end))
}

fn extend_one_token(input: &str, end: usize) -> usize {
    let rest = &input[end..];
    let trimmed = rest.trim_start_matches([' ', '\t']);
    if trimmed.len() == rest.len() || trimmed.is_empty() {
        return end;
    }
    let start = input.len() - trimmed.len();
    trimmed
        .find(|character: char| evidence_delimiter(character))
        .map_or(input.len(), |offset| start + offset)
}

pub(crate) fn redact_evidence(input: &str, home: Option<&Path>) -> String {
    let rewritten = rewrite_home_paths(input, home);
    let input = rewritten.as_str();
    let lower = input.to_ascii_lowercase();
    let mut spans = Vec::new();
    for key in SENSITIVE_KEYS.split_ascii_whitespace() {
        for (start, _) in lower.match_indices(key) {
            let end = start + key.len();
            if word(input, start.wrapping_sub(1)) || word(input, end) {
                continue;
            }
            if let Some((value_start, value_end)) = assignment_value_span(input, end) {
                // "Authorization: Bearer <credential>": the first token is the
                // scheme; the credential follows it. Cover one more token so the
                // secret, not just the scheme word, is redacted.
                let value_end = if key == "authorization" {
                    extend_one_token(input, value_end)
                } else {
                    value_end
                };
                spans.push((value_start, value_end));
            }
        }
    }
    for (start, scheme) in lower
        .match_indices("http://")
        .chain(lower.match_indices("https://"))
    {
        let authority_start = start + scheme.len();
        let authority_end = input[authority_start..]
            .find(|character: char| "/?#\"' \t\r\n".contains(character))
            .map_or(input.len(), |offset| authority_start + offset);
        if let Some(at) = input[authority_start..authority_end].rfind('@') {
            spans.push((authority_start, authority_start + at));
        }
    }
    let mut token_start = None;
    for (end, character) in input
        .char_indices()
        .chain(std::iter::once((input.len(), ' ')))
    {
        if character.is_ascii_alphanumeric() || "_-./+=".contains(character) {
            token_start.get_or_insert(end);
            continue;
        }
        let Some(start) = token_start.take() else {
            continue;
        };
        let token = &input[start..end];
        let unique = token.bytes().collect::<HashSet<_>>().len();
        let mixed = token.bytes().any(|byte| byte.is_ascii_lowercase())
            && token.bytes().any(|byte| byte.is_ascii_uppercase())
            && token.bytes().any(|byte| byte.is_ascii_digit());
        if token.len() >= 24 && unique >= 12 && mixed {
            spans.push((start, end));
        }
    }
    spans.sort_unstable();
    // Merge overlaps before emitting: dropping an overlapping span whole would
    // leak the part of a secret that extends past the previous span's end.
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        match merged.last_mut() {
            Some((_, last_end)) if start <= *last_end => *last_end = (*last_end).max(end),
            _ => merged.push((start, end)),
        }
    }
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    for (start, end) in merged {
        output.push_str(&input[cursor..start]);
        output.push_str("<redacted>");
        cursor = end;
    }
    output.push_str(&input[cursor..]);
    output
}

pub(crate) fn read_text(
    text: Option<String>,
    record_name: &str,
    command_name: &str,
) -> AppResult<String> {
    let use_stdin =
        text.as_deref() == Some("-") || (text.is_none() && !std::io::stdin().is_terminal());
    let mut text = if use_stdin {
        // r25 validates the *redacted* text, so `validate_text`'s 10000-byte
        // limit cannot gate the raw read: oversized input that redacts below it
        // is accepted. Gate this lane at the sibling stdin/stderr scale instead,
        // so an endless producer cannot grow the buffer without bound.
        //
        // The budget is on bytes *read*, so it is measured before the trailing
        // newline is trimmed, exactly as `--stderr-file` measures the whole
        // file. Trimming first would leave a hole at the boundary: a stream of
        // exactly the limit, then a newline, then more data fills the reader to
        // the cap, and the trim would drop that newline back to the limit and
        // accept — silently discarding everything the reader never reached.
        // Test length before decoding, too: a stream cut mid-codepoint at the
        // cap would otherwise report a misleading UTF-8 error.
        let mut input = Vec::new();
        std::io::stdin()
            .lock()
            .take(STDIN_INPUT_LIMIT + 1)
            .read_to_end(&mut input)
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdin")))?;
        if input.len() > STDIN_INPUT_LIMIT as usize {
            return Err(AppError::invalid_input(
                format!(
                    "{record_name} text from stdin exceeds the {STDIN_INPUT_LIMIT}-byte read limit"
                ),
                format!("Pipe at most {STDIN_INPUT_LIMIT} bytes to `blotter {command_name} -`."),
            ));
        }
        String::from_utf8(input).map_err(|_| {
            AppError::invalid_input(
                format!("{record_name} text from stdin is not valid UTF-8"),
                format!("Pipe UTF-8 text to `blotter {command_name} -`."),
            )
        })?
    } else {
        text.ok_or_else(|| {
            AppError::invalid_argument(
                format!("{command_name} requires TEXT when stdin is a terminal"),
                format!(
                    "Run `blotter {command_name} \"text\"` or pipe text to `blotter {command_name} -`."
                ),
            )
        })?
    };
    while text.ends_with('\n') || text.ends_with('\r') {
        text.pop();
    }
    Ok(text)
}

pub(crate) fn validate_text(text: &str, record_name: &str) -> AppResult<()> {
    if text.trim().is_empty() {
        return Err(AppError::invalid_input(
            format!("{record_name} text cannot be empty or whitespace-only"),
            "Pass non-empty TEXT or pipe it on stdin.",
        ));
    }
    if text.len() > 10_000 {
        return Err(AppError::invalid_input(
            format!(
                "{record_name} text is {} bytes; the maximum is 10000",
                text.len()
            ),
            format!("Shorten the {record_name} text to at most 10000 UTF-8 bytes."),
        ));
    }
    Ok(())
}
