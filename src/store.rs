use crate::error::{AppError, AppResult};
use crate::{ListItem, LogEvent, Resolution, format_timestamp};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::Duration;

const LOCK_ATTEMPTS: usize = 50;
const LOCK_DELAY: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct ResolvedFile {
    pub path: PathBuf,
    pub explicit: bool,
    pub repo: Option<PathBuf>,
    pub warnings: Vec<String>,
}

impl ResolvedFile {
    /// Repo root for cwd relativization. Only a log living inside the repo
    /// stores repo-relative cwd; explicit and global logs are machine-local,
    /// keep absolute cwd, and would otherwise lose all provenance now that
    /// records carry no repo field.
    pub fn cwd_repo(&self) -> Option<&Path> {
        self.repo
            .as_deref()
            .filter(|root| self.path.starts_with(root))
    }
}

#[derive(Debug, Default)]
pub struct FoldResult {
    pub items: Vec<ListItem>,
    pub warnings: Vec<String>,
    records: BTreeMap<String, LogEvent>,
    winning_amends: HashMap<String, LogEvent>,
}

pub struct LoadedFold {
    pub items: Vec<ListItem>,
    pub warnings: Vec<String>,
}

impl FoldResult {
    pub fn record(&self, id: &str) -> Option<&LogEvent> {
        self.records.get(id)
    }

    /// Materialize a resolve against the fold that made the append decision,
    /// reporting what a complete subsequent fold would show. A base resolve
    /// activates an earlier orphan amend. An appended amend does *not* simply
    /// win: the fold picks the amend with the latest timestamp, so a stored
    /// amend carrying a later clock keeps the materialized fields, and only an
    /// exact tie falls to the appended event as the last in file order.
    /// Reached with a backdated `BLOTTER_NOW`, where the envelope would
    /// otherwise report a note that no read command agrees with.
    pub(crate) fn materialized_appended_resolution(&self, event: &LogEvent) -> Resolution {
        let LogEvent::Resolve { id, amend, .. } = event else {
            unreachable!("only resolve events materialize resolutions")
        };
        let effective = match self.winning_amends.get(id) {
            Some(stored) if !*amend => stored,
            Some(stored) if later_resolve(stored, event) => stored,
            _ => event,
        };
        resolution_from_event(effective)
    }
}

#[derive(Default)]
struct WarningCounts {
    torn: usize,
    malformed: usize,
    unknown: usize,
    duplicate_cuts: usize,
    duplicate_dogears: usize,
    duplicate_resolves: usize,
    orphans: usize,
}

pub(crate) struct ScannedLine<'a> {
    pub line: usize,
    pub raw: &'a [u8],
    pub event: Result<LogEvent, ScanIssue>,
}

pub(crate) enum ScanIssue {
    Malformed(String),
    Unknown(Option<String>),
    Torn,
}

pub fn discover(flag: Option<PathBuf>) -> AppResult<ResolvedFile> {
    let cwd = std::env::current_dir().map_err(|error| AppError::from_io(error, Path::new(".")))?;
    discover_from(&cwd, flag)
}

pub fn discover_from(cwd: &Path, flag: Option<PathBuf>) -> AppResult<ResolvedFile> {
    let repo = find_repo_root(cwd);
    if let Some(path) = flag {
        return Ok(resolved_file(absolute(cwd, path), true, repo));
    }
    if let Some(path) = std::env::var_os("BLOTTER_FILE")
        && !path.is_empty()
    {
        return Ok(resolved_file(
            absolute(cwd, PathBuf::from(path)),
            true,
            repo,
        ));
    }
    if let Some(root) = repo.clone() {
        let path = default_log_path(&root);
        return Ok(resolved_file(path, false, Some(root)));
    }
    let home = home_dir(cwd).ok_or_else(|| {
        AppError::config(
            "cannot resolve the home directory for the default blotter file",
            "Set HOME or pass --file PATH.",
        )
    })?;
    Ok(resolved_file(home.join(".blotter/log.jsonl"), false, None))
}

fn resolved_file(path: PathBuf, explicit: bool, repo: Option<PathBuf>) -> ResolvedFile {
    ResolvedFile {
        warnings: Vec::new(),
        path,
        explicit,
        repo,
    }
}

pub fn default_log_path(root: &Path) -> PathBuf {
    root.join(".blotter.jsonl")
}

pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())
        .map(Path::to_path_buf)
}

pub fn home_dir(cwd: &Path) -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|home| absolute(cwd, home))
}

pub fn record_cwd(cwd: &Path, repo: Option<&Path>, home: Option<&Path>) -> String {
    if let Some(relative) = repo.and_then(|root| cwd.strip_prefix(root).ok()) {
        return match relative.as_os_str().is_empty() {
            true => ".".into(),
            false => relative.to_string_lossy().into_owned(),
        };
    }
    // The whole-string scanner, not a prefix-anchored match: a dash-encoded home
    // (`/private/tmp/<session>/-Users-<user>-<repo>`) appears mid-path, and only
    // this scanner applies the generic `/Users/` and `/home/` rules that
    // `doctor --leaks` gates on. Its exact-home branch subsumes strip_prefix.
    crate::redact::rewrite_home_paths(&cwd.to_string_lossy(), home)
}

