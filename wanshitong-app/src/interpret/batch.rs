//! Batches: directories of videos that aren't mostly movies', e.g. a
//! season's, whose own titles are often their episodes' rather than their
//! series' name, so a search that goes by them is tried for a few of them
//! and skipped for the rest if it finds nothing more.

use super::knowledge_base::{KbResult, KnowledgeBase, KnowledgeBaseError, RankedSearchFields};
use futures::{StreamExt, stream::FuturesUnordered};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};
use wanshitong_core::domain::{
    ClaimPredicate, DataSourceUuid, FileFacts, KbQueryEvent, PathId, Ranked, RankedClaimPredicate,
    SubjectIndex, WorkKind,
};

/// Searches for the first videos of each batch among `files`, for each
/// kind, with and without their own names, to tell whether the rest's are
/// worth searching for, and if not, drops them from those videos' `fields`.
/// Each batch's videos' fields also get the words the batch's titles share,
/// to look their series up by. What the trials found, by their videos,
/// stands for those videos' searches.
pub(super) async fn search_batches(
    kb: &dyn KnowledgeBase,
    files: &[(PathId, FileFacts)],
    claims: &HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
    fields: &mut HashMap<SubjectIndex, RankedSearchFields>,
) -> Trials {
    let batches = batches(files, claims);
    for batch in &batches {
        for subject in &batch.subjects {
            if let Some(search_fields) = fields.get_mut(subject) {
                search_fields.batch = Some(batch.shared.clone());
            }
        }
    }
    try_batches(kb, &batches, fields).await
}

/// A directory of videos that aren't mostly movies' by their kinds, e.g. a
/// season's or a series' whole run, whose own titles are often their
/// episodes' rather than their series' name.
struct Batch {
    /// Its videos, in their paths' order.
    subjects: Vec<SubjectIndex>,
    /// The words that the highest-ranked movie titles of its videos with one
    /// all begin with, if any, e.g. `perman` for `perman 第140話 「…」` and
    /// the rest.
    shared: Option<String>,
}

/// How many of a batch's videos of a kind are tried, searched
/// for with and without the names that are theirs alone, to tell whether
/// the others' are worth searching for.
const BATCH_TRIALS: usize = 3;

/// What a video's search found, and the queries it made.
type SearchResult = Result<(Vec<KbResult>, Vec<KbQueryEvent>), KnowledgeBaseError>;

/// What trying batches' videos found: the trials' searches, with
/// their own names, by their videos, as any other video's search, and the
/// queries of those without them.
pub(super) struct Trials {
    pub(super) searched: HashMap<SubjectIndex, SearchResult>,
    pub(super) queries: HashMap<SubjectIndex, Vec<KbQueryEvent>>,
}

