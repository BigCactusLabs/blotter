use crate::cli::TriageArgs;
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{ItemStatus, ListItem};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct TriageData {
    pub clusters: Vec<TriageCluster>,
    pub count: usize,
    pub scanned: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TriageCluster {
    pub count: usize,
    pub occurrences: usize,
    pub ids: Vec<String>,
    pub tags: Vec<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub suggested_action: String,
}

#[derive(Clone)]
pub(crate) struct Candidate {
    pub(crate) item: ListItem,
    pub(crate) timestamp: Timestamp,
    pub(crate) tags: BTreeSet<String>,
    pub(crate) normalized_title: String,
    pub(crate) tokens: BTreeSet<String>,
}

pub(crate) struct TokenFrequencies {
    counts: BTreeMap<String, usize>,
    candidate_count: usize,
}

pub(crate) struct ChronicCluster {
    pub(crate) members: Vec<Candidate>,
    pub(crate) member_occurrences: Vec<usize>,
    displayed_occurrences: usize,
}

pub(crate) struct ChronicAnalysis {
    pub(crate) clusters: Vec<ChronicCluster>,
    pub(crate) scanned: usize,
}

struct OrderedCluster {
    data: ChronicCluster,
    oldest_timestamp: Timestamp,
    first_id: String,
}

/// Candidate pools preserve the triage relation while avoiding a scan of every
/// later cut for each representative. Exact titles bypass tags, while the
/// normal scoring path can only use candidates in a shared-tag or untagged
/// pool. Token bitsets then identify the candidates that satisfy either r19
/// scoring path before `linked` provides the final contract guard.
///
/// `by_token` holds only tokens that two or more candidates share. A token in
/// exactly one candidate can never raise the shared-token count between two
/// different candidates, so indexing it would add an N-bit row that only ever
/// counts a candidate against itself — and self is dropped by the
/// `candidate <= representative` guard. Skipping those tokens keeps the index
/// proportional to the shared vocabulary instead of the whole vocabulary,
/// which is what bounds memory when every record brings new words.
struct CandidateIndex {
    by_title: BTreeMap<String, Vec<usize>>,
    by_tag: BTreeMap<String, BitSet>,
    untagged: BitSet,
    by_token: BTreeMap<String, BitSet>,
    by_token_count: BTreeMap<usize, BitSet>,
    all: BitSet,
}

impl CandidateIndex {
    fn new(candidates: &[Candidate], frequencies: &TokenFrequencies) -> Self {
        let words = candidates.len().div_ceil(64);
        let mut by_title = BTreeMap::new();
        let mut by_tag = BTreeMap::new();
        let mut untagged = BitSet::empty(words);
        let mut by_token = BTreeMap::new();
        let mut by_token_count = BTreeMap::new();

        for (index, candidate) in candidates.iter().enumerate() {
            if !candidate.normalized_title.is_empty() {
                by_title
                    .entry(candidate.normalized_title.clone())
                    .or_insert_with(Vec::new)
                    .push(index);
            }
            if candidate.tags.is_empty() {
                untagged.set(index);
            } else {
                for tag in &candidate.tags {
                    by_tag
                        .entry(tag.clone())
                        .or_insert_with(|| BitSet::empty(words))
                        .set(index);
                }
            }
            if !candidate.tokens.is_empty() {
                by_token_count
                    .entry(candidate.tokens.len())
                    .or_insert_with(|| BitSet::empty(words))
                    .set(index);
                for token in &candidate.tokens {
                    if frequencies.is_shared(token) {
                        by_token
                            .entry(token.clone())
                            .or_insert_with(|| BitSet::empty(words))
                            .set(index);
                    }
                }
            }
        }

        Self {
            by_title,
            by_tag,
            untagged,
            by_token,
            by_token_count,
            all: BitSet::full(candidates.len()),
        }
    }
}

struct BitSet {
    words: Vec<u64>,
}

impl BitSet {
    fn empty(words: usize) -> Self {
        Self {
            words: vec![0; words],
        }
    }

    fn full(bits: usize) -> Self {
        let words = bits.div_ceil(64);
        let mut result = Self {
            words: vec![u64::MAX; words],
        };
        if let Some(last) = result.words.last_mut()
            && !bits.is_multiple_of(64)
        {
            *last = (1_u64 << (bits % 64)) - 1;
        }
        result
    }

    fn clear(&mut self) {
        self.words.fill(0);
    }

    fn set(&mut self, index: usize) {
        self.words[index / 64] |= 1_u64 << (index % 64);
    }

    fn copy_from(&mut self, source: &Self) {
        self.words.copy_from_slice(&source.words);
    }

    fn or_assign(&mut self, source: &Self) {
        for (target, source) in self.words.iter_mut().zip(&source.words) {
            *target |= source;
        }
    }

    fn and_assign(&mut self, source: &Self) {
        for (target, source) in self.words.iter_mut().zip(&source.words) {
            *target &= source;
        }
    }

    fn indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.words
            .iter()
            .enumerate()
            .flat_map(|(word_index, word)| {
                let mut remaining = *word;
                std::iter::from_fn(move || {
                    if remaining == 0 {
                        return None;
                    }
                    let bit = remaining.trailing_zeros() as usize;
                    remaining &= remaining - 1;
                    Some(word_index * 64 + bit)
                })
            })
    }
}

