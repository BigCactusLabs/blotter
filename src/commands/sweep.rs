use crate::cli::{ListKind, SweepArgs};
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{ItemStatus, ListItem, parse_since};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct SweepData {
    pub repos: Vec<SweepRepo>,
    pub totals: SweepTotals,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SweepRepo {
    pub path: String,
    pub counts: SweepCounts,
    pub by_tag: Vec<TagCount>,
    pub items: Vec<ListItem>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SweepCounts {
    pub open_cuts: usize,
    pub open_dogears: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TagCount {
    pub tag: String,
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SweepTotals {
    pub repos_swept: usize,
    pub repos_skipped: usize,
    pub open_cuts: usize,
    pub open_dogears: usize,
}

pub fn run(args: SweepArgs, file: Option<PathBuf>, pretty: bool, now: Timestamp) -> AppResult<i32> {
    if file.is_some() {
        return Err(AppError::invalid_argument(
            "--file conflicts with sweep",
            "List repository paths directly or use --registry FILE.",
        ));
    }
    let since = args
        .since
        .as_deref()
        .map(|value| parse_since(value, now))
        .transpose()?;
    let inputs = input_paths(&args)?;
    if inputs.is_empty() {
        return Err(AppError::invalid_argument(
            "nothing to sweep",
            "Pass one or more repository paths or --registry FILE.",
        ));
    }

    let mut warnings = Vec::new();
    let mut paths = BTreeSet::new();
    let mut repos_skipped = 0;
    for input in inputs {
        match resolve_log_path(&input) {
            Ok(path) => {
                paths.insert(path);
            }
            Err(reason) => {
                repos_skipped += 1;
                warnings.push(format!("skipped {}: {reason}", input.display()));
            }
        }
    }

    let mut repos = Vec::new();
    let mut totals = SweepTotals {
        repos_swept: 0,
        repos_skipped: 0,
        open_cuts: 0,
        open_dogears: 0,
    };
    for path in paths {
        match store::with_shared(&path, |file| {
            let bytes = store::read_bytes(file, &path)?;
            Ok(store::fold_bytes(&bytes))
        }) {
            Ok(folded) => {
                for warning in folded.warnings {
                    warnings.push(format!("{}: {warning}", path.display()));
                }
                let repo = sweep_repo(path, folded.items, args.kind, since);
                totals.repos_swept += 1;
                totals.open_cuts += repo.counts.open_cuts;
                totals.open_dogears += repo.counts.open_dogears;
                repos.push(repo);
            }
            Err(error) => {
                repos_skipped += 1;
                let reason = if error.code == "lock_timeout" {
                    "lock timeout (retryable)".into()
                } else {
                    error.message
                };
                warnings.push(format!("skipped {}: {reason}", path.display()));
            }
        }
    }
    totals.repos_skipped = repos_skipped;
    let mut meta = Meta::new();
    meta.warnings = warnings;
    output::write_success(SweepData { repos, totals }, pretty, meta)
        .map_err(|error| AppError::from_io(error, Path::new("stdout")))?;
    Ok(0)
}

fn input_paths(args: &SweepArgs) -> AppResult<Vec<PathBuf>> {
    let mut paths = args.paths.clone();
    if let Some(registry) = args.registry.as_deref() {
        paths.extend(read_registry(registry)?);
    }
    Ok(paths)
}

fn read_registry(registry: &Path) -> AppResult<Vec<PathBuf>> {
    let registry = fs::canonicalize(registry)
        .map_err(|error| AppError::from_registry_file(error, registry))?;
    let contents = fs::read_to_string(&registry)
        .map_err(|error| AppError::from_registry_file(error, &registry))?;
    let directory = registry.parent().unwrap_or(Path::new("."));
    Ok(contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                directory.join(path)
            }
        })
        .collect())
}

fn resolve_log_path(input: &Path) -> Result<PathBuf, String> {
    let input = fs::canonicalize(input).map_err(|error| format!("cannot resolve path: {error}"))?;
    let metadata = fs::metadata(&input).map_err(|error| format!("cannot inspect path: {error}"))?;
    let log = if metadata.is_dir() {
        let root =
            store::find_repo_root(&input).ok_or_else(|| "not a repository directory".to_owned())?;
        store::default_log_path(&root)
    } else if metadata.is_file() {
        input
    } else {
        return Err("must be a repository directory or regular JSONL file".into());
    };
    fs::canonicalize(&log).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "blotter file does not exist".into()
        } else {
            format!("cannot resolve blotter file: {error}")
        }
    })
}

fn sweep_repo(
    path: PathBuf,
    items: Vec<ListItem>,
    kind: ListKind,
    since: Option<Timestamp>,
) -> SweepRepo {
    let counts = SweepCounts {
        open_cuts: items
            .iter()
            .filter(|item| item.kind == "cut" && item.status == ItemStatus::Open)
            .count(),
        open_dogears: items
            .iter()
            .filter(|item| item.kind == "dogear" && item.status == ItemStatus::Open)
            .count(),
    };
    let items: Vec<_> = items
        .into_iter()
        .filter(|item| item.status == ItemStatus::Open)
        .filter(|item| matches_kind(item, kind))
        .filter(|item| {
            since.is_none_or(|threshold| {
                item.ts
                    .parse::<Timestamp>()
                    .is_ok_and(|timestamp| timestamp >= threshold)
            })
        })
        .collect();
    let by_tag = tag_counts(&items);
    let truncated = items.len() > 50;

    SweepRepo {
        path: path.to_string_lossy().into_owned(),
        counts,
        by_tag,
        items: items.into_iter().take(50).collect(),
        truncated,
    }
}

fn matches_kind(item: &ListItem, kind: ListKind) -> bool {
    match kind {
        ListKind::Cut => item.kind == "cut",
        ListKind::Dogear => item.kind == "dogear",
        ListKind::All => true,
    }
}

fn tag_counts(items: &[ListItem]) -> Vec<TagCount> {
    let mut tags = BTreeMap::<String, usize>::new();
    for item in items {
        if item.tags.is_empty() {
            *tags.entry(String::new()).or_default() += 1;
        } else {
            for tag in &item.tags {
                *tags.entry(tag.clone()).or_default() += 1;
            }
        }
    }
    let mut tags: Vec<_> = tags
        .into_iter()
        .map(|(tag, count)| TagCount { tag, count })
        .collect();
    tags.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.tag.cmp(&right.tag))
    });
    tags
}