/// Absolutize a log path. `.` folds away textually, but `..` cannot: when a
/// component is a symlink to a directory elsewhere, the OS resolves `..`
/// against the link's target, and a lexical `pop()` would name a different file
/// than the one every later open, lock, backup, and `meta.file` acts on. A path
/// carrying `..` therefore resolves through the OS — the longest existing
/// ancestor is canonicalized and only the components that do not exist yet fold
/// lexically. The final component is never canonicalized: a final-component
/// symlink is `resolve_symlinked_log`'s policy, not this function's. A path with
/// no `..` keeps its spelling, because the lexical join already names what the
/// OS opens.
fn absolute(cwd: &Path, path: PathBuf) -> PathBuf {
    let joined = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };
    let components: Vec<Component> = joined.components().collect();
    if !components
        .iter()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return fold_lexically(PathBuf::new(), &components);
    }
    let trailing = match components.last() {
        Some(Component::Normal(_)) => components.len() - 1,
        _ => components.len(),
    };
    let mut resolved = resolve_existing_prefix(&components[..trailing]);
    if let Some(Component::Normal(name)) = components.get(trailing) {
        resolved.push(name);
    }
    resolved
}

/// Canonicalize the longest prefix of `components` that exists, then fold the
/// remainder lexically. A path that exists resolves exactly as the OS resolves
/// it; one that does not yet exist still resolves, with the lexical fold applied
/// only to the components no directory backs.
fn resolve_existing_prefix(components: &[Component]) -> PathBuf {
    for split in (1..=components.len()).rev() {
        // Verbatim, never folded: canonicalize must see `..` itself, or the
        // fold would answer for the link instead of for its target.
        let mut candidate = PathBuf::new();
        for component in &components[..split] {
            candidate.push(component.as_os_str());
        }
        if let Ok(canonical) = fs::canonicalize(&candidate) {
            return fold_lexically(canonical, &components[split..]);
        }
    }
    fold_lexically(PathBuf::new(), components)
}

fn fold_lexically(mut base: PathBuf, components: &[Component]) -> PathBuf {
    for component in components {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                base.pop();
            }
            other => base.push(other.as_os_str()),
        }
    }
    base
}

pub fn with_shared<T>(path: &Path, action: impl FnOnce(&mut File) -> AppResult<T>) -> AppResult<T> {
    let mut file = open_locked(path, false, || {
        // O_NONBLOCK does not make a FIFO fail; it makes the open return
        // immediately instead of blocking for a writer, and the regular-file
        // check in open_locked is what rejects it. The flag has no effect on
        // regular-file reads or writes on Linux or macOS.
        #[cfg(unix)]
        let opened = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path);
        #[cfg(not(unix))]
        let opened = File::open(path);
        opened.map_err(|error| AppError::from_log_open(error, path))
    })?;
    let result = action(&mut file);
    let unlock = file
        .unlock()
        .map_err(|error| AppError::from_io(error, path));
    match (result, unlock) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

pub fn read_or_empty<T>(
    path: &Path,
    explicit: bool,
    warnings: &mut Vec<String>,
    warning: &str,
    suggested_fix: &str,
    empty: impl FnOnce() -> T,
    read: impl FnOnce(&mut File) -> AppResult<T>,
) -> AppResult<(T, bool)> {
    match with_shared(path, read) {
        Ok(value) => Ok((value, true)),
        Err(error) if error.code == "not_found" && error.exit_code == 66 && !explicit => {
            warnings.push(warning.into());
            Ok((empty(), false))
        }
        Err(error) if error.code == "not_found" && error.exit_code == 66 => {
            Err(AppError::not_found(
                format!("blotter file not found: {}", path.display()),
                suggested_fix,
            ))
        }
        Err(error) => Err(error),
    }
}

pub fn load_folded(resolved: &ResolvedFile) -> AppResult<LoadedFold> {
    let mut warnings = resolved.warnings.clone();
    let (folded, _) = read_or_empty(
        &resolved.path,
        resolved.explicit,
        &mut warnings,
        "no blotter file yet; blotter add creates it",
        "Pass an existing --file PATH or run `blotter add` to create a discovered default file.",
        FoldResult::default,
        |log| {
            let bytes = read_bytes(log, &resolved.path)?;
            Ok(fold_bytes(&bytes))
        },
    )?;
    warnings.extend(folded.warnings);
    Ok(LoadedFold {
        items: folded.items,
        warnings,
    })
}