/// A bit-sliced counter counts each representative token's candidate posting
/// set in parallel. The data remains a per-candidate document-frequency view:
/// every candidate contributes at most one bit per deduplicated token.
struct BitSlicedCounter {
    words: usize,
    planes: Vec<Vec<u64>>,
}

impl BitSlicedCounter {
    fn new(words: usize) -> Self {
        Self {
            words,
            planes: Vec::new(),
        }
    }

    fn reset(&mut self, maximum: usize) {
        let required_planes = if maximum == 0 {
            0
        } else {
            maximum.ilog2() as usize + 1
        };
        self.planes
            .resize_with(required_planes, || vec![0; self.words]);
        for plane in &mut self.planes {
            plane.fill(0);
        }
    }

    fn add(&mut self, source: &BitSet) {
        for word_index in 0..self.words {
            let mut carry = source.words[word_index];
            for plane in &mut self.planes {
                let next_carry = plane[word_index] & carry;
                plane[word_index] ^= carry;
                carry = next_carry;
            }
        }
    }

    fn add_at_least(&self, threshold: usize, allowed: &BitSet, target: &mut BitSet) {
        debug_assert!(threshold > 0);
        for word_index in 0..self.words {
            let mut equal = allowed.words[word_index];
            let mut greater = 0;
            for (bit, plane) in self.planes.iter().enumerate().rev() {
                if threshold & (1 << bit) == 0 {
                    greater |= equal & plane[word_index];
                    equal &= !plane[word_index];
                } else {
                    equal &= plane[word_index];
                }
            }
            target.words[word_index] |= greater | equal;
        }
    }
}

struct CandidateScratch {
    tag_pool: BitSet,
    matches: BitSet,
    overlap: BitSlicedCounter,
    rare: BitSlicedCounter,
}

impl CandidateScratch {
    fn new(candidate_count: usize) -> Self {
        let words = candidate_count.div_ceil(64);
        Self {
            tag_pool: BitSet::empty(words),
            matches: BitSet::empty(words),
            overlap: BitSlicedCounter::new(words),
            rare: BitSlicedCounter::new(words),
        }
    }