/// Each batch's videos with names of their own, e.g. their
/// episodes' titles, `the dawn of adventure` of `romance dawn 01 - the dawn
/// of adventure`, aren't searched for by them if they find nothing for the
/// first [`BATCH_TRIALS`] of their kind that a search without them doesn't,
/// as a search a video, one per video, then finds nothing more. Their names
/// the directory's others have too, e.g. their series' in their folder's,
/// are searched for still, and a video left with no title nor series' name
/// is searched for by the words the directory's titles all begin with. Of
/// a kind: numbered as episodes, a special, or neither, as e.g. a special's
/// title among episodes' names it, and its episodes' don't.
async fn try_batches(
    kb: &dyn KnowledgeBase,
    batches: &[Batch],
    fields: &mut HashMap<SubjectIndex, RankedSearchFields>,
) -> Trials {
    let mut kinds = Vec::new();
    let mut trials = FuturesUnordered::new();
    for batch in batches {
        let mut counts = HashMap::<&str, usize>::new();
        for subject in &batch.subjects {
            let names: HashSet<&str> = fields
                .get(subject)
                .into_iter()
                .flat_map(search_names)
                .map(|name| name.value.as_str())
                .collect();
            for name in names {
                *counts.entry(name).or_default() += 1;
            }
        }
        let own: HashSet<String> = counts
            .into_iter()
            .filter(|&(_, count)| count == 1)
            .map(|(name, _)| name.to_string())
            .collect();
        let mut of_kind = HashMap::<(bool, bool), Vec<SubjectIndex>>::new();
        for &subject in &batch.subjects {
            let Some(search_fields) = fields.get(&subject) else {
                continue;
            };
            if search_names(search_fields).any(|name| own.contains(&name.value)) {
                let numbered = search_fields.episode_range.is_some()
                    || search_fields.absolute_episode_range.is_some();
                let special = search_fields.special_of.is_some();
                of_kind
                    .entry((numbered, special))
                    .or_default()
                    .push(subject);
            }
        }
        // Only a kind with others to skip.
        for subjects in of_kind.into_values().filter(|s| s.len() > BATCH_TRIALS) {
            let kind = kinds.len();
            for &subject in &subjects[..BATCH_TRIALS] {
                let with_own = fields[&subject].clone();
                let mut without_own = with_own.clone();
                skip_own_names(&mut without_own, &own, &batch.shared);
                trials.push(async move {
                    let with = kb.search_for_claims(&with_own).await;
                    let without = kb.search_for_claims(&without_own).await;
                    (kind, subject, with, without)
                });
            }
            kinds.push((own.clone(), batch.shared.clone(), subjects));
        }
    }
    let mut worth = vec![false; kinds.len()];
    let mut searched = HashMap::new();
    let mut queries = HashMap::new();
    while let Some((kind, subject, with, without)) = trials.next().await {
        match (&with, without) {
            (Ok((with, _)), Ok((without, without_queries))) => {
                worth[kind] |= !found(with).is_subset(&found(&without));
                queries.insert(subject, without_queries);
            }
            // As if worth it: the trial says nothing.
            _ => worth[kind] = true,
        }
        searched.insert(subject, with);
    }
    for (kind, (own, shared, subjects)) in kinds.into_iter().enumerate() {
        if worth[kind] {
            continue;
        }
        for subject in &subjects[BATCH_TRIALS..] {
            if let Some(search_fields) = fields.get_mut(subject) {
                skip_own_names(search_fields, &own, &shared);
            }
        }
    }
    Trials { searched, queries }
}

/// Drops a file's `own` names from those it's searched for by, and if no
/// title nor series' name is left, searches for its directory's `shared`
/// words instead, ranked as its best title was.
fn skip_own_names(fields: &mut RankedSearchFields, own: &HashSet<String>, shared: &Option<String>) {
    let best_title = fields.titles.iter().map(|title| title.rank).max();
    keep_names(fields, |name| !own.contains(name));
    if fields.titles.is_empty()
        && fields.series_names.is_empty()
        && let (Some(rank), Some(shared)) = (best_title, shared)
    {
        fields.titles.push(Ranked {
            rank,
            value: shared.clone(),
        });
    }
}

/// What a search found: each work, by its ID, and which of its parts, e.g.
/// a series' special, as a file's own name may find a special of a series
/// that its directory's name finds too.
fn found(results: &[KbResult]) -> HashSet<String> {
    results
        .iter()
        .map(|result| {
            let mut found: Vec<String> = result
                .iter()
                .filter_map(|claim| match &claim.value {
                    value @ (ClaimPredicate::MovieExternalId(_)
                    | ClaimPredicate::TvSeriesExternalId(_)
                    | ClaimPredicate::Season(_)
                    | ClaimPredicate::EpisodeRange(_)
                    | ClaimPredicate::AbsoluteEpisodeRange(_)
                    | ClaimPredicate::Special) => Some(format!("{value:?}")),
                    _ => None,
                })
                .collect();
            found.sort();
            found.join(" ")
        })
        .filter(|found| !found.is_empty())
        .collect()
}

