use crate::cli::VerifyArgs;
use crate::commands::triage::{self, Candidate};
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{ItemStatus, ListItem};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyData {
    pub recurrences: Vec<Recurrence>,
    pub count: usize,
    pub distinct_recurring_cuts: usize,
    pub scanned: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Recurrence {
    pub resolved_id: String,
    pub resolved_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub resolution: VerifyResolution,
    pub recurrence_ids: Vec<String>,
    pub count: usize,
    pub first_recurrence_ts: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyResolution {
    pub ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

struct ResolvedAnchor {
    candidate: Candidate,
    resolution_timestamp: Timestamp,
}

pub(crate) struct RecurrenceGroup {
    pub(crate) anchor: Candidate,
    pub(crate) members: Vec<Candidate>,
}

pub(crate) struct RecurrenceAnalysis {
    pub(crate) recurrences: Vec<RecurrenceGroup>,
    pub(crate) scanned: usize,
}

struct OrderedRecurrence {
    data: RecurrenceGroup,
    first_recurrence_timestamp: Timestamp,
}

fn is_verify_eligible(item: &ListItem) -> bool {
    if item.kind != "cut" {
        return false;
    }

    match item.status {
        ItemStatus::Open => true,
        ItemStatus::Resolved => {
            let resolution = item
                .resolution
                .as_ref()
                .expect("resolved folded items have a resolution");
            !resolution.dropped && !triage::normalized_title(&item.text).is_empty()
        }
    }
}

pub fn run(_args: VerifyArgs, file: Option<PathBuf>, pretty: bool) -> AppResult<i32> {
    let resolved = store::discover(file)?;
    let store::LoadedFold { items, warnings } = store::load_folded(&resolved)?;

    let data = verify(items);
    let exit = i32::from(!data.recurrences.is_empty());
    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.warnings = warnings;
    output::write_success(data, pretty, meta)
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(exit)
}

fn verify(items: Vec<ListItem>) -> VerifyData {
    let analysis = recurrence_groups(items);
    let recurrences: Vec<_> = analysis
        .recurrences
        .iter()
        .map(materialize_recurrence)
        .collect();

    // r16 makes every eligible resolved cut an independent anchor, so one open
    // cut recurs once against each anchor it resembles. `count` is therefore
    // the number of historical cuts that came back, and this is the number of
    // live ones.
    let distinct_recurring_cuts = recurrences
        .iter()
        .flat_map(|recurrence| recurrence.recurrence_ids.iter().map(String::as_str))
        .collect::<BTreeSet<_>>()
        .len();

    VerifyData {
        count: recurrences.len(),
        distinct_recurring_cuts,
        recurrences,
        scanned: analysis.scanned,
    }
}

pub(crate) fn recurrence_groups(items: Vec<ListItem>) -> RecurrenceAnalysis {
    let mut open = Vec::new();
    let mut anchors = Vec::new();

    for item in items {
        if !is_verify_eligible(&item) {
            continue;
        }

        let normalized_title = triage::normalized_title(&item.text);
        let candidate = Candidate {
            timestamp: item
                .ts
                .parse()
                .expect("folded items have valid RFC3339 timestamps"),
            tags: item.tags.iter().cloned().collect(),
            tokens: triage::scoring_tokens(&normalized_title),
            normalized_title,
            item,
        };

        match candidate.item.status {
            ItemStatus::Open => open.push(candidate),
            ItemStatus::Resolved => {
                let resolution = candidate
                    .item
                    .resolution
                    .as_ref()
                    .expect("resolved folded items have a resolution");
                anchors.push(ResolvedAnchor {
                    resolution_timestamp: resolution
                        .ts
                        .parse()
                        .expect("folded resolutions have valid RFC3339 timestamps"),
                    candidate,
                });
            }
        }
    }

    open.sort_by(triage::candidate_order);
    let scanned = open.len();
    let frequencies = triage::corpus_frequencies(
        open.iter()
            .chain(anchors.iter().map(|anchor| &anchor.candidate)),
    );
    // The prefilter indexes the open cuts, but the frequencies stay the
    // open-plus-anchor counts computed above. Rebuilding them over `open` alone
    // would change document-frequency rarity — and so `linked`'s verdict — and
    // would panic on an anchor token that no open cut carries, because the
    // representative scored here is an anchor.
    let index = triage::CandidateIndex::new(&open, &frequencies);
    let mut scratch = triage::CandidateScratch::new(scanned);
    let mut recurrences = Vec::new();
    for anchor in anchors {
        // `open` is sorted by (timestamp, id) and the prefilter returns
        // positions into it, so the post-resolution cutoff is a floor on the
        // bitset walk rather than a second pass. Triage's
        // `candidate <= representative` self-exclusion has no counterpart: an
        // anchor is never a member of `open`.
        let floor =
            open.partition_point(|candidate| candidate.timestamp <= anchor.resolution_timestamp);
        let recurring: Vec<_> = scratch
            .matching_candidates(&anchor.candidate, &index, &frequencies, floor)
            .indices_from(floor)
            .filter(|&candidate| triage::linked(&anchor.candidate, &open[candidate], &frequencies))
            .map(|candidate| open[candidate].clone())
            .collect();
        let Some(first) = recurring.first() else {
            continue;
        };
        recurrences.push(OrderedRecurrence {
            first_recurrence_timestamp: first.timestamp,
            data: RecurrenceGroup {
                anchor: anchor.candidate,
                members: recurring,
            },
        });
    }
    recurrences.sort_by(|left, right| {
        left.first_recurrence_timestamp
            .cmp(&right.first_recurrence_timestamp)
            .then_with(|| left.data.anchor.item.id.cmp(&right.data.anchor.item.id))
    });

    RecurrenceAnalysis {
        recurrences: recurrences
            .into_iter()
            .map(|recurrence| recurrence.data)
            .collect(),
        scanned,
    }
}

fn materialize_recurrence(group: &RecurrenceGroup) -> Recurrence {
    let resolution = group
        .anchor
        .item
        .resolution
        .as_ref()
        .expect("resolved anchors have a resolution");
    let first = group
        .members
        .first()
        .expect("recurrence groups have members");

    Recurrence {
        resolved_id: group.anchor.item.id.clone(),
        resolved_text: group.anchor.item.text.clone(),
        source: group.anchor.item.source.clone(),
        resolution: VerifyResolution {
            ts: resolution.ts.clone(),
            task: resolution.task.clone(),
            pr: resolution.pr.clone(),
            commit: resolution.commit.clone(),
        },
        recurrence_ids: group
            .members
            .iter()
            .map(|candidate| candidate.item.id.clone())
            .collect(),
        count: group.members.len(),
        first_recurrence_ts: first.item.ts.clone(),
    }
}