    fn matching_candidates<'a>(
        &'a mut self,
        representative: &Candidate,
        index: &CandidateIndex,
        frequencies: &TokenFrequencies,
    ) -> &'a BitSet {
        self.tag_pool.clear();
        if representative.tags.is_empty() {
            self.tag_pool.copy_from(&index.untagged);
        } else {
            for tag in &representative.tags {
                if let Some(pool) = index.by_tag.get(tag) {
                    self.tag_pool.or_assign(pool);
                }
            }
        }

        self.matches.clear();
        if !representative.tokens.is_empty() {
            self.overlap.reset(representative.tokens.len());
            for token in &representative.tokens {
                // An unshared token has no posting set; it could only have
                // counted the representative against itself.
                if let Some(posting) = index.by_token.get(token) {
                    self.overlap.add(posting);
                }
            }
            for (token_count, candidates) in &index.by_token_count {
                let shorter = representative.tokens.len().min(*token_count);
                let threshold = (shorter * MIN_OVERLAP_NUMERATOR).div_ceil(MIN_OVERLAP_DENOMINATOR);
                self.overlap
                    .add_at_least(threshold, candidates, &mut self.matches);
            }

            let rare_token_count = representative
                .tokens
                .iter()
                .filter(|token| frequencies.is_rare(token))
                .count();
            if rare_token_count >= MIN_RARE_SHARED_TOKENS {
                self.rare.reset(rare_token_count);
                for token in representative
                    .tokens
                    .iter()
                    .filter(|token| frequencies.is_rare(token))
                {
                    if let Some(posting) = index.by_token.get(token) {
                        self.rare.add(posting);
                    }
                }
                self.rare
                    .add_at_least(MIN_RARE_SHARED_TOKENS, &index.all, &mut self.matches);
            }
            self.matches.and_assign(&self.tag_pool);
        }

        if !representative.normalized_title.is_empty()
            && let Some(candidates) = index.by_title.get(&representative.normalized_title)
        {
            for &candidate in candidates {
                self.matches.set(candidate);
            }
        }
        &self.matches
    }
}

pub fn run(args: TriageArgs, file: Option<PathBuf>, pretty: bool) -> AppResult<i32> {
    if args.min_count < 2 {
        return Err(AppError::invalid_argument(
            "--min-count must be at least 2",
            "Pass --min-count 2 or greater.",
        ));
    }

    let resolved = store::discover(file)?;
    let store::LoadedFold {
        items,
        mut warnings,
    } = store::load_folded(&resolved)?;
    let (items, auto_captures) = crate::partition_auto_captures(items, args.include_auto);
    let hidden = auto_captures
        .iter()
        .filter(|item| is_open_cut(item))
        .count();
    if hidden > 0 {
        warnings.push(crate::auto_capture_warning(hidden));
    }

    let data = triage(items, args.min_count);
    let exit = i32::from(!data.clusters.is_empty());
    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.warnings = warnings;
    output::write_success(data, pretty, meta)
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(exit)
}

pub(crate) fn triage(items: Vec<ListItem>, min_count: usize) -> TriageData {
    let analysis = chronic_clusters(items, min_count);
    let clusters: Vec<_> = analysis.clusters.iter().map(materialize_cluster).collect();

    TriageData {
        count: clusters.len(),
        clusters,
        scanned: analysis.scanned,
    }
}

pub(crate) fn chronic_clusters(items: Vec<ListItem>, min_count: usize) -> ChronicAnalysis {
    let mut candidates: Vec<_> = items
        .into_iter()
        .filter(is_open_cut)
        .map(|item| {
            let normalized_title = normalized_title(&item.text);
            Candidate {
                timestamp: item
                    .ts
                    .parse()
                    .expect("folded items have valid RFC3339 timestamps"),
                tags: item.tags.iter().cloned().collect(),
                tokens: scoring_tokens(&normalized_title),
                normalized_title,
                item,
            }
        })
        .collect();
    candidates.sort_by(candidate_order);

    let scanned = candidates.len();
    let frequencies = token_frequencies(candidates.iter());
    // Count every folded open cut by title. This is a recurrence signal, not
    // an ID deduplication pass, so independently materialized pc_/bl_ records
    // both contribute when their normalized titles match.
    let mut title_occurrences = BTreeMap::new();
    for candidate in &candidates {
        *title_occurrences
            .entry(candidate.normalized_title.clone())
            .or_insert(0) += 1;
    }

    let candidate_index = CandidateIndex::new(&candidates, &frequencies);
    let mut scratch = CandidateScratch::new(scanned);
    let mut claimed = vec![false; scanned];
    let mut clusters = Vec::new();
    for representative in 0..scanned {
        if claimed[representative] {
            continue;
        }

        // The earliest unclaimed candidate (then lowest ID) is the stable
        // representative. Members must link directly to it; unioning every
        // pair would turn an A~B~C chain into a transitive A/B/C cluster.
        let mut members = vec![representative];
        for candidate in scratch
            .matching_candidates(&candidates[representative], &candidate_index, &frequencies)
            .indices()
        {
            if candidate <= representative {
                continue;
            }
            if !claimed[candidate]
                && linked(
                    &candidates[representative],
                    &candidates[candidate],
                    &frequencies,
                )
            {
                members.push(candidate);
            }
        }

        // Claim only when the cluster is actually reported: members consumed by
        // a below-threshold cluster must stay free to join a later
        // representative, or real chronic clusters go unreported.
        if members.len() >= min_count {
            for &member in &members {
                claimed[member] = true;
            }
            clusters.push(ordered_cluster(&candidates, members, &title_occurrences));
        }
    }
    clusters.sort_by(|left, right| {
        right
            .data
            .members
            .len()
            .cmp(&left.data.members.len())
            .then_with(|| left.oldest_timestamp.cmp(&right.oldest_timestamp))
            .then_with(|| left.first_id.cmp(&right.first_id))
    });
    ChronicAnalysis {
        clusters: clusters.into_iter().map(|cluster| cluster.data).collect(),
        scanned,
    }
}

