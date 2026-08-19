pub mod cli;
pub mod commands;
pub mod error;
pub mod output;
pub(crate) mod redact;
pub mod store;

use crate::error::{AppError, AppResult};
use jiff::{SignedDuration, Timestamp, Unit};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Minor,
    Major,
    Blocker,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minor => "minor",
            Self::Major => "major",
            Self::Blocker => "blocker",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Minor => 0,
            Self::Major => 1,
            Self::Blocker => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LogEvent {
    Cut {
        id: String,
        ts: String,
        agent: String,
        text: String,
        tags: Vec<String>,
        severity: Severity,
        cwd: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        evidence: Option<Evidence>,
    },
    Dogear {
        id: String,
        ts: String,
        agent: String,
        text: String,
        tags: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        evidence: Option<String>,
        cwd: String,
    },
    Resolve {
        id: String,
        ts: String,
        agent: String,
        note: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pr: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        commit: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        dropped: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        amend: bool,
    },
    #[serde(other)]
    Unknown,
}

impl LogEvent {
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Cut { id, .. } | Self::Dogear { id, .. } | Self::Resolve { id, .. } => Some(id),
            Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub ts: String,
    pub agent: String,
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dropped: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub amended: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    pub kind: String,
    pub id: String,
    pub ts: String,
    pub agent: String,
    pub text: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<serde_json::Value>,
    pub status: ItemStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<Resolution>,
}

impl ListItem {
    pub(crate) fn from_record(event: LogEvent, resolution: Option<Resolution>) -> Self {
        let status = if resolution.is_some() {
            ItemStatus::Resolved
        } else {
            ItemStatus::Open
        };
        match event {
            LogEvent::Cut {
                id,
                ts,
                agent,
                text,
                tags,
                severity,
                cwd,
                source,
                evidence,
            } => Self {
                kind: "cut".into(),
                id,
                ts,
                agent,
                text,
                tags,
                severity: Some(severity),
                cwd,
                source,
                evidence: evidence
                    .map(|evidence| serde_json::to_value(evidence).expect("evidence serializes")),
                status,
                resolution,
            },
            LogEvent::Dogear {
                id,
                ts,
                agent,
                text,
                tags,
                evidence,
                cwd,
            } => Self {
                kind: "dogear".into(),
                id,
                ts,
                agent,
                text,
                tags,
                severity: None,
                cwd,
                source: None,
                evidence: evidence.map(serde_json::Value::String),
                status,
                resolution,
            },
            LogEvent::Resolve { .. } | LogEvent::Unknown => {
                unreachable!("folded records are cut or dogear")
            }
        }
    }
}

pub fn is_auto_capture(tags: &[String]) -> bool {
    tags.iter().any(|tag| tag == "auto")
}

pub(crate) fn partition_auto_captures(
    items: Vec<ListItem>,
    include_auto: bool,
) -> (Vec<ListItem>, Vec<ListItem>) {
    if include_auto {
        (items, Vec::new())
    } else {
        items
            .into_iter()
            .partition(|item| !is_auto_capture(&item.tags))
    }
}

pub(crate) fn auto_capture_warning(count: usize) -> String {
    let noun = if count == 1 { "record" } else { "records" };
    format!("{count} auto-captured {noun} hidden; use --include-auto to include them")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    Open,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdNamespace {
    Bl,
    Pc,
}

pub(crate) fn id_namespace(id: &str) -> Option<IdNamespace> {
    match id.get(..3) {
        Some(prefix) if prefix.eq_ignore_ascii_case("bl_") => Some(IdNamespace::Bl),
        Some(prefix) if prefix.eq_ignore_ascii_case("pc_") => Some(IdNamespace::Pc),
        _ => None,
    }
}

pub fn effective_now() -> AppResult<Timestamp> {
    let timestamp = match std::env::var("BLOTTER_NOW") {
        Ok(value) if !value.is_empty() => value.parse::<Timestamp>().map_err(|_| {
            AppError::config(
                "BLOTTER_NOW must be a full RFC3339 timestamp",
                "Set BLOTTER_NOW to a value like 2026-07-09T18:30:00Z or unset it.",
            )
        })?,
        Ok(_) | Err(std::env::VarError::NotPresent) => Timestamp::now(),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(AppError::config(
                "BLOTTER_NOW is not valid UTF-8",
                "Set BLOTTER_NOW to a full RFC3339 timestamp or unset it.",
            ));
        }
    };
    timestamp
        .round(Unit::Millisecond)
        .map_err(|error| AppError::internal(error.to_string()))
}

pub fn format_timestamp(timestamp: Timestamp) -> String {
    format!("{timestamp:.3}")
}

pub fn parse_since(value: &str, now: Timestamp) -> AppResult<Timestamp> {
    parse_cutoff("--since", value, now)
}

pub fn parse_before(value: &str, now: Timestamp) -> AppResult<Timestamp> {
    parse_cutoff("--before", value, now)
}

