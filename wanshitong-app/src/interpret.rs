//! Interpretation: deriving a catalog from the facts of an interpretation.
//!
//! This runs heuristics, queries knowledge bases, applies belief policy, and
//! builds the catalog, which is then handed to Core for persistence.

mod audit_log;
mod batch;
mod belief;
mod heuristics;
pub(crate) mod knowledge_base;
mod manifest;
#[cfg(feature = "corpus")]
mod probe;
mod release_words;
#[cfg(feature = "corpus")]
pub use probe::LastResortProbe;
#[cfg(feature = "corpus")]
pub(crate) use probe::probe_last_resorts;

use crate::AppError;
use audit_log::AuditLogger;
use batch::{Trials, search_batches};
use belief::{apply_belief_policy, deactivate_duplicate_claims};
use futures::{StreamExt, stream::FuturesUnordered};
use heuristics::{ReleaseMarkers, run_heuristics};
use knowledge_base::{
    EpisodeRun, KnowledgeBase, KnowledgeBaseError, RankedSearchFields, find_kb_search_fields,
};
use manifest::{build_works, enrich_with_kb_data, new_catalog};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Duration,
};
use tracing::warn;
use wanshitong_core::{
    CoreContext,
    commands::{NewCatalog, NewInterpretationError, NewMovie, NewTvSeries, record_catalog},
    domain::{
        Assertion, ClaimPredicate, ClaimSource, DataSourceUuid, Facts, FileFacts,
        InterpretationRowId, KbQueryEvent, PathId, Rank, Ranked, RankedClaimPredicate,
        SubjectIndex,
    },
    queries::{list_facts, list_interpretation_assertions},
};

/// Derives the interpretation's catalog and records it. What it couldn't do
/// along the way, and went on without, e.g. a knowledge base search that
/// failed, goes in `errors`, whether or not it got as far as the catalog.
pub(crate) async fn generate_catalog(
    ctx: &CoreContext,
    kb: Option<&dyn KnowledgeBase>,
    interpretation_id: InterpretationRowId,
    errors: &mut Vec<NewInterpretationError>,
) -> Result<(), AppError> {
    let Facts { files } = list_facts(ctx, interpretation_id).await?;
    let assertions = list_interpretation_assertions(ctx, interpretation_id).await?;
    let mut catalog = interpret_facts(&files, &assertions, kb, errors).await?;
    // The catalog holds only identified works. The files of the rest are among
    // its unidentified videos, and their claims stay in the audit logs.
    catalog.movies.retain(NewMovie::is_identified);
    catalog.tv_series.retain(NewTvSeries::is_identified);
    record_catalog(ctx, interpretation_id, catalog).await?;
    Ok(())
}

/// Derives the catalog of `files`, with `assertions` about them: every work
/// the heuristics and the knowledge base make out, identified by an external
/// ID or not. Fails only when the knowledge base is down, as
/// [`add_claims_from_knowledge_base`] tells; a search that failed on its own
/// goes in `errors`.
pub(crate) async fn interpret_facts(
    files: &[(PathId, FileFacts)],
    assertions: &[Assertion],
    kb: Option<&dyn KnowledgeBase>,
    errors: &mut Vec<NewInterpretationError>,
) -> Result<NewCatalog, KnowledgeBaseError> {
    let mut logger = AuditLogger::default();
    let mut claims = run_heuristics(files);
    add_assertion_claims(files, assertions, &mut claims);
    deactivate_duplicate_claims(&mut claims);
    if let Some(kb) = kb {
        add_claims_from_knowledge_base(kb, files, &mut logger, &mut claims, errors).await?;
    }
    apply_belief_policy(&mut claims);
    let audit_logs = logger.finish(claims.clone());
    let mut works = build_works(files, claims);
    if let Some(kb) = kb {
        enrich_with_kb_data(kb, &mut works, errors).await;
    }
    Ok(new_catalog(files, works, audit_logs))
}

/// Adds each assertion as a claim on the file it names, if that file is in the
/// interpretation. They go in before the knowledge base is queried, so that an
/// asserted ID brings along what the knowledge base knows about it, at nearly
/// the assertion's rank.
fn add_assertion_claims(
    files: &[(PathId, FileFacts)],
    assertions: &[Assertion],
    claims: &mut HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
) {
    let subjects: HashMap<(DataSourceUuid, &Path), SubjectIndex> = files
        .iter()
        .enumerate()
        .map(|(subject, (path, _))| ((path.data_source, path.rel_path.as_path()), subject))
        .collect();
    for assertion in assertions {
        for file in &assertion.files {
            let key = (file.data_source_uuid, file.rel_path.as_path());
            let Some(&subject) = subjects.get(&key) else {
                continue;
            };
            claims
                .entry(subject)
                .or_default()
                .push(RankedClaimPredicate::new(
                    ClaimSource::Assertion,
                    Rank::Assertion,
                    assertion.claim.clone(),
                ));
        }
    }
}