pub fn with_exclusive<T>(
    path: &Path,
    create: bool,
    action: impl FnOnce(&mut File) -> AppResult<T>,
) -> AppResult<T> {
    if create && let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| AppError::from_io(error, parent))?;
    }
    let mut file = open_locked(path, true, || {
        let mut options = OpenOptions::new();
        options.read(true).append(true).create(create);
        // See with_shared: O_NONBLOCK only keeps the open from blocking on a
        // FIFO; open_locked's regular-file check is what rejects one.
        #[cfg(unix)]
        options.custom_flags(libc::O_NONBLOCK);
        options
            .open(path)
            .map_err(|error| AppError::from_log_open(error, path))
    })?;
    let result = action(&mut file);
    let unlock = file
        .unlock()
        .map_err(|error| AppError::from_io(error, path));
    match (result, unlock) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

fn open_locked(
    path: &Path,
    exclusive: bool,
    mut open: impl FnMut() -> AppResult<File>,
) -> AppResult<File> {
    let mut file = Some(regular_file(open()?, path)?);
    // The last reopen that found nothing. Kept so an exhausted budget whose
    // final failure was a vanished log answers not_found (66) instead of
    // blaming contention that never happened; any later failure clears it.
    let mut missing: Option<AppError> = None;
    for attempt in 0..LOCK_ATTEMPTS {
        if file.is_none() {
            match open() {
                // A reopen that succeeds needs no clear: the lock attempt below
                // ends this iteration in a branch that returns or clears.
                Ok(opened) => file = Some(regular_file(opened, path)?),
                Err(error) if error.code == "not_found" => {
                    missing = Some(error);
                    delay_before_retry(attempt);
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        let result = if exclusive {
            file.as_ref().expect("file is open").try_lock()
        } else {
            file.as_ref().expect("file is open").try_lock_shared()
        };
        match result {
            Ok(()) => {
                if path_identity_matches(file.as_ref().expect("file is open"), path)? {
                    return Ok(file.take().expect("file is open"));
                }
                let stale = file.take().expect("file is open");
                let _ = stale.unlock();
                // The path names another inode now. Reopening costs the same
                // delay every other retry pays, so the attempt budget cannot
                // burn through in microseconds and report a timeout nobody
                // waited for.
                missing = None;
                delay_before_retry(attempt);
            }
            Err(error) => {
                let error: std::io::Error = error.into();
                if error.kind() != std::io::ErrorKind::WouldBlock {
                    return Err(AppError::from_io(error, path));
                }
                missing = None;
                delay_before_retry(attempt);
            }
        }
    }
    Err(missing.unwrap_or_else(|| AppError::lock_timeout(path)))
}

/// Pay the retry delay unless this was the last attempt, where the caller
/// returns instead of retrying.
fn delay_before_retry(attempt: usize) {
    if attempt + 1 < LOCK_ATTEMPTS {
        thread::sleep(LOCK_DELAY);
    }
}

/// Reject a log path that is not a regular file, before the lock and before any
/// read. flock reports ENOTSUP on a macOS FIFO, so a post-lock check would
/// surface io_error instead of invalid_input, and a path that can never be valid
/// would first burn the whole retry budget. `File::metadata` is fstat on the open
/// handle, so this cannot race a swap the way a path stat can.
fn regular_file(file: File, path: &Path) -> AppResult<File> {
    let metadata = file
        .metadata()
        .map_err(|error| AppError::from_io(error, path))?;
    if !metadata.is_file() {
        return Err(AppError::invalid_input(
            format!("blotter file is not a regular file: {}", path.display()),
            "Point --file PATH or BLOTTER_FILE at a regular JSONL file; FIFOs and devices are not accepted.",
        ));
    }
    Ok(file)
}

#[cfg(unix)]
fn path_identity_matches(file: &File, path: &Path) -> AppResult<bool> {
    // File::metadata uses fstat; fs::metadata obtains a fresh stat of the path.
    let locked = file
        .metadata()
        .map_err(|error| AppError::from_io(error, path))?;
    match std::fs::metadata(path) {
        Ok(current) => Ok(locked.dev() == current.dev() && locked.ino() == current.ino()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AppError::from_io(error, path)),
    }
}

#[cfg(not(unix))]
fn path_identity_matches(_file: &File, _path: &Path) -> AppResult<bool> {
    Ok(true)
}

pub fn read_bytes(file: &mut File, path: &Path) -> AppResult<Vec<u8>> {
    file.seek(SeekFrom::Start(0))
        .and_then(|_| {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).map(|_| bytes)
        })
        .map_err(|error| AppError::from_io(error, path))
}

pub fn write_new_file(path: &Path, bytes: &[u8], permissions: &Permissions) -> AppResult<PathBuf> {
    let mut file = create_new_file(path, permissions, false)
        .map_err(|error| AppError::from_io(error, path))?;
    if let Err(error) = file.write_all(bytes) {
        discard_new_file(file, path);
        return Err(AppError::from_io(error, path));
    }
    if let Err(error) = file.sync_all() {
        discard_new_file(file, path);
        return Err(AppError::from_io(error, path));
    }
    Ok(path.to_path_buf())
}

pub fn append_file(path: &Path, bytes: &[u8], permissions: &Permissions) -> AppResult<PathBuf> {
    let (mut file, created) = match create_new_file(path, permissions, false) {
        Ok(file) => (file, true),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => (
            OpenOptions::new()
                .append(true)
                .open(path)
                .map_err(|error| AppError::from_io(error, path))?,
            false,
        ),
        Err(error) => return Err(AppError::from_io(error, path)),
    };
    if let Err(error) = file.write_all(bytes) {
        if created {
            discard_new_file(file, path);
        }
        return Err(AppError::from_io(error, path));
    }
    if let Err(error) = file.sync_all() {
        if created {
            discard_new_file(file, path);
        }
        return Err(AppError::from_io(error, path));
    }
    Ok(path.to_path_buf())
}

pub fn replace_log(
    path: &Path,
    bytes: &[u8],
    permissions: &Permissions,
    temporary_suffix: &str,
) -> AppResult<()> {
    let temporary = suffixed_path(path, temporary_suffix);
    let mut file = create_new_file(&temporary, permissions, true)
        .map_err(|error| AppError::from_io(error, &temporary))?;
    if let Err(error) = file.write_all(bytes) {
        discard_new_file(file, &temporary);
        return Err(AppError::from_io(error, &temporary));
    }
    if let Err(error) = file.sync_all() {
        discard_new_file(file, &temporary);
        return Err(AppError::from_io(error, &temporary));
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(AppError::from_io(error, path));
    }
    if let Some(parent) = path.parent()
        && let Ok(directory) = File::open(parent)
    {
        let _ = directory.sync_all();
    }
    Ok(())
}

/// Resolve a symlinked log path to its target before a copy-and-swap, so the
/// backup, sidecar, and atomic replacement all act on the real file and the
/// link survives. Only final-component links are chased; parent components
/// keep their spelling so envelope paths stay stable for regular files.
pub fn resolve_symlinked_log(path: &Path) -> AppResult<PathBuf> {
    let mut current = path.to_path_buf();
    for _ in 0..40 {
        let metadata =
            fs::symlink_metadata(&current).map_err(|error| AppError::from_io(error, &current))?;
        if !metadata.file_type().is_symlink() {
            return Ok(current);
        }
        let target = fs::read_link(&current).map_err(|error| AppError::from_io(error, &current))?;
        current = if target.is_absolute() {
            target
        } else {
            match current.parent() {
                Some(parent) => parent.join(&target),
                None => target,
            }
        };
    }
    Err(AppError::from_io(
        std::io::Error::other("too many levels of symbolic links"),
        path,
    ))
}

pub fn suffixed_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

pub fn backup_timestamp(now: jiff::Timestamp) -> String {
    format_timestamp(now)
        .chars()
        .filter(|character| !matches!(character, '-' | ':' | '.'))
        .collect()
}

pub fn restore_hint(backup: &Path, path: &Path) -> String {
    format!("cp {} {}", shell_quote(backup), shell_quote(path))
}

fn create_new_file(
    path: &Path,
    permissions: &Permissions,
    set_permissions_on_non_unix: bool,
) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(permissions.mode());
    let file = options.open(path)?;
    #[cfg(unix)]
    let permissions_result = {
        let _ = set_permissions_on_non_unix;
        file.set_permissions(permissions.clone())
    };
    #[cfg(not(unix))]
    let permissions_result = set_permissions_on_non_unix
        .then(|| file.set_permissions(permissions.clone()))
        .transpose()
        .map(|_| ());
    if let Err(error) = permissions_result {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(file)
}

fn discard_new_file(file: File, path: &Path) {
    drop(file);
    let _ = fs::remove_file(path);
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

pub fn append_json<T: serde::Serialize>(
    file: &mut File,
    path: &Path,
    prior: &[u8],
    record: &T,
) -> AppResult<()> {
    let mut record_bytes = Vec::new();
    serde_json::to_writer(&mut record_bytes, record)
        .map_err(|error| AppError::internal(error.to_string()))?;
    record_bytes.push(b'\n');
    append_bytes(file, path, prior, &record_bytes)
}

pub fn append_unique(path: &Path, record: LogEvent, dry_run: bool) -> AppResult<(bool, LogEvent)> {
    if dry_run {
        return Ok((false, record));
    }
    let id = record.id().expect("new records have IDs").to_owned();
    let kind = match &record {
        LogEvent::Cut { .. } => "cut",
        LogEvent::Dogear { .. } => "dogear",
        _ => unreachable!("append_unique only receives cut or dogear records"),
    };
    with_exclusive(path, true, |log| {
        let bytes = read_bytes(log, path)?;
        let records = fold_records(&bytes);
        if let Some(existing) = records.get(&id) {
            return if std::mem::discriminant(&record) == std::mem::discriminant(existing) {
                Ok((false, existing.clone()))
            } else {
                Err(AppError::internal(format!(
                    "{kind} ID collides with an existing non-{kind} record"
                )))
            };
        }
        append_json(log, path, &bytes, &record)?;
        Ok((true, record))
    })
}

pub fn append_json_batch<T: serde::Serialize>(
    file: &mut File,
    path: &Path,
    prior: &[u8],
    records: &[T],
) -> AppResult<()> {
    let mut record_bytes = Vec::new();
    for record in records {
        serde_json::to_writer(&mut record_bytes, record)
            .map_err(|error| AppError::internal(error.to_string()))?;
        record_bytes.push(b'\n');
    }
    append_bytes(file, path, prior, &record_bytes)
}

fn append_bytes(file: &mut File, path: &Path, prior: &[u8], record_bytes: &[u8]) -> AppResult<()> {
    append_bytes_with(file, path, prior, record_bytes, |file, bytes| {
        file.write_all(bytes)
    })
}

fn append_bytes_with(
    file: &mut File,
    path: &Path,
    prior: &[u8],
    record_bytes: &[u8],
    write: impl FnOnce(&mut File, &[u8]) -> std::io::Result<()>,
) -> AppResult<()> {
    let original_len = file
        .metadata()
        .map_err(|error| AppError::from_io(error, path))?
        .len();
    let mut bytes = Vec::new();
    if !is_empty_log(prior) && !prior.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(record_bytes);
    // If the write fails, roll back to the pre-write length; if rollback also fails, surface both.
    if let Err(error) = write(file, &bytes) {
        if let Err(rollback) = file.set_len(original_len) {
            return Err(AppError {
                code: "io_error",
                message: format!(
                    "append failed: {error}; rollback to original length {original_len} failed: {rollback}"
                ),
                details: json!({}),
                retryable: false,
                suggested_fix: "Check the blotter file and filesystem, then retry.".into(),
                exit_code: 74,
            });
        }
        return Err(AppError::from_io(error, path));
    }
    Ok(())
}

/// A log holding no physical line: an empty file, or the single newline that
/// `scan` reads as a terminator rather than a line (r26). The append path and
/// `scan` share this predicate so a log both call empty stays empty for both:
/// the appender adds no tear-healing separator to it, and the leading empty
/// segment the append leaves behind is never counted as a line.
pub(crate) fn is_empty_log(bytes: &[u8]) -> bool {
    bytes.is_empty() || bytes == b"\n"
}

/// Scan physical JSONL lines once. A final non-newline line is accepted only
/// when its decoded JSON carries a recognized kind, so consumers cannot
/// disagree on torn tails.
pub(crate) fn scan(bytes: &[u8]) -> impl Iterator<Item = ScannedLine<'_>> + '_ {
    // A leading empty segment is the terminator of an empty log, not a physical
    // line: the log was empty or held only "\n" when the record was appended,
    // and an append-only writer cannot remove the byte that precedes it. An
    // empty segment after a record is still malformed.
    let terminated = bytes.ends_with(b"\n");
    let body = if terminated {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    };
    let line_count = body.split(|byte| *byte == b'\n').count();
    body.split(|byte| *byte == b'\n')
        .enumerate()
        .filter_map(move |(index, raw)| {
            let final_line = index + 1 == line_count;
            if raw.is_empty() && index == 0 {
                return None;
            }
            let decoded = serde_json::from_slice::<Value>(raw);
            let known = decoded.as_ref().ok().and_then(known_kind);
            let event = if final_line && !terminated && known.is_none() {
                Err(ScanIssue::Torn)
            } else {
                match decoded {
                    Ok(value) => parse_event(value, known),
                    Err(_) => Err(ScanIssue::Malformed("line is not valid JSON".into())),
                }
            };
            Some(ScannedLine {
                line: index + 1,
                raw,
                event,
            })
        })
}

fn known_kind(value: &Value) -> Option<&'static str> {
    match value.get("kind").and_then(Value::as_str) {
        Some("cut") => Some("cut"),
        Some("dogear") => Some("dogear"),
        Some("resolve") => Some("resolve"),
        _ => None,
    }
}

fn parse_event(value: Value, known: Option<&'static str>) -> Result<LogEvent, ScanIssue> {
    let unknown = value.get("kind").and_then(Value::as_str).map(str::to_owned);
    match serde_json::from_value::<LogEvent>(value) {
        Ok(LogEvent::Unknown) => Err(ScanIssue::Unknown(unknown)),
        Ok(event) => {
            let ts = match &event {
                LogEvent::Cut { ts, .. }
                | LogEvent::Dogear { ts, .. }
                | LogEvent::Resolve { ts, .. } => ts,
                LogEvent::Unknown => unreachable!("unknown events are classified above"),
            };
            match ts.parse::<jiff::Timestamp>() {
                Ok(_) => Ok(event),
                Err(_) => Err(ScanIssue::Malformed(format!(
                    "{} ts is not a full RFC3339 timestamp",
                    known.expect("parsed events have a known kind")
                ))),
            }
        }
        Err(error) => match known {
            Some(kind) => Err(ScanIssue::Malformed(format!(
                "invalid {kind} record: {error}"
            ))),
            None => Err(ScanIssue::Unknown(unknown)),
        },
    }
}

/// Is `stored` strictly later than `candidate`? Only strictly: the fold breaks
/// an exact tie toward the last event in file order, and `candidate` is the one
/// being appended. An unparseable timestamp never wins, so this cannot panic on
/// a hand-edited log the way the fold's validated parse would.
fn later_resolve(stored: &LogEvent, candidate: &LogEvent) -> bool {
    let timestamp = |event: &LogEvent| match event {
        LogEvent::Resolve { ts, .. } => ts.parse::<jiff::Timestamp>().ok(),
        _ => None,
    };
    match (timestamp(stored), timestamp(candidate)) {
        (Some(stored), Some(candidate)) => stored > candidate,
        _ => false,
    }
}

fn resolution_from_event(event: &LogEvent) -> Resolution {
    let LogEvent::Resolve {
        ts,
        agent,
        note,
        task,
        pr,
        commit,
        url,
        dropped,
        amend,
        ..
    } = event
    else {
        unreachable!("only resolve events materialize resolutions")
    };
    Resolution {
        ts: ts.clone(),
        agent: agent.clone(),
        note: note.clone(),
        task: task.clone(),
        pr: pr.clone(),
        commit: commit.clone(),
        url: url.clone(),
        dropped: *dropped,
        amended: *amend,
    }
}

/// Records-only fold for the append path. `append_unique` needs one fact — does
/// this ID already exist — so it skips the resolution join, the ListItem clones,
/// the timestamp parses, and the sort that `fold_bytes` would discard, inside
/// the exclusive lock. Tag normalization must match `fold_bytes`: the duplicate
/// branch returns this record straight into the add/dogear response envelope.
fn fold_records(bytes: &[u8]) -> BTreeMap<String, LogEvent> {
    let mut records = BTreeMap::<String, LogEvent>::new();
    for scanned in scan(bytes) {
        let Ok(mut event) = scanned.event else {
            continue;
        };
        match &mut event {
            LogEvent::Cut { tags, .. } | LogEvent::Dogear { tags, .. } => {
                tags.sort();
                tags.dedup();
            }
            LogEvent::Resolve { .. } | LogEvent::Unknown => continue,
        }
        let id = event.id().expect("parsed records have IDs").to_owned();
        records.entry(id).or_insert(event);
    }
    records
}

pub fn fold_bytes(bytes: &[u8]) -> FoldResult {
    let mut records = BTreeMap::<String, LogEvent>::new();
    let mut resolves = HashMap::<String, LogEvent>::new();
    // Amends carry their parsed timestamp so the winner is chosen by clock, not
    // by byte position, without reparsing the incumbent for every candidate.
    let mut amends = HashMap::<String, (jiff::Timestamp, LogEvent)>::new();
    let mut counts = WarningCounts::default();
    for scanned in scan(bytes) {
        match scanned.event {
            Err(ScanIssue::Malformed(_)) => counts.malformed += 1,
            Err(ScanIssue::Unknown(_)) => counts.unknown += 1,
            Err(ScanIssue::Torn) => counts.torn += 1,
            Ok(mut event) => match &mut event {
                LogEvent::Cut { tags, .. } => {
                    // Fold normalizes legacy tag arrays for list output. Doctor
                    // receives the scanner's unmodified parsed event instead.
                    tags.sort();
                    tags.dedup();
                    let id = event.id().expect("parsed cuts have IDs").to_owned();
                    if let std::collections::btree_map::Entry::Vacant(entry) = records.entry(id) {
                        entry.insert(event);
                    } else {
                        counts.duplicate_cuts += 1;
                    }
                }
                LogEvent::Dogear { tags, .. } => {
                    tags.sort();
                    tags.dedup();
                    let id = event.id().expect("parsed dogears have IDs").to_owned();
                    if let std::collections::btree_map::Entry::Vacant(entry) = records.entry(id) {
                        entry.insert(event);
                    } else {
                        counts.duplicate_dogears += 1;
                    }
                }
                LogEvent::Resolve { id, ts, amend, .. } => {
                    let id = id.clone();
                    let amend = *amend;
                    if amend {
                        let timestamp = ts
                            .parse::<jiff::Timestamp>()
                            .expect("parsed resolves have valid RFC3339 timestamps");
                        match amends.entry(id) {
                            std::collections::hash_map::Entry::Occupied(mut entry) => {
                                // `>=`, not `>`: equal timestamps are reachable
                                // under a frozen BLOTTER_NOW, and there the last
                                // amend in file order keeps winning.
                                if timestamp >= entry.get().0 {
                                    entry.insert((timestamp, event));
                                }
                            }
                            std::collections::hash_map::Entry::Vacant(entry) => {
                                entry.insert((timestamp, event));
                            }
                        }
                    } else if let std::collections::hash_map::Entry::Vacant(entry) =
                        resolves.entry(id)
                    {
                        entry.insert(event);
                    } else {
                        counts.duplicate_resolves += 1;
                    }
                }
                LogEvent::Unknown => counts.unknown += 1,
            },
        }
    }

    // Base resolves remain first-wins. The winning amend is the one with the
    // latest timestamp, with the last in file order breaking an exact tie; file
    // position never decides, because a `merge=union` log concatenates branches
    // in branch order. A latest amend only materializes when the full scan found
    // a base resolve, so merge-reordered base resolves work. Every winner is
    // also kept in `winning_amends`, whether or not a base resolve claimed it,
    // so `materialized_appended_resolution` can apply the same rule to an amend
    // that has not been folded yet.
    let mut winning_amends = HashMap::new();
    for (id, (_, amend)) in amends {
        winning_amends.insert(id.clone(), amend.clone());
        match resolves.entry(id) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.insert(amend);
            }
            // An amend with no base resolve stays out of `resolves`, so the
            // record remains open, exactly as before.
            std::collections::hash_map::Entry::Vacant(_) => counts.orphans += 1,
        }
    }

    for id in resolves.keys() {
        if !records.contains_key(id) {
            counts.orphans += 1;
        }
    }
    let mut items: Vec<_> = records
        .values()
        .cloned()
        .map(|record| {
            let resolution = record
                .id()
                .and_then(|id| resolves.get(id))
                .map(resolution_from_event);
            let item = ListItem::from_record(record, resolution);
            let timestamp = item
                .ts
                .parse::<jiff::Timestamp>()
                .expect("folded items have valid RFC3339 timestamps");
            (item, timestamp)
        })
        .collect();
    items.sort_by(|(left, left_timestamp), (right, right_timestamp)| {
        match (left.kind.as_str(), right.kind.as_str()) {
            ("cut", "cut") => right
                .severity
                .expect("cut has severity")
                .rank()
                .cmp(&left.severity.expect("cut has severity").rank())
                .then_with(|| right_timestamp.cmp(left_timestamp))
                .then_with(|| left.id.cmp(&right.id)),
            ("dogear", "dogear") => right_timestamp
                .cmp(left_timestamp)
                .then_with(|| left.id.cmp(&right.id)),
            ("cut", "dogear") => std::cmp::Ordering::Less,
            ("dogear", "cut") => std::cmp::Ordering::Greater,
            _ => left.kind.cmp(&right.kind),
        }
    });
    let items = items.into_iter().map(|(item, _)| item).collect();

    let mut warnings = Vec::new();
    warning(&mut warnings, counts.torn, "torn final line");
    warning(&mut warnings, counts.malformed, "malformed line");
    warning(&mut warnings, counts.unknown, "unknown event");
    warning(&mut warnings, counts.duplicate_cuts, "duplicate cut");
    warning(&mut warnings, counts.duplicate_dogears, "duplicate dogear");
    warning(
        &mut warnings,
        counts.duplicate_resolves,
        "duplicate resolve",
    );
    warning(&mut warnings, counts.orphans, "orphan resolve");
    FoldResult {
        items,
        warnings,
        records,
        winning_amends,
    }
}