fn is_open_cut(item: &ListItem) -> bool {
    item.kind == "cut" && item.status == ItemStatus::Open
}

pub(crate) fn candidate_order(left: &Candidate, right: &Candidate) -> std::cmp::Ordering {
    left.timestamp
        .cmp(&right.timestamp)
        .then_with(|| left.item.id.cmp(&right.item.id))
}

pub(crate) fn normalized_title(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

const STOPWORDS: &[&str] = &[
    "and", "are", "but", "cannot", "for", "from", "into", "need", "one", "that", "the", "this",
    "to", "use", "uses", "with",
];
const MIN_OVERLAP_NUMERATOR: usize = 4;
const MIN_OVERLAP_DENOMINATOR: usize = 5;
const MIN_RARE_SHARED_TOKENS: usize = 3;

pub(crate) fn scoring_tokens(normalized_title: &str) -> BTreeSet<String> {
    normalized_title
        .split_whitespace()
        .filter(|token| token.chars().count() > 2 && !STOPWORDS.contains(token))
        .map(str::to_owned)
        .collect()
}

pub(crate) fn token_frequencies<'a>(
    candidates: impl IntoIterator<Item = &'a Candidate>,
) -> TokenFrequencies {
    let mut counts = BTreeMap::new();
    let mut candidate_count = 0;
    for candidate in candidates {
        candidate_count += 1;
        for token in &candidate.tokens {
            *counts.entry(token.clone()).or_insert(0) += 1;
        }
    }
    TokenFrequencies {
        counts,
        candidate_count,
    }
}

impl TokenFrequencies {
    /// True when at least two candidates carry the token, so it can take part
    /// in a shared-token count between two different candidates.
    fn is_shared(&self, token: &str) -> bool {
        self.counts
            .get(token)
            .copied()
            .expect("tokens being indexed were counted")
            > 1
    }

    fn is_rare(&self, token: &str) -> bool {
        let rare_limit = self.candidate_count.div_ceil(4).max(2);
        self.counts
            .get(token)
            .copied()
            .expect("tokens being scored were counted")
            <= rare_limit
    }
}

pub(crate) fn linked(left: &Candidate, right: &Candidate, frequencies: &TokenFrequencies) -> bool {
    // Identical non-empty normalized titles always link: recurrence of the
    // exact same title is the strongest chronic signal, so tags must not
    // suppress it.
    if !left.normalized_title.is_empty() && left.normalized_title == right.normalized_title {
        return true;
    }

    // Otherwise, retain untagged-to-untagged matches: tags are optional, so
    // untagged near-duplicates remain direct matches. They are the likeliest
    // bridges, so cluster construction only compares candidates to a stable
    // representative rather than taking a transitive closure through members.
    let matching_tags =
        (left.tags.is_empty() && right.tags.is_empty()) || !left.tags.is_disjoint(&right.tags);
    matching_tags && similar_enough(&left.tokens, &right.tokens, frequencies)
}