/// Searches the knowledge base for each file's claims. A search that fails
/// leaves its file with only its other claims, and is noted in `errors`.
/// Unless every search failed: then the knowledge base is down, and a catalog
/// without any of its answers would be a poor one to go on with.
async fn add_claims_from_knowledge_base(
    kb: &dyn KnowledgeBase,
    files: &[(PathId, FileFacts)],
    logger: &mut AuditLogger,
    claims: &mut HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
    errors: &mut Vec<NewInterpretationError>,
) -> Result<(), KnowledgeBaseError> {
    let parts_runtimes = parts_runtimes(files, claims);
    let release_markers = ReleaseMarkers::new();
    let mut fields = HashMap::new();
    for (&subject, subject_claims) in &*claims {
        let Some(mut search_fields) =
            find_kb_search_fields(subject_claims, |title| release_markers.in_title(title))
        else {
            continue;
        };
        search_fields.parts_runtime = parts_runtimes.get(&subject).copied();
        fields.insert(subject, search_fields);
    }
    for (subject, episodes) in season_episodes(files, &fields) {
        if let Some(search_fields) = fields.get_mut(&subject) {
            search_fields.season_episodes = Some(episodes);
        }
    }
    for (subject, run) in episode_runs(files, &fields) {
        if let Some(search_fields) = fields.get_mut(&subject) {
            search_fields.run = Some(run);
        }
    }
    let Trials {
        searched: tried,
        mut queries,
    } = search_batches(kb, files, claims, &mut fields).await;
    let mut workers = FuturesUnordered::new();
    for (subject, search_fields) in fields {
        if tried.contains_key(&subject) {
            continue;
        }
        workers.push(async move {
            let result = kb.search_for_claims(&search_fields).await;
            (subject, result)
        });
    }
    let mut results = Vec::new();
    while let Some(result) = workers.next().await {
        results.push(result);
    }
    results.extend(tried);
    let searched = results.len();
    let mut failed = 0;
    let mut first_error = None;
    for (subject, result) in results {
        let (kb_results, kb_queries) = match result {
            Ok(found) => found,
            Err(e) => {
                let (path, facts) = &files[subject];
                warn!(
                    "Searching the knowledge base for \"{}\" failed: {e:#}",
                    path.rel_path.display()
                );
                errors.push(NewInterpretationError {
                    file_id: Some(facts.row_id),
                    subject: format!(
                        "Searching the knowledge base for \"{}\"",
                        path.rel_path.display()
                    ),
                    error: format!("{e:#}"),
                });
                failed += 1;
                first_error.get_or_insert(e);
                continue;
            }
        };
        let subject_claims = claims.get_mut(&subject).unwrap();
        for (kb_result, kb_claims) in (0..).zip(kb_results) {
            subject_claims.extend(kb_claims.into_iter().map(|Ranked { rank, value }| {
                RankedClaimPredicate::new(
                    ClaimSource::KnowledgeBase { result: kb_result },
                    rank,
                    value,
                )
            }));
        }
        // A trial's queries without the file's own names that the search
        // with them didn't make too, e.g. the last resort's.
        let made: HashSet<String> = kb_queries.iter().filter_map(query_key).collect();
        let without_own = queries.remove(&subject).unwrap_or_default();
        let others = without_own
            .into_iter()
            .filter(|query| query_key(query).is_none_or(|key| !made.contains(&key)));
        for event in kb_queries.into_iter().chain(others) {
            logger.log_kb_query(subject, event);
        }
    }
    match first_error {
        Some(e) if failed == searched => {
            Err(e.context(format!("every knowledge base search failed ({failed})")))
        }
        _ => Ok(()),
    }
}

/// A query's identity, to tell it from another: its kind and what it asked.
fn query_key(query: &KbQueryEvent) -> Option<String> {
    serde_json::to_string(query).ok()
}

/// How long the parts of each subject's work last together: the sum of the
/// measured durations of the files in its directory that claim to be parts of
/// a work of the same title, e.g. a concert's `[01]` and `[02]`, if there are
/// several and each was measured.
fn parts_runtimes(
    files: &[(PathId, FileFacts)],
    claims: &HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
) -> HashMap<SubjectIndex, Duration> {
    let mut sets = HashMap::<(DataSourceUuid, &Path, &str), Vec<SubjectIndex>>::new();
    for (&subject, subject_claims) in claims {
        let active = || subject_claims.iter().filter(|claim| claim.active);
        if !active().any(|claim| matches!(claim.pred, ClaimPredicate::Part(_))) {
            continue;
        }
        let title = active()
            .filter_map(|claim| match &claim.pred {
                ClaimPredicate::MovieTitle(title) => Some((claim.rank, title.as_str())),
                _ => None,
            })
            .max_by_key(|&(rank, _)| rank);
        let path = &files[subject].0;
        if let (Some((_, title)), Some(dir)) = (title, path.rel_path.parent()) {
            sets.entry((path.data_source, dir, title))
                .or_default()
                .push(subject);
        }
    }
    let mut runtimes = HashMap::new();
    for parts in sets.into_values().filter(|parts| parts.len() > 1) {
        let durations: Option<Vec<Duration>> = parts
            .iter()
            .map(|&subject| files[subject].1.media.as_ref()?.duration())
            .collect();
        if let Some(durations) = durations {
            let total = durations.into_iter().sum();
            runtimes.extend(parts.into_iter().map(|subject| (subject, total)));
        }
    }
    runtimes
}

