use crate::cli::DogearArgs;
use crate::commands::add::{read_text, redact_evidence, validate_text};
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::redact::rewrite_home_paths;
use crate::store;
use crate::{LogEvent, compute_dogear_id, format_timestamp, resolve_agent_checked};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct DogearData {
    pub changed: bool,
    pub record: LogEvent,
}

pub fn run(
    args: DogearArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let resolved = store::discover(file)?;
    let text = read_text(args.text, "dogear", "dogear")?;
    let home = store::home_dir(&resolved.cwd);
    let text = rewrite_home_paths(&text, home.as_deref());
    validate_text(&text, "dogear")?;
    let (agent, source) = resolve_agent_checked(args.agent, true)?;
    let mut tags = args.tags;
    tags.sort();
    tags.dedup();
    let ts = format_timestamp(now);
    let record = LogEvent::Dogear {
        id: compute_dogear_id(&ts, &agent, &text, &tags),
        ts,
        agent,
        text,
        tags,
        evidence: args
            .evidence
            .as_deref()
            .map(|value| redact_evidence(value, home.as_deref())),
        cwd: store::record_cwd(&resolved.cwd, resolved.cwd_repo(), home.as_deref()),
    };
    let (changed, record) = store::append_unique(&resolved.path, record, args.dry_run)?;
    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.agent_source = Some(source.into());
    meta.warnings = resolved.warnings.clone();
    if args.dry_run {
        meta.warnings.push("dry run; no record appended".into());
    } else if !changed {
        meta.warnings
            .push("duplicate dogear; existing record returned".into());
    }
    output::write_success(DogearData { changed, record }, pretty, meta)
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(0)
}