fn similar_enough(
    left: &BTreeSet<String>,
    right: &BTreeSet<String>,
    frequencies: &TokenFrequencies,
) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let shared = left.intersection(right).count();
    let shorter = left.len().min(right.len());
    // Near-duplicates already have strong evidence in their filtered token
    // overlap. Reworded descriptions instead need several locally rare terms.
    if shared * MIN_OVERLAP_DENOMINATOR >= shorter * MIN_OVERLAP_NUMERATOR {
        return true;
    }

    left.intersection(right)
        .filter(|token| frequencies.is_rare(token))
        .count()
        >= MIN_RARE_SHARED_TOKENS
}

fn ordered_cluster(
    candidates: &[Candidate],
    mut members: Vec<usize>,
    title_occurrences: &BTreeMap<String, usize>,
) -> OrderedCluster {
    members.sort_by(|left, right| candidate_order(&candidates[*left], &candidates[*right]));
    let representative = &candidates[members[0]];
    let member_occurrences: Vec<_> = members
        .iter()
        .map(|index| {
            title_occurrences
                .get(&candidates[*index].normalized_title)
                .copied()
                .expect("every candidate title is counted")
        })
        .collect();

    OrderedCluster {
        oldest_timestamp: representative.timestamp,
        first_id: representative.item.id.clone(),
        data: ChronicCluster {
            // The latest member is the final one because the indices are in
            // stable candidate order. Preserve its title occurrence count for
            // triage's existing envelope, while exposing every member's count
            // to read-only consumers that need aggregate evidence.
            displayed_occurrences: *member_occurrences
                .last()
                .expect("chronic clusters have members"),
            member_occurrences,
            members: members
                .into_iter()
                .map(|index| candidates[index].clone())
                .collect(),
        },
    }
}