/// How many episodes of its season each episode's release has: the episode
/// numbers of the season that the videos of its release, a data source's
/// top directory, number, with the same series' name. Each is counted once,
/// as a release may hold an episode twice, e.g. in two qualities.
fn season_episodes(
    files: &[(PathId, FileFacts)],
    fields: &HashMap<SubjectIndex, RankedSearchFields>,
) -> HashMap<SubjectIndex, u32> {
    let mut seasons = HashMap::<_, (Vec<SubjectIndex>, HashSet<u32>)>::new();
    for (&subject, fields) in fields {
        let (Some(season), Some([first, last])) = (fields.season, fields.episode_range) else {
            continue;
        };
        let Some(series) = fields.series_names.iter().max_by_key(|name| name.rank) else {
            continue;
        };
        let path = &files[subject].0;
        let release = path
            .rel_path
            .components()
            .next()
            .filter(|_| path.rel_path.components().count() > 1);
        let (subjects, episodes) = seasons
            .entry((path.data_source, release, series.value.as_str(), season))
            .or_default();
        subjects.push(subject);
        episodes.extend(first..=last);
    }
    seasons
        .into_values()
        .flat_map(|(subjects, episodes)| {
            let count = episodes.len() as u32;
            subjects.into_iter().map(move |subject| (subject, count))
        })
        .collect()
}

/// The run of its release's episodes that each episode numbered without a
/// season is in: those of its release, a data source's top directory, with
/// the same series' name, if their numbers start from 1, with their family:
/// the runs of the release named as one of them and more, e.g. `mahoromatic
/// ~motto utsukushii mono~` beside `mahoromatic`.
fn episode_runs(
    files: &[(PathId, FileFacts)],
    fields: &HashMap<SubjectIndex, RankedSearchFields>,
) -> HashMap<SubjectIndex, EpisodeRun> {
    struct Run<'a> {
        name: &'a str,
        subjects: Vec<SubjectIndex>,
        episodes: HashSet<u32>,
    }
    let mut releases = HashMap::<_, HashMap<&str, Run>>::new();
    for (&subject, fields) in fields {
        let (None, Some([first, last])) = (fields.season, fields.absolute_episode_range) else {
            continue;
        };
        let Some(series) = fields.series_names.iter().max_by_key(|name| name.rank) else {
            continue;
        };
        let path = &files[subject].0;
        let release = path
            .rel_path
            .components()
            .next()
            .filter(|_| path.rel_path.components().count() > 1);
        let run = releases
            .entry((path.data_source, release))
            .or_default()
            .entry(series.value.as_str())
            .or_insert_with(|| Run {
                name: &series.value,
                subjects: Vec::new(),
                episodes: HashSet::new(),
            });
        run.subjects.push(subject);
        run.episodes.extend(first..=last);
    }
    let mut runs = HashMap::new();
    for release in releases.into_values() {
        let release: Vec<Run> = release
            .into_values()
            .filter(|run| run.episodes.contains(&1))
            .collect();
        // Each run's family: the run of the shortest name its own begins
        // with, by its place in the release.
        let first_of = |at: usize| {
            (0..release.len())
                .filter(|&other| release[at].name.starts_with(release[other].name))
                .min_by_key(|&other| release[other].name.len())
                .unwrap_or(at)
        };
        for (at, run) in release.iter().enumerate() {
            let first = first_of(at);
            let family: Vec<(String, u32)> = (0..release.len())
                .filter(|&other| first_of(other) == first)
                .map(|other| {
                    let run = &release[other];
                    (run.name.to_string(), run.episodes.len() as u32)
                })
                .collect();
            let episode_run = EpisodeRun {
                name: run.name.to_string(),
                episodes: run.episodes.len() as u32,
                last: run.episodes.iter().copied().max().unwrap_or(0),
                family: if family.len() > 1 { family } else { Vec::new() },
            };
            runs.extend(
                run.subjects
                    .iter()
                    .map(|&subject| (subject, episode_run.clone())),
            );
        }
    }
    runs
}

#[cfg(test)]
mod tests;
