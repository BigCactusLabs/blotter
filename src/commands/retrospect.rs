use crate::cli::RetrospectArgs;
use crate::commands::triage::{self, Candidate, ChronicCluster};
use crate::commands::verify::{self, RecurrenceGroup};
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The first program word of an evidence command, after leading `VAR=value` assignments and
/// reduced to its basename. Best-effort: this does not parse the shell.
fn leading_program(command: &str) -> Option<&str> {
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

fn is_environment_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(byte) if byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

const MAX_EVIDENCE_TEXTS: usize = 10;
const MAX_RESOLUTION_NOTES: usize = 5;

#[derive(Debug, Serialize, Deserialize)]
pub struct RetrospectData {
    pub candidates: Vec<RetrospectCandidate>,
    pub count: usize,
    pub scanned: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RetrospectCandidate {
    pub pattern: crate::Pattern,
    pub suggested: Vec<crate::ArtifactType>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    pub record_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_anchor_ids: Option<Vec<String>>,
    pub occurrences: usize,
    pub first_ts: String,
    pub last_ts: String,
    pub evidence: RetrospectEvidence,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RetrospectEvidence {
    pub texts: Vec<String>,
    pub resolution_notes: Vec<String>,
}

struct OrderedCandidate {
    data: RetrospectCandidate,
    first_timestamp: Timestamp,
}

pub fn run(_args: RetrospectArgs, file: Option<PathBuf>, pretty: bool) -> AppResult<i32> {
    let resolved = store::discover(file)?;
    let store::LoadedFold {
        items, warnings, ..
    } = store::load_folded(&resolved)?;
    let data = retrospect(items);
    let exit = i32::from(!data.candidates.is_empty());
    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.warnings = warnings;
    output::write_success(data, pretty, meta)
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(exit)
}

fn retrospect(items: Vec<crate::ListItem>) -> RetrospectData {
    // Retrospect's open-cut candidates use the same representative algorithm
    // as triage, with the task's candidate threshold of two linked records.
    let chronic = triage::chronic_clusters(items.clone(), 2);
    let recurrences = verify::recurrence_groups(items);
    debug_assert_eq!(chronic.scanned, recurrences.scanned);

    let mut candidates: Vec<_> = chronic
        .clusters
        .iter()
        .filter_map(cluster_candidate)
        .chain(recurrences.recurrences.iter().filter_map(skill_candidate))
        .collect();
    candidates.sort_by(|left, right| {
        // Same comparator as before the pattern/suggested rename (r48): the
        // tie-break that used to run on `candidate_type` now runs on
        // `pattern` then `suggested`, so a fixture whose old type collapsed
        // two candidates onto one tie-break value can now separate them
        // (or vice versa) purely from the renamed axis, never from new data.
        left.first_timestamp
            .cmp(&right.first_timestamp)
            .then_with(|| left.data.title.cmp(&right.data.title))
            .then_with(|| (left.data.pattern as u8).cmp(&(right.data.pattern as u8)))
            .then_with(|| {
                left.data
                    .suggested
                    .iter()
                    .map(|artifact| artifact.as_str())
                    .cmp(
                        right
                            .data
                            .suggested
                            .iter()
                            .map(|artifact| artifact.as_str()),
                    )
            })
            .then_with(|| left.data.record_ids.cmp(&right.data.record_ids))
            .then_with(|| {
                left.data
                    .resolved_anchor_ids
                    .cmp(&right.data.resolved_anchor_ids)
            })
    });
    let candidates: Vec<_> = candidates
        .into_iter()
        .map(|candidate| candidate.data)
        .collect();

    RetrospectData {
        count: candidates.len(),
        candidates,
        scanned: chronic.scanned,
    }
}

fn cluster_candidate(cluster: &ChronicCluster) -> Option<OrderedCandidate> {
    let members = &cluster.members;
    let program = shared_program(members);
    // r48: the shared-failing-program rule takes precedence over the
    // docs-tag rule (first match wins); it now decides `suggested`, not
    // `pattern` — both mappings land on the same recurrent_friction pattern.
    let suggested: Vec<crate::ArtifactType> = if program.is_some() {
        vec![crate::ArtifactType::Tool, crate::ArtifactType::Guard]
    } else if docs_member_count(members) >= members.len().div_ceil(2) {
        vec![crate::ArtifactType::Doc]
    } else {
        return None;
    };
    let first = members.first().expect("chronic clusters have members");
    let last = members.last().expect("chronic clusters have members");

    Some(OrderedCandidate {
        first_timestamp: first.timestamp,
        data: RetrospectCandidate {
            pattern: crate::Pattern::RecurrentFriction,
            suggested,
            // Triage's first member is the stable representative. Its raw
            // text is the promotion title, matching triage's cluster text
            // and the resolved-anchor titles below.
            title: first.item.text.clone(),
            program,
            record_ids: members
                .iter()
                .map(|member| member.item.id.clone())
                .collect(),
            resolved_anchor_ids: None,
            occurrences: distinct_title_occurrences(cluster),
            first_ts: first.item.ts.clone(),
            last_ts: last.item.ts.clone(),
            evidence: RetrospectEvidence {
                texts: members
                    .iter()
                    .take(MAX_EVIDENCE_TEXTS)
                    .map(|member| member.item.text.clone())
                    .collect(),
                resolution_notes: Vec::new(),
            },
        },
    })
}

/// Members carry the *global* occurrence count of their normalized title, so
/// members sharing a title repeat the same count. Sum each distinct title once.
fn distinct_title_occurrences(cluster: &ChronicCluster) -> usize {
    let mut seen = BTreeSet::new();
    cluster
        .members
        .iter()
        .zip(&cluster.member_occurrences)
        .filter(|(member, _)| seen.insert(member.normalized_title.as_str()))
        .map(|(_, occurrences)| *occurrences)
        .sum()
}

fn shared_program(members: &[Candidate]) -> Option<String> {
    let mut programs = BTreeMap::<String, usize>::new();
    for member in members {
        let Some(command) = member
            .item
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.get("cmd"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(program) = leading_program(command) else {
            continue;
        };
        *programs.entry(program.into()).or_default() += 1;
    }

    let threshold = members.len().div_ceil(2);
    let mut eligible: Vec<_> = programs
        .into_iter()
        .filter(|(_, count)| *count >= threshold)
        .collect();
    // A command-bearing cluster can have two programs tied at half its
    // membership. Prefer the most frequent program, then a lexical tie-break,
    // so the one emitted value stays deterministic.
    eligible.sort_by(|(left_program, left_count), (right_program, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_program.cmp(right_program))
    });
    eligible.into_iter().next().map(|(program, _)| program)
}

fn docs_member_count(members: &[Candidate]) -> usize {
    members
        .iter()
        .filter(|member| {
            member
                .item
                .tags
                .iter()
                .any(|tag| matches!(tag.as_str(), "docs" | "documentation"))
        })
        .count()
}

fn skill_candidate(group: &RecurrenceGroup) -> Option<OrderedCandidate> {
    if group.members.len() < 2 {
        return None;
    }
    let first = group
        .members
        .first()
        .expect("recurrence groups have members");
    let last = group
        .members
        .last()
        .expect("recurrence groups have members");
    let resolution = group
        .anchor
        .item
        .resolution
        .as_ref()
        .expect("resolved anchors have a resolution");

    Some(OrderedCandidate {
        first_timestamp: first.timestamp,
        data: RetrospectCandidate {
            pattern: crate::Pattern::FailedIntervention,
            suggested: vec![crate::ArtifactType::Skill],
            title: group.anchor.item.text.clone(),
            program: None,
            record_ids: group
                .members
                .iter()
                .map(|member| member.item.id.clone())
                .collect(),
            resolved_anchor_ids: Some(vec![group.anchor.item.id.clone()]),
            occurrences: group.members.len(),
            first_ts: first.item.ts.clone(),
            last_ts: last.item.ts.clone(),
            evidence: RetrospectEvidence {
                texts: group
                    .members
                    .iter()
                    .take(MAX_EVIDENCE_TEXTS)
                    .map(|member| member.item.text.clone())
                    .collect(),
                resolution_notes: resolution
                    .note
                    .clone()
                    .into_iter()
                    .take(MAX_RESOLUTION_NOTES)
                    .collect(),
            },
        },
    })
}