fn materialize_cluster(cluster: &ChronicCluster) -> TriageCluster {
    let latest = cluster
        .members
        .last()
        .expect("chronic clusters have members");
    let tags: BTreeSet<_> = cluster
        .members
        .iter()
        .flat_map(|candidate| candidate.tags.iter().cloned())
        .collect();

    TriageCluster {
        count: cluster.members.len(),
        // Keyed on the same record whose text the cluster displays, so a
        // consumer can interpret occurrences against the title it sees.
        occurrences: cluster.displayed_occurrences,
        ids: cluster
            .members
            .iter()
            .map(|candidate| candidate.item.id.clone())
            .collect(),
        tags: tags.into_iter().collect(),
        text: latest.item.text.clone(),
        source: latest.item.source.clone(),
        suggested_action: "graduate".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;

    fn candidate(index: usize, text: &str, tags: &[&str]) -> Candidate {
        let timestamp = format!("2026-08-18T00:00:{index:02}.000Z");
        let normalized_title = normalized_title(text);
        let item = ListItem {
            kind: "cut".into(),
            id: format!("bl_{index:012x}"),
            ts: timestamp.clone(),
            agent: "test".into(),
            text: text.into(),
            tags: tags.iter().map(|tag| (*tag).into()).collect(),
            severity: Some(Severity::Minor),
            cwd: ".".into(),
            source: None,
            evidence: None,
            status: ItemStatus::Open,
            resolution: None,
        };
        Candidate {
            timestamp: timestamp.parse().unwrap(),
            tags: item.tags.iter().cloned().collect(),
            tokens: scoring_tokens(&normalized_title),
            normalized_title,
            item,
        }
    }

    #[test]
    fn candidate_index_matches_the_direct_link_rule_for_mixed_pools() {
        // This checks the index against the public pair rule, rather than a
        // second clustering implementation. It covers exact-title matches
        // across tags, shared-tag and untagged scoring, the rare-token path,
        // overlapping tag pools, and titles with empty scoring sets.
        let mut candidates = vec![
            candidate(0, "Exact title is strongest", &["alpha"]),
            candidate(1, "exact-title is strongest!", &["beta"]),
            candidate(
                2,
                "cache endpoint returns alpha beta gamma",
                &["ops", "api"],
            ),
            candidate(3, "cache endpoint returns alpha beta delta", &["ops"]),
            candidate(4, "cache endpoint returns alpha beta delta", &["billing"]),
            candidate(5, "red green blue purple orange", &["rare"]),
            candidate(6, "red green blue violet magenta", &["rare"]),
            candidate(7, "untagged data source cache fail", &[]),
            candidate(8, "untagged data cache source failing", &[]),
            candidate(9, "same words different isolated tag", &["isolated"]),
            candidate(10, "same words different other tag", &["other"]),
            candidate(11, "the and for", &["ops"]),
            candidate(12, "this with need", &["ops"]),
            candidate(13, "!!!", &["ops"]),
            candidate(14, "???", &["ops"]),
            // Every scoring token here occurs in exactly one candidate, so none
            // of them reach the index. The representative must still match
            // itself through its exact title.
            candidate(15, "wholly unshared vocabulary xyzzy", &["ops"]),
        ];
        candidates.sort_by(candidate_order);
        let frequencies = token_frequencies(candidates.iter());
        let index = CandidateIndex::new(&candidates, &frequencies);
        let mut scratch = CandidateScratch::new(candidates.len());

        for (representative_index, representative) in candidates.iter().enumerate() {
            let actual: Vec<_> = scratch
                .matching_candidates(representative, &index, &frequencies)
                .indices()
                .collect();
            let expected: Vec<_> = candidates
                .iter()
                .enumerate()
                .filter_map(|(candidate_index, candidate)| {
                    linked(representative, candidate, &frequencies).then_some(candidate_index)
                })
                .collect();
            assert_eq!(
                actual, expected,
                "candidate index changed the direct-link set for representative {representative_index}"
            );
        }
    }

    /// A tiny deterministic generator. The suite must stay reproducible, and
    /// this needs no distribution quality beyond "spreads the corpus around".
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, limit: usize) -> usize {
            (self.next() % limit as u64) as usize
        }
    }

    #[test]
    fn candidate_index_matches_the_direct_link_rule_across_random_corpora() {
        // Skipping unshared tokens is only safe if the index still reproduces
        // the pair rule exactly. Each seed builds a corpus that mixes a small
        // shared vocabulary with per-candidate words nothing else uses, which
        // is the shape that drives the document frequency of most tokens to 1.
        const SHARED_WORDS: &[&str] = &[
            "cache", "endpoint", "timeout", "retry", "parse", "lock", "schema", "digest",
        ];
        const TAGS: &[&str] = &["ops", "api", "billing", "store"];

        for seed in 1..=400_u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
            let count = 2 + rng.below(14);
            let mut candidates = Vec::with_capacity(count);
            for index in 0..count {
                let mut words = Vec::new();
                for _ in 0..1 + rng.below(5) {
                    words.push(SHARED_WORDS[rng.below(SHARED_WORDS.len())].to_owned());
                }
                // Words unique to this candidate, so their document frequency
                // is 1 and the index must leave them out.
                for unique in 0..rng.below(4) {
                    words.push(format!("uniq{seed}x{index}x{unique}"));
                }
                let tag_count = rng.below(3);
                let tags: Vec<_> = (0..tag_count)
                    .map(|_| TAGS[rng.below(TAGS.len())])
                    .collect();
                candidates.push(candidate(index, &words.join(" "), &tags));
            }
            candidates.sort_by(candidate_order);

            let frequencies = token_frequencies(candidates.iter());
            let index = CandidateIndex::new(&candidates, &frequencies);
            let mut scratch = CandidateScratch::new(candidates.len());
            for (representative_index, representative) in candidates.iter().enumerate() {
                let actual: Vec<_> = scratch
                    .matching_candidates(representative, &index, &frequencies)
                    .indices()
                    .collect();
                let expected: Vec<_> = candidates
                    .iter()
                    .enumerate()
                    .filter_map(|(candidate_index, candidate)| {
                        linked(representative, candidate, &frequencies).then_some(candidate_index)
                    })
                    .collect();
                assert_eq!(
                    actual, expected,
                    "seed {seed} representative {representative_index} diverged from the pair rule"
                );
            }
        }
    }
}