fn parse_cutoff(flag_name: &str, value: &str, now: Timestamp) -> AppResult<Timestamp> {
    let is_since = flag_name == "--since";
    let relative_suggested_fix = || {
        if is_since {
            "Use a full RFC3339 timestamp, Nd, or Nh.".to_owned()
        } else {
            format!("Use {flag_name} with a full RFC3339 timestamp, Nd, or Nh.")
        }
    };
    let smaller_duration_suggested_fix = || {
        if is_since {
            "Use a smaller Nd or Nh duration.".to_owned()
        } else {
            format!("Use a smaller relative value for {flag_name}.")
        }
    };
    let absolute_suggested_fix = || {
        if is_since {
            "Use a full RFC3339 timestamp such as 2026-07-09T18:30:00Z, or a relative value such as 7d or 12h.".to_owned()
        } else {
            format!(
                "Use {flag_name} with a full RFC3339 timestamp such as 2026-07-09T18:30:00Z, or a relative value such as 7d or 12h."
            )
        }
    };
    if let Some((number, unit)) = value.split_at_checked(value.len().saturating_sub(1))
        && !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && matches!(unit, "d" | "h")
    {
        let amount = number.parse::<i64>().map_err(|_| {
            AppError::invalid_argument(
                format!("invalid {flag_name} value '{value}'"),
                relative_suggested_fix(),
            )
        })?;
        let duration = if unit == "d" {
            amount.checked_mul(24)
        } else {
            Some(amount)
        }
        .and_then(SignedDuration::try_from_hours)
        .ok_or_else(|| {
            AppError::invalid_argument(
                format!("{flag_name} value '{value}' is too large"),
                smaller_duration_suggested_fix(),
            )
        })?;
        return now.checked_sub(duration).map_err(|_| {
            AppError::invalid_argument(
                format!("{flag_name} value '{value}' is outside the supported range"),
                smaller_duration_suggested_fix(),
            )
        });
    }

    value.parse::<Timestamp>().map_err(|_| {
        AppError::invalid_argument(
            format!("invalid {flag_name} value '{value}'"),
            absolute_suggested_fix(),
        )
    })
}

pub fn compute_id(
    ts: &str,
    agent: &str,
    text: &str,
    severity: Severity,
    tags: &[String],
) -> String {
    let mut tags = tags.to_vec();
    tags.sort();
    tags.dedup();
    let count = tags.len().to_string();
    let mut fields: Vec<&str> = vec![
        "bl1",
        "cut",
        ts,
        agent,
        text,
        severity.as_str(),
        count.as_str(),
    ];
    fields.extend(tags.iter().map(String::as_str));
    compute_id_fields_bytes(&fields, 6)
}

pub fn compute_dogear_id(ts: &str, agent: &str, text: &str, tags: &[String]) -> String {
    let mut tags = tags.to_vec();
    tags.sort();
    tags.dedup();
    let count = tags.len().to_string();
    // v1 dogear identity: a version literal and the kind provide domain
    // separation, and every tag is its own length-prefixed field (TupleHash
    // style) so tag-set boundaries cannot collide (`["a","b"]` != `["a,b"]`).
    // 80-bit digest; cut IDs use the matching framed scheme at 48 bits.
    let mut fields: Vec<&str> = vec!["bl1", "dogear", ts, agent, text, count.as_str()];
    fields.extend(tags.iter().map(String::as_str));
    compute_id_fields_bytes(&fields, 10)
}

fn compute_id_fields_bytes(fields: &[&str], bytes: usize) -> String {
    let mut hash = Sha256::new();
    for field in fields {
        hash.update((field.len() as u32).to_le_bytes());
        hash.update(field.as_bytes());
    }
    let digest = hash.finalize();
    let mut id = String::with_capacity(3 + bytes * 2);
    id.push_str("bl_");
    for byte in &digest[..bytes] {
        write!(&mut id, "{byte:02x}").expect("writing to a String cannot fail");
    }
    id
}

pub fn resolve_agent(flag: Option<String>) -> (String, &'static str) {
    if let Some(agent) = flag.filter(|value| !value.is_empty()) {
        return (agent, "flag");
    }
    if let Ok(agent) = std::env::var("BLOTTER_AGENT")
        && !agent.is_empty()
    {
        return (agent, "env");
    }
    if std::env::var_os("CLAUDECODE").is_some() {
        return ("claude-code".into(), "detected");
    }
    if std::env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CODEX_")) {
        return ("codex".into(), "detected");
    }
    if std::env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CURSOR_")) {
        return ("cursor".into(), "detected");
    }
    ("unknown".into(), "default")
}

pub(crate) fn resolve_agent_checked(
    flag: Option<String>,
    reject_resolved_whitespace: bool,
) -> AppResult<(String, &'static str)> {
    if flag.as_deref().is_some_and(|agent| agent.trim().is_empty()) {
        return Err(AppError::invalid_input(
            "agent name cannot be empty or whitespace-only",
            "Pass a non-empty --agent NAME or omit the flag.",
        ));
    }
    let (agent, source) = resolve_agent(flag);
    if reject_resolved_whitespace && agent.trim().is_empty() {
        return Err(AppError::invalid_input(
            "agent name cannot be whitespace-only",
            "Pass a non-empty --agent NAME or set BLOTTER_AGENT.",
        ));
    }
    Ok((agent, source))
}