fn warning(warnings: &mut Vec<String>, count: usize, label: &str) {
    if count > 0 {
        warnings.push(format!(
            "skipped {count} {label}{}",
            if count == 1 { "" } else { "s" }
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ItemStatus, Severity, compute_id};
    use std::io::Write;
    use tempfile::TempDir;

    fn cut(id: &str) -> String {
        cut_with_text(id, "x")
    }

    fn cut_with_text(id: &str, text: &str) -> String {
        serde_json::json!({
            "kind":"cut", "id":id, "ts":"2026-07-09T00:00:00.000Z",
            "agent":"a", "text":text, "tags":[], "severity":"minor",
            "cwd":"/tmp", "repo":null
        })
        .to_string()
    }

    fn resolve(id: &str) -> String {
        serde_json::json!({
            "kind":"resolve", "id":id, "ts":"2026-07-10T00:00:00.000Z",
            "agent":"a", "note":null
        })
        .to_string()
    }

    #[cfg(unix)]
    #[test]
    fn exclusive_lock_reopens_a_replaced_path_before_appending() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cuts.jsonl");
        std::fs::write(&path, b"old\n").unwrap();

        let holder = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        holder.lock().unwrap();

        let preopened = OpenOptions::new()
            .read(true)
            .append(true)
            .open(&path)
            .unwrap();
        let (opened_tx, opened_rx) = std::sync::mpsc::channel();
        let writer_path = path.clone();
        let writer = std::thread::spawn(move || {
            let mut first_open = Some(preopened);
            let mut file = open_locked(&writer_path, true, || {
                if let Some(file) = first_open.take() {
                    // The writer now owns a descriptor for the old inode.
                    opened_tx.send(()).unwrap();
                    Ok(file)
                } else {
                    OpenOptions::new()
                        .read(true)
                        .append(true)
                        .open(&writer_path)
                        .map_err(|error| AppError::from_log_open(error, &writer_path))
                }
            })
            .unwrap();
            file.write_all(b"writer\n").unwrap();
            file.unlock().unwrap();
        });

        opened_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let replacement = temp.path().join("replacement.jsonl");
        std::fs::write(&replacement, b"replacement\n").unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        holder.unlock().unwrap();
        writer.join().unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"replacement\nwriter\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_permanent_path_identity_mismatch_still_pays_the_retry_delay() {
        // The locked descriptor never names the requested path, so every
        // attempt mismatches. The budget must still span the published bound
        // rather than burning through in microseconds.
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cuts.jsonl");
        let other = temp.path().join("other.jsonl");
        std::fs::write(&path, b"").unwrap();
        std::fs::write(&other, b"").unwrap();

        let started = std::time::Instant::now();
        let error = open_locked(&path, true, || {
            OpenOptions::new()
                .read(true)
                .append(true)
                .open(&other)
                .map_err(|error| AppError::from_log_open(error, &other))
        })
        .expect_err("a permanent identity mismatch never locks the path");
        let elapsed = started.elapsed();

        assert_eq!(error.code, "lock_timeout");
        assert_eq!(error.exit_code, 75);
        assert!(
            elapsed >= LOCK_DELAY * (LOCK_ATTEMPTS as u32 - 1),
            "gave up after {elapsed:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_log_that_vanishes_during_the_retry_budget_reports_not_found() {
        // First open lands on another inode, so the identity check rejects it;
        // every reopen then finds nothing. Exhaustion must name the missing
        // log, not contention that never happened.
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cuts.jsonl");
        let other = temp.path().join("other.jsonl");
        std::fs::write(&other, b"").unwrap();

        let mut first = true;
        let error = open_locked(&path, true, || {
            let target = if std::mem::take(&mut first) {
                other.as_path()
            } else {
                path.as_path()
            };
            OpenOptions::new()
                .read(true)
                .append(true)
                .open(target)
                .map_err(|error| AppError::from_log_open(error, target))
        })
        .expect_err("a log that never appears cannot be locked");

        assert_eq!(error.code, "not_found");
        assert_eq!(error.exit_code, 66);
    }

    #[test]
    fn batch_append_rollback_restores_a_torn_tail_after_partial_write_failure() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cuts.jsonl");
        let original = b"{\"kind\":\"cut\"}\n{\"kind\":";
        std::fs::write(&path, original).unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .open(&path)
            .unwrap();

        let error = append_bytes_with(
            &mut file,
            &path,
            original,
            b"{\"kind\":\"resolve\"}\n{\"kind\":\"resolve\"}\n",
            |file, bytes| {
                file.write_all(&bytes[..8])?;
                Err(std::io::Error::other("injected partial write failure"))
            },
        )
        .unwrap_err();

        assert_eq!(error.code, "io_error");
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn fold_matrix() {
        let id = compute_id("2026-07-09T00:00:00.000Z", "a", "x", Severity::Minor, &[]);
        let cases = [
            ("cut", format!("{}\n", cut(&id)), 1, ItemStatus::Open, 0),
            (
                "resolve before cut",
                format!("{}\n{}\n", resolve(&id), cut(&id)),
                1,
                ItemStatus::Resolved,
                0,
            ),
            (
                "duplicates",
                format!(
                    "{}\n{}\n{}\n{}\n",
                    cut(&id),
                    cut(&id),
                    resolve(&id),
                    resolve(&id)
                ),
                1,
                ItemStatus::Resolved,
                2,
            ),
            (
                "unknown malformed orphan",
                format!(
                    "{{\"kind\":\"future\"}}\nnope\n{}\n{}\n",
                    resolve("bl_deadbeef0000"),
                    cut(&id)
                ),
                1,
                ItemStatus::Open,
                3,
            ),
            (
                "torn tail",
                format!("{}\n{{\"kind\":", cut(&id)),
                1,
                ItemStatus::Open,
                1,
            ),
            (
                "all adversarial orderings interleaved",
                format!(
                    "{}\n{{\"kind\":\"future\"}}\n{}\n{}\n{}\n{}\n{}\nnope\n{{\"kind\":",
                    resolve(&id),
                    cut(&id),
                    cut(&id),
                    cut_with_text(&id, "conflicting payload"),
                    resolve(&id),
                    resolve("bl_deadbeef0000"),
                ),
                1,
                ItemStatus::Resolved,
                6,
            ),
        ];
        for (name, input, item_count, status, warning_count) in cases {
            let folded = fold_bytes(input.as_bytes());
            assert_eq!(folded.items.len(), item_count, "{name}");
            if !folded.items.is_empty() {
                assert_eq!(folded.items[0].status, status, "{name}");
                assert_eq!(folded.items[0].text, "x", "{name}");
            }
            assert_eq!(folded.warnings.len(), warning_count, "{name}");
        }
    }
}
