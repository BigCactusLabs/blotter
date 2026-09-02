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
    pub origin: Option<crate::Origin>,
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

pub(crate) struct CorpusFrequencies {
    token_counts: BTreeMap<String, usize>,
    tag_counts: BTreeMap<String, usize>,
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
/// counts a candidate against itself — and self is below the floor every caller
/// scans from. Skipping those tokens keeps the index proportional to the shared
/// vocabulary instead of the whole vocabulary, which is what bounds memory when
/// every record brings new words.
///
/// `by_tag` is bounded the same way. A tag counted once in the analyzed
/// population is carried by exactly one record: either no representative
/// carries it and its pool is never queried, or the sole carrier is the
/// representative, whose own bit every caller discards. Either way the pool
/// cannot change a result, and a log where most tags name a single record
/// would otherwise pay a full N-bit row for each of them.
///
/// `verify` reuses the index with resolved anchors as representatives and the
/// open cuts as the indexed candidates. That stays sound as long as the
/// frequencies passed here also count the representatives: a token shared by an
/// anchor and an open cut then has a count of at least two and is indexed, and
/// every representative token has a counted frequency to score against.
pub(crate) struct CandidateIndex {
    by_title: BTreeMap<String, Vec<usize>>,
    by_tag: BTreeMap<String, BitSet>,
    untagged: BitSet,
    by_token: BTreeMap<String, BitSet>,
    by_token_count: BTreeMap<usize, BitSet>,
    all: BitSet,
}

impl CandidateIndex {
    pub(crate) fn new(candidates: &[Candidate], frequencies: &CorpusFrequencies) -> Self {
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
                    if frequencies.is_shared_tag(tag) {
                        by_tag
                            .entry(tag.clone())
                            .or_insert_with(|| BitSet::empty(words))
                            .set(index);
                    }
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

pub(crate) struct BitSet {
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

    fn clear_from(&mut self, word: usize) {
        self.words[word..].fill(0);
    }

    fn set(&mut self, index: usize) {
        self.words[index / 64] |= 1_u64 << (index % 64);
    }

    fn copy_from(&mut self, word: usize, source: &Self) {
        self.words[word..].copy_from_slice(&source.words[word..]);
    }

    fn or_assign(&mut self, word: usize, source: &Self) {
        for (target, source) in self.words[word..].iter_mut().zip(&source.words[word..]) {
            *target |= source;
        }
    }

    fn and_assign(&mut self, word: usize, source: &Self) {
        for (target, source) in self.words[word..].iter_mut().zip(&source.words[word..]) {
            *target &= source;
        }
    }

    /// Ascending set positions at or above `floor`. Callers whose candidates
    /// are sorted can turn an ordered cutoff into a starting word instead of a
    /// second filtering pass: whole words below the floor are never visited.
    pub(crate) fn indices_from(&self, floor: usize) -> impl Iterator<Item = usize> + '_ {
        let first_word = floor / 64;
        let first_mask = u64::MAX << (floor % 64);
        self.words
            .iter()
            .enumerate()
            .skip(first_word)
            .flat_map(move |(word_index, word)| {
                let mut remaining = if word_index == first_word {
                    *word & first_mask
                } else {
                    *word
                };
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

    fn reset(&mut self, word: usize, maximum: usize) {
        let required_planes = if maximum == 0 {
            0
        } else {
            maximum.ilog2() as usize + 1
        };
        self.planes
            .resize_with(required_planes, || vec![0; self.words]);
        for plane in &mut self.planes {
            plane[word..].fill(0);
        }
    }

    fn add(&mut self, word: usize, source: &BitSet) {
        for word_index in word..self.words {
            let mut carry = source.words[word_index];
            for plane in &mut self.planes {
                let next_carry = plane[word_index] & carry;
                plane[word_index] ^= carry;
                carry = next_carry;
            }
        }
    }

    fn add_at_least(&self, word: usize, threshold: usize, allowed: &BitSet, target: &mut BitSet) {
        debug_assert!(threshold > 0);
        for word_index in word..self.words {
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

pub(crate) struct CandidateScratch {
    tag_pool: BitSet,
    matches: BitSet,
    overlap: BitSlicedCounter,
    rare: BitSlicedCounter,
}

impl CandidateScratch {
    pub(crate) fn new(candidate_count: usize) -> Self {
        let words = candidate_count.div_ceil(64);
        Self {
            tag_pool: BitSet::empty(words),
            matches: BitSet::empty(words),
            overlap: BitSlicedCounter::new(words),
            rare: BitSlicedCounter::new(words),
        }
    }

    /// Positions below `floor` are unspecified in the returned set: the caller
    /// promises to read it with `indices_from(floor)` or tighter. Both callers
    /// already discard that prefix — triage drops every candidate at or before
    /// its representative, and verify drops everything up to the resolution —
    /// so the whole bit-parallel count can start at the floor's word instead of
    /// computing a prefix that is thrown away. It is the one bound that scales
    /// with the representative's own position rather than the corpus size.
    pub(crate) fn matching_candidates<'a>(
        &'a mut self,
        representative: &Candidate,
        index: &CandidateIndex,
        frequencies: &CorpusFrequencies,
        floor: usize,
    ) -> &'a BitSet {
        let word = floor / 64;
        self.matches.clear_from(word);
        if self.score_tokens(representative, index, frequencies, word) {
            self.tag_pool.clear_from(word);
            if representative.tags.is_empty() {
                self.tag_pool.copy_from(word, &index.untagged);
            } else {
                for tag in &representative.tags {
                    if let Some(pool) = index.by_tag.get(tag) {
                        self.tag_pool.or_assign(word, pool);
                    }
                }
            }
            self.matches.and_assign(word, &self.tag_pool);
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

    /// Runs whichever r19 scoring path can still reach its threshold, and
    /// reports whether either ran. When neither can, the scored match set is
    /// provably empty and the tag pool that would mask it is never built.
    ///
    /// Only a token with a posting set can raise a candidate's shared-token
    /// count, so the number of the representative's indexed tokens is an upper
    /// bound on every count this scan can produce. A threshold above that bound
    /// is unreachable, and counting against it costs a full pass over the
    /// candidate space to prove an answer already known. Both paths are skipped
    /// on the bound instead. Records whose wording is mostly their own are the
    /// common case in a real log, and they are exactly the ones the bound
    /// retires.
    fn score_tokens(
        &mut self,
        representative: &Candidate,
        index: &CandidateIndex,
        frequencies: &CorpusFrequencies,
        word: usize,
    ) -> bool {
        if representative.tokens.is_empty() {
            return false;
        }
        let reachable = representative
            .tokens
            .iter()
            .filter(|token| index.by_token.contains_key(*token))
            .count();
        let mut scored = false;

        // Thresholds rise with the candidate's token count, so the smallest
        // indexed count carries the smallest threshold in the whole scan.
        let lowest = index
            .by_token_count
            .keys()
            .next()
            .map(|count| overlap_threshold(representative.tokens.len(), *count));
        if lowest.is_some_and(|threshold| threshold <= reachable) {
            // `reachable`, not the token count, is the largest value a counter
            // can hold, so it is what sizes the planes.
            self.overlap.reset(word, reachable);
            for token in &representative.tokens {
                // An unshared token has no posting set; it could only have
                // counted the representative against itself.
                if let Some(posting) = index.by_token.get(token) {
                    self.overlap.add(word, posting);
                }
            }
            for (token_count, candidates) in &index.by_token_count {
                let threshold = overlap_threshold(representative.tokens.len(), *token_count);
                if threshold > reachable {
                    break;
                }
                self.overlap
                    .add_at_least(word, threshold, candidates, &mut self.matches);
            }
            scored = true;
        }

        // The same bound applies to the rare path, over the rare tokens alone.
        let rare_reachable = representative
            .tokens
            .iter()
            .filter(|token| frequencies.is_rare(token) && index.by_token.contains_key(*token))
            .count();
        if rare_reachable >= MIN_RARE_SHARED_TOKENS {
            self.rare.reset(word, rare_reachable);
            for token in representative
                .tokens
                .iter()
                .filter(|token| frequencies.is_rare(token))
            {
                if let Some(posting) = index.by_token.get(token) {
                    self.rare.add(word, posting);
                }
            }
            self.rare
                .add_at_least(word, MIN_RARE_SHARED_TOKENS, &index.all, &mut self.matches);
            scored = true;
        }

        scored
    }
}

fn overlap_threshold(representative_tokens: usize, candidate_tokens: usize) -> usize {
    (representative_tokens.min(candidate_tokens) * MIN_OVERLAP_NUMERATOR)
        .div_ceil(MIN_OVERLAP_DENOMINATOR)
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
        items, warnings, ..
    } = store::load_folded(&resolved)?;

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
    let frequencies = corpus_frequencies(candidates.iter());
    // Count every folded open cut by title. This is a recurrence signal, not
    // an ID deduplication pass, so two independently materialized records
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
        // Earlier candidates are already in a cluster or were already compared
        // against this one, so the self-exclusion is a floor on the scan rather
        // than a test inside it.
        let mut members = vec![representative];
        for candidate in scratch
            .matching_candidates(
                &candidates[representative],
                &candidate_index,
                &frequencies,
                representative + 1,
            )
            .indices_from(representative + 1)
        {
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

/// Tokens that carry no topical signal. This is the Snowball English stopword
/// list (https://snowballstem.org/algorithms/english/stop.txt, 174 entries)
/// passed through blotter's own normalization — lowercased, every
/// non-alphanumeric character replaced by a space, split on whitespace — and
/// reduced to the tokens `scoring_tokens` can see: three or more characters.
/// Deriving it through the normalizer instead of copying it verbatim is what
/// makes `wouldn`, `doesn` and `let` members: blotter splits `wouldn't` into
/// `wouldn` and `t`, so the published apostrophe spellings would never match.
///
/// Four r19 entries Snowball does not carry are retained because they are
/// filler in friction narration: `need`, `one`, `use`, `uses`. r19's `to` is
/// dropped as dead weight — a two-character token never reaches this check.
///
/// Frequency cannot do this job at fixture scale. `is_rare` accepts
/// `df <= max(2, ceil(N/4))`, a token shared by the two candidates under test
/// always has `df >= 2`, and the floor of 2 exists because a lower floor would
/// make no shared token rare and retire the path. In a four-candidate analysis
/// every shared token is rare, so no ratio separates filler from content there.
/// A common English word is removable as a word at every scale, and as a
/// frequency only at some. See design doc r44.
///
/// Sorted and unique; `scoring_tokens` binary-searches it.
const STOPWORDS: &[&str] = &[
    "about",
    "above",
    "after",
    "again",
    "against",
    "all",
    "and",
    "any",
    "are",
    "aren",
    "because",
    "been",
    "before",
    "being",
    "below",
    "between",
    "both",
    "but",
    "can",
    "cannot",
    "could",
    "couldn",
    "did",
    "didn",
    "does",
    "doesn",
    "doing",
    "don",
    "down",
    "during",
    "each",
    "few",
    "for",
    "from",
    "further",
    "had",
    "hadn",
    "has",
    "hasn",
    "have",
    "haven",
    "having",
    "her",
    "here",
    "hers",
    "herself",
    "him",
    "himself",
    "his",
    "how",
    "into",
    "isn",
    "its",
    "itself",
    "let",
    "more",
    "most",
    "mustn",
    "myself",
    "need",
    "nor",
    "not",
    "off",
    "once",
    "one",
    "only",
    "other",
    "ought",
    "our",
    "ours",
    "ourselves",
    "out",
    "over",
    "own",
    "same",
    "shan",
    "she",
    "should",
    "shouldn",
    "some",
    "such",
    "than",
    "that",
    "the",
    "their",
    "theirs",
    "them",
    "themselves",
    "then",
    "there",
    "these",
    "they",
    "this",
    "those",
    "through",
    "too",
    "under",
    "until",
    "use",
    "uses",
    "very",
    "was",
    "wasn",
    "were",
    "weren",
    "what",
    "when",
    "where",
    "which",
    "while",
    "who",
    "whom",
    "why",
    "with",
    "won",
    "would",
    "wouldn",
    "you",
    "your",
    "yours",
    "yourself",
    "yourselves",
];
const MIN_OVERLAP_NUMERATOR: usize = 4;
const MIN_OVERLAP_DENOMINATOR: usize = 5;
const MIN_RARE_SHARED_TOKENS: usize = 3;

pub(crate) fn scoring_tokens(normalized_title: &str) -> BTreeSet<String> {
    normalized_title
        .split_whitespace()
        .filter(|token| token.chars().count() > 2 && STOPWORDS.binary_search(token).is_err())
        .map(str::to_owned)
        .collect()
}

/// Counts tokens and tags over the whole analyzed population — the candidates
/// plus any representative that is not one of them, as `verify` has. Both
/// counts decide what the index is allowed to leave out, so both must see the
/// representatives: a tag or token shared by a representative and one candidate
/// has to count as shared.
pub(crate) fn corpus_frequencies<'a>(
    candidates: impl IntoIterator<Item = &'a Candidate>,
) -> CorpusFrequencies {
    let mut token_counts = BTreeMap::new();
    let mut tag_counts = BTreeMap::new();
    let mut candidate_count = 0;
    for candidate in candidates {
        candidate_count += 1;
        for token in &candidate.tokens {
            *token_counts.entry(token.clone()).or_insert(0) += 1;
        }
        for tag in &candidate.tags {
            *tag_counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }
    CorpusFrequencies {
        token_counts,
        tag_counts,
        candidate_count,
    }
}

impl CorpusFrequencies {
    /// True when at least two candidates carry the token, so it can take part
    /// in a shared-token count between two different candidates.
    fn is_shared(&self, token: &str) -> bool {
        self.token_counts
            .get(token)
            .copied()
            .expect("tokens being indexed were counted")
            > 1
    }

    /// True when at least two records carry the tag, so its pool can hold a
    /// record other than the representative that queries it.
    fn is_shared_tag(&self, tag: &str) -> bool {
        self.tag_counts
            .get(tag)
            .copied()
            .expect("tags being indexed were counted")
            > 1
    }

    fn is_rare(&self, token: &str) -> bool {
        let rare_limit = self.candidate_count.div_ceil(4).max(2);
        self.token_counts
            .get(token)
            .copied()
            .expect("tokens being scored were counted")
            <= rare_limit
    }
}

pub(crate) fn linked(left: &Candidate, right: &Candidate, frequencies: &CorpusFrequencies) -> bool {
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
    frequencies: &CorpusFrequencies,
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
        origin: latest.item.origin.clone(),
        suggested_action: "graduate".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Impact;

    #[test]
    fn stopwords_are_sorted_and_unique_for_binary_search() {
        assert!(
            STOPWORDS.windows(2).all(|pair| pair[0] < pair[1]),
            "STOPWORDS must be sorted and deduplicated: binary_search depends on it"
        );
    }

    /// Every entry must be a token the tokenizer can actually emit. A stopword
    /// list that disagrees with its own tokenizer silently does nothing:
    /// scikit-learn documents exactly this against its own default tokenizer,
    /// which splits `we've` into `we` and `ve`, so listing `we've` without `ve`
    /// retains `ve`; Nothman, Qin and Yurchak (ACL 2018, W18-2502) found the
    /// same class of defect across the popular published lists. r44's list is
    /// derived *through* `normalized_title` so that it does not have it -- 174
    /// Snowball entries become 118 after normalization and the length floor,
    /// plus the four r19 retentions -- and this test is what stops a later hand
    /// edit from reintroducing it.
    ///
    /// Asserting against the real normalizer rather than restating its rules
    /// keeps the two from drifting apart: `normalized_title` lowercases and
    /// replaces every non-alphanumeric character with a space, so any entry it
    /// does not return unchanged is one no token can equal. `scoring_tokens`
    /// then drops tokens of two or fewer characters before the lookup, so a
    /// shorter entry is unreachable as well.
    #[test]
    fn stopwords_are_tokens_the_tokenizer_can_emit() {
        for word in STOPWORDS {
            assert_eq!(
                normalized_title(word),
                *word,
                "`{word}` is not what the tokenizer produces, so no token can ever match it"
            );
            assert!(
                word.chars().count() > 2,
                "`{word}` is below the length filter `scoring_tokens` applies before the lookup"
            );
        }
    }

    fn candidate(index: usize, text: &str, tags: &[&str]) -> Candidate {
        let timestamp = format!("2026-08-18T00:00:{index:02}.000Z");
        let normalized_title = normalized_title(text);
        let item = ListItem {
            kind: "cut".into(),
            id: format!("bl_{index:020x}"),
            ts: timestamp.clone(),
            agent: "test".into(),
            text: text.into(),
            tags: tags.iter().map(|tag| (*tag).into()).collect(),
            impact: Some(Impact::Low),
            cwd: ".".into(),
            origin: None,
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
        let frequencies = corpus_frequencies(candidates.iter());
        let index = CandidateIndex::new(&candidates, &frequencies);
        let mut scratch = CandidateScratch::new(candidates.len());

        for (representative_index, representative) in candidates.iter().enumerate() {
            let actual: Vec<_> = scratch
                .matching_candidates(representative, &index, &frequencies, 0)
                .indices_from(0)
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

    #[test]
    fn candidate_index_matches_the_direct_link_rule_for_external_representatives() {
        // `verify` scores resolved anchors against an index built over the open
        // cuts alone. A tag or token carried by exactly one anchor and one open
        // cut is shared by two records but appears only once among the indexed
        // candidates, so counting the representatives is what keeps its pool in
        // the index at all.
        let mut open = vec![
            candidate(
                0,
                "cache endpoint returns alpha beta gamma",
                &["shared-once"],
            ),
            candidate(1, "unrelated wording nothing repeats here", &["solo"]),
            candidate(2, "cache endpoint returns alpha beta gamma", &["ops"]),
        ];
        open.sort_by(candidate_order);
        let anchors = [
            candidate(
                10,
                "cache endpoint returns alpha beta delta",
                &["shared-once"],
            ),
            candidate(
                11,
                "cache endpoint returns alpha beta delta",
                &["absent-tag"],
            ),
        ];

        let frequencies = corpus_frequencies(open.iter().chain(anchors.iter()));
        let index = CandidateIndex::new(&open, &frequencies);
        let mut scratch = CandidateScratch::new(open.len());
        for (anchor_index, anchor) in anchors.iter().enumerate() {
            let actual: Vec<_> = scratch
                .matching_candidates(anchor, &index, &frequencies, 0)
                .indices_from(0)
                .collect();
            let expected: Vec<_> = open
                .iter()
                .enumerate()
                .filter_map(|(candidate_index, candidate)| {
                    linked(anchor, candidate, &frequencies).then_some(candidate_index)
                })
                .collect();
            assert_eq!(
                actual, expected,
                "anchor {anchor_index} diverged from the pair rule"
            );
        }
        assert!(
            !expected_is_empty(&open, &anchors[0], &frequencies),
            "the anchor sharing a once-carried tag must link, or this test proves nothing"
        );
    }

    fn expected_is_empty(
        open: &[Candidate],
        anchor: &Candidate,
        frequencies: &CorpusFrequencies,
    ) -> bool {
        !open
            .iter()
            .any(|candidate| linked(anchor, candidate, frequencies))
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

            let frequencies = corpus_frequencies(candidates.iter());
            let index = CandidateIndex::new(&candidates, &frequencies);
            let mut scratch = CandidateScratch::new(candidates.len());
            for (representative_index, representative) in candidates.iter().enumerate() {
                let actual: Vec<_> = scratch
                    .matching_candidates(representative, &index, &frequencies, 0)
                    .indices_from(0)
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
