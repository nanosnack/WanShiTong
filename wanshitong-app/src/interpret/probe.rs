//! What each last resort finds alone, to measure them against a corpus.

use super::{
    ReleaseMarkers, deactivate_duplicate_claims,
    knowledge_base::{self, KnowledgeBaseError, find_kb_search_fields},
    run_heuristics,
};
use wanshitong_core::domain::{
    ClaimPredicate, ClaimSource, ExternalId, FileFacts, PathId, SubjectIndex,
};

/// What a last resort found for a video, alone: the strategy, the title it
/// tried and the heuristic that claimed it, if a last-resort title's, and
/// the works it found, e.g. `movie:tmdb:603`.
pub struct LastResortProbe {
    pub path: std::path::PathBuf,
    pub strategy: String,
    pub title: Option<String>,
    pub heuristic: Option<String>,
    pub works: Vec<String>,
}

/// For up to `max_videos` of `files`' videos that only the last resorts
/// could find, what each last resort finds alone (see
/// `knowledge_base::probe_last_resorts`). Not as an interpretation runs:
/// without a batch's shared title or its parts' runtimes.
pub(crate) async fn probe_last_resorts(
    files: &[(PathId, FileFacts)],
    kb: &dyn knowledge_base::KnowledgeBaseApi,
    max_videos: usize,
) -> Result<Vec<LastResortProbe>, KnowledgeBaseError> {
    let mut claims = run_heuristics(files);
    deactivate_duplicate_claims(&mut claims);
    let release_markers = ReleaseMarkers::new();
    let mut subjects: Vec<SubjectIndex> = claims.keys().copied().collect();
    subjects.sort();
    let mut probed = Vec::new();
    let mut videos = 0;
    for subject in subjects {
        if videos == max_videos {
            break;
        }
        let subject_claims = &claims[&subject];
        let Some(fields) =
            find_kb_search_fields(subject_claims, |title| release_markers.in_title(title))
        else {
            continue;
        };
        let Some(probes) = knowledge_base::probe_last_resorts(kb, &fields).await? else {
            continue;
        };
        videos += 1;
        let heuristic_of = |title: &str| {
            subject_claims.iter().find_map(|claim| {
                let named = matches!(&claim.pred,
                    ClaimPredicate::MovieTitle(t) | ClaimPredicate::TvSeriesName(t) if t == title);
                match claim.source {
                    ClaimSource::Heuristic(heuristic)
                        if named && heuristic.is_last_resort_title() =>
                    {
                        Some(format!("{heuristic:?}"))
                    }
                    _ => None,
                }
            })
        };
        for (strategy, title, results) in probes {
            let works = results
                .iter()
                .filter_map(|result| {
                    result.iter().find_map(|claim| match &claim.value {
                        ClaimPredicate::MovieExternalId(ExternalId::Tmdb(id)) => {
                            Some(format!("movie:tmdb:{id}"))
                        }
                        ClaimPredicate::TvSeriesExternalId(ExternalId::Tmdb(id)) => {
                            Some(format!("series:tmdb:{id}"))
                        }
                        _ => None,
                    })
                })
                .collect();
            probed.push(LastResortProbe {
                path: files[subject].0.rel_path.clone(),
                heuristic: title.as_deref().and_then(heuristic_of),
                strategy,
                title,
                works,
            });
        }
    }
    Ok(probed)
}
