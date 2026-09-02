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
pub enum Impact {
    Low,
    Material,
    Blocking,
}

impl Impact {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Material => "material",
            Self::Blocking => "blocking",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Material => 1,
            Self::Blocking => 2,
        }
    }
}

/// How a cut's resolution classifies it (r48). Cuts always carry one; dogears
/// never do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Disposition {
    Fixed,
    Promoted,
    Accepted,
    Invalid,
}

/// The two `retrospect` candidate patterns (r48/r51). Pattern detection and
/// suggested intervention are separate axes: a pattern names what was
/// observed, `suggested` names what kind of artifact might answer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    RecurrentFriction,
    FailedIntervention,
}

/// The closed promotion artifact vocabulary (r48). An unrecognized
/// `--artifact-type` is rejected by clap as `invalid_argument`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactType {
    Doc,
    Skill,
    Guard,
    Test,
    Tool,
    Process,
}

impl ArtifactType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Doc => "doc",
            Self::Skill => "skill",
            Self::Guard => "guard",
            Self::Test => "test",
            Self::Tool => "tool",
            Self::Process => "process",
        }
    }
}

/// What a promotion says these experiences became.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    #[serde(rename = "type")]
    pub kind: ArtifactType,
    #[serde(rename = "ref")]
    pub r#ref: String,
}

/// Structured provenance (r49). A typed three-member struct, never flattened
/// and never an opaque `Value`: a `null` published member reads as absent, a
/// non-string one fails the line, and an unknown member survives in the log's
/// bytes without reaching any envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
}

impl Origin {
    /// The only origin any 1.0.0 command writes.
    pub fn agent() -> Self {
        Self {
            kind: "agent".into(),
            provider: None,
            r#ref: None,
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
        impact: Impact,
        cwd: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin: Option<Origin>,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin: Option<Origin>,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        disposition: Option<Disposition>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        disposition_ts: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        promotion: Option<String>,
    },
    Promotion {
        id: String,
        ts: String,
        agent: String,
        sources: Vec<String>,
        artifact: Artifact,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin: Option<Origin>,
        cwd: String,
    },
    #[serde(other)]
    Unknown,
}

impl LogEvent {
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Cut { id, .. }
            | Self::Dogear { id, .. }
            | Self::Resolve { id, .. }
            | Self::Promotion { id, .. } => Some(id),
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<Disposition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition_ts: Option<String>,
    /// The promotion this resolution links to (r48). Present only under
    /// `disposition: promoted`, and only when the link is mutual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion: Option<String>,
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
    pub impact: Option<Impact>,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
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
                impact,
                cwd,
                origin,
                evidence,
            } => Self {
                kind: "cut".into(),
                id,
                ts,
                agent,
                text,
                tags,
                impact: Some(impact),
                cwd,
                origin,
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
                origin,
            } => Self {
                kind: "dogear".into(),
                id,
                ts,
                agent,
                text,
                tags,
                impact: None,
                cwd,
                origin,
                evidence: evidence.map(serde_json::Value::String),
                status,
                resolution,
            },
            LogEvent::Resolve { .. } | LogEvent::Promotion { .. } | LogEvent::Unknown => {
                unreachable!("folded list items are cut or dogear")
            }
        }
    }
}

/// The promotion arm of `list`'s tagged union (r48). It carries no `status`,
/// `resolution`, `text`, `tags`, `impact`, or `evidence`: a promotion has no
/// lifecycle to filter and no friction text of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionItem {
    pub kind: String,
    pub id: String,
    pub ts: String,
    pub agent: String,
    pub sources: Vec<String>,
    pub artifact: Artifact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
}