/// The titles and series' names a file's searched for by.
fn search_names(fields: &RankedSearchFields) -> impl Iterator<Item = &Ranked<String>> {
    fields
        .titles
        .iter()
        .chain(&fields.series_names)
        .chain(&fields.last_resort_titles)
        .chain(&fields.last_resort_movie_titles)
        .chain(&fields.last_resort_series_names)
}

/// Keeps the titles and series' names a file's searched for by that `keep`.
fn keep_names(fields: &mut RankedSearchFields, keep: impl Fn(&str) -> bool) {
    for names in [
        &mut fields.titles,
        &mut fields.series_names,
        &mut fields.last_resort_titles,
        &mut fields.last_resort_movie_titles,
        &mut fields.last_resort_series_names,
    ] {
        names.retain(|name| keep(&name.value));
    }
}

/// Each directory of videos that aren't mostly movies' by their kinds, e.g.
/// a season's or a series' whole run, or its specials, and the words their
/// titles share. Not of one video, which has no others to share names with.
fn batches(
    files: &[(PathId, FileFacts)],
    claims: &HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
) -> Vec<Batch> {
    let mut by_dir = HashMap::<(DataSourceUuid, &Path), Vec<SubjectIndex>>::new();
    for (subject, (path, facts)) in files.iter().enumerate() {
        if facts.is_video()
            && let Some(dir) = path.rel_path.parent()
        {
            by_dir
                .entry((path.data_source, dir))
                .or_default()
                .push(subject);
        }
    }
    let title_of = |subject: &SubjectIndex| {
        claims
            .get(subject)?
            .iter()
            .filter(|claim| claim.active)
            .filter_map(|claim| match &claim.pred {
                ClaimPredicate::MovieTitle(title) => Some((claim.rank, title.as_str())),
                _ => None,
            })
            .max_by_key(|&(rank, _)| rank)
            .map(|(_, title)| title)
    };
    let kind_of = |subject: &SubjectIndex| {
        claims
            .get(subject)?
            .iter()
            .find_map(|claim| match claim.pred {
                ClaimPredicate::WorkKind(kind) if claim.active => Some(kind),
                _ => None,
            })
    };
    let mut batches = Vec::new();
    for mut subjects in by_dir.into_values() {
        let movies = subjects
            .iter()
            .filter(|subject| kind_of(subject) == Some(WorkKind::Movie))
            .count();
        let series = subjects
            .iter()
            .filter(|subject| kind_of(subject) == Some(WorkKind::TvSeries))
            .count();
        if subjects.len() < 2 || movies > series {
            continue;
        }
        subjects.sort_by_key(|&subject| &files[subject].0.rel_path);
        let mut titles = subjects.iter().filter_map(title_of);
        let shared = titles.next().map(|first| {
            titles.fold(
                first.split_whitespace().collect::<Vec<_>>(),
                |shared, title| {
                    shared
                        .iter()
                        .zip(title.split_whitespace())
                        .take_while(|(a, b)| **a == *b)
                        .map(|(a, _)| *a)
                        .collect()
                },
            )
        });
        let shared = shared
            .map(|words| words.join(" "))
            .map(|shared| {
                shared
                    .trim_end_matches([' ', '-', '~', '(', '['])
                    .to_string()
            })
            .filter(|shared| shared.chars().any(char::is_alphabetic))
            .filter(|shared| {
                // One word alike in Latin letters is often a kind of video,
                // e.g. `preview` of `preview 01` to `preview 24`, which names
                // no series; in a script without spaces, e.g. `パーマン`, one
                // word may be a whole name.
                let words = crate::title::words(shared);
                words.len() >= 2
                    || words
                        .iter()
                        .all(|word| !word.chars().any(|c| c.is_ascii_alphabetic()))
            });
        batches.push(Batch { subjects, shared });
    }
    batches
}