impl PromotionItem {
    pub(crate) fn from_record(event: LogEvent) -> Self {
        let LogEvent::Promotion {
            id,
            ts,
            agent,
            sources,
            artifact,
            note,
            origin,
            cwd,
        } = event
        else {
            unreachable!("only promotion records become promotion items")
        };
        Self {
            kind: "promotion".into(),
            id,
            ts,
            agent,
            sources,
            artifact,
            note,
            cwd,
            origin,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    Open,
    Resolved,
}

/// One ID namespace under `bl2` (r48): every record kind is `bl_` plus lowercase
/// hex, matched case-insensitively. `pc_` is gone, so this is a plain prefix
/// predicate rather than a namespace enum.
pub(crate) fn is_bl_id(id: &str) -> bool {
    id.get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bl_"))
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

pub fn compute_id(ts: &str, agent: &str, text: &str, impact: Impact, tags: &[String]) -> String {
    let mut tags = tags.to_vec();
    tags.sort();
    tags.dedup();
    let count = tags.len().to_string();
    let mut fields: Vec<&str> = vec![
        "bl2",
        "cut",
        ts,
        agent,
        text,
        impact.as_str(),
        count.as_str(),
    ];
    fields.extend(tags.iter().map(String::as_str));
    compute_id_fields_bytes(&fields, 10)
}

pub fn compute_dogear_id(ts: &str, agent: &str, text: &str, tags: &[String]) -> String {
    let mut tags = tags.to_vec();
    tags.sort();
    tags.dedup();
    let count = tags.len().to_string();
    // bl2 dogear identity: the domain literal and the kind field provide domain
    // separation, and every tag is its own length-prefixed field (TupleHash
    // style) so tag-set boundaries cannot collide (`["a","b"]` != `["a,b"]`).
    // 80-bit digest; every v2 identity is the same width (r51).
    let mut fields: Vec<&str> = vec!["bl2", "dogear", ts, agent, text, count.as_str()];
    fields.extend(tags.iter().map(String::as_str));
    compute_id_fields_bytes(&fields, 10)
}

/// The `bl2` promotion identity (r48, r51): `note` stays outside the hash for
/// r34's reason — it is authored commentary, and a promotion whose note is
/// reworded is the same promotion. `sources` are normalized exactly as tags are:
/// ascending raw UTF-8 byte order, exact-byte deduplication, case-sensitive.
pub fn compute_promotion_id(
    ts: &str,
    agent: &str,
    sources: &[String],
    artifact_type: &str,
    artifact_ref: &str,
) -> String {
    let sources = normalized(sources);
    let count = sources.len().to_string();
    let mut fields: Vec<&str> = vec!["bl2", "promotion", ts, agent, count.as_str()];
    fields.extend(sources.iter().map(String::as_str));
    fields.push(artifact_type);
    fields.push(artifact_ref);
    compute_id_fields_bytes(&fields, 10)
}

/// Ascending raw UTF-8 byte order with exact-byte deduplication (r48). The one
/// normalization every hashed list — tags and promotion sources — runs through.
pub fn normalized(values: &[String]) -> Vec<String> {
    let mut values = values.to_vec();
    values.sort();
    values.dedup();
    values
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

pub fn resolve_agent(flag: Option<String>) -> AppResult<(String, &'static str)> {
    if let Some(agent) = flag.filter(|value| !value.is_empty()) {
        return Ok((agent, "flag"));
    }
    match std::env::var("BLOTTER_AGENT") {
        Ok(agent) if !agent.is_empty() => return Ok((agent, "env")),
        // An unset or empty BLOTTER_AGENT falls through to detection; a
        // non-UTF-8 one names an agent this build cannot read, and silently
        // filing under a detected name would contradict the operator.
        Ok(_) | Err(std::env::VarError::NotPresent) => {}
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(AppError::config(
                "BLOTTER_AGENT is not valid UTF-8",
                "Set BLOTTER_AGENT to a UTF-8 agent name or unset it.",
            ));
        }
    }
    if std::env::var_os("CLAUDECODE").is_some() {
        return Ok(("claude-code".into(), "detected"));
    }
    if std::env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CODEX_")) {
        return Ok(("codex".into(), "detected"));
    }
    if std::env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("CURSOR_")) {
        return Ok(("cursor".into(), "detected"));
    }
    Ok(("unknown".into(), "default"))
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
    let (agent, source) = resolve_agent(flag)?;
    if reject_resolved_whitespace && agent.trim().is_empty() {
        return Err(AppError::invalid_input(
            "agent name cannot be whitespace-only",
            "Pass a non-empty --agent NAME or set BLOTTER_AGENT.",
        ));
    }
    Ok((agent, source))
}
