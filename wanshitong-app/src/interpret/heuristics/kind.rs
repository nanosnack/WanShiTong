//! What kind of work a file is of, a movie or a series, by its other claims.

use super::{AddWithRank, HeuristicView};
use std::{collections::HashMap, path::Path};
use wanshitong_core::domain::{
    ClaimPredicate, ClaimSource, DataSourceUuid, FileRole, Heuristic, RankedClaimPredicate,
    WorkKind,
};

/// Claims the kind of work a video is of, by the claims made of it before: a
/// series' if it's numbered as an episode or a season's, a movie's if it's a
/// release's main file and says its year. Neither if it's both, or neither.
/// Of labeled releases, 99.9% of those it claims a kind of are of that
/// kind, and 96% of movies and 99% of series get one.
///
/// After the heuristics whose claims it goes by.
pub(super) fn kind_of_work(view: HeuristicView, mut adder: AddWithRank) {
    for (subject, (_, facts)) in view.files.iter().enumerate() {
        if !facts.has_video_streams() {
            continue;
        }
        let claims = adder.claims_of(subject);
        let has = |is: &dyn Fn(&ClaimPredicate) -> bool| {
            claims.iter().any(|claim| claim.active && is(&claim.pred))
        };
        let episode = has(&|pred| {
            matches!(
                pred,
                ClaimPredicate::EpisodeRange(_)
                    | ClaimPredicate::AbsoluteEpisodeRange(_)
                    | ClaimPredicate::Season(_)
            )
        });
        let main = claims.iter().any(|claim| {
            claim.active
                && claim.pred == ClaimPredicate::FileRole(FileRole::Main)
                && matches!(
                    claim.source,
                    ClaimSource::Heuristic(
                        Heuristic::ReleaseName
                            | Heuristic::MovieDirectory
                            | Heuristic::Bdmv
                            | Heuristic::Dvd
                    )
                )
        });
        let dated = has(&|pred| matches!(pred, ClaimPredicate::MovieReleaseYear(_)));
        let kind = match (episode, main && dated) {
            (true, false) => WorkKind::TvSeries,
            (false, true) => WorkKind::Movie,
            _ => continue,
        };
        if let Some(mut adder) = adder.with_subject(subject) {
            adder.add_claim(ClaimPredicate::WorkKind(kind));
        }
    }
}

/// Claims a video file with no kind of its own is a movie's if it runs at
/// least [`MOVIE_MINUTES`]. Of a library's videos without one, nearly all that
/// long are movies, and the rest TV movies or specials. Not a series' if it's
/// short: so are movies' extras. Nor a disc's, whose longest title may play
/// every episode on it.
pub(super) fn kind_of_duration(view: HeuristicView, mut adder: AddWithRank) {
    for (subject, (_, facts)) in view.files.iter().enumerate() {
        let long = facts
            .media
            .as_ref()
            .and_then(|media| media.container.as_ref()?.duration_ms)
            .is_some_and(|ms| ms >= MOVIE_MINUTES * 60_000);
        if long
            && facts.has_video_streams()
            && kind_of(adder.claims_of(subject)).is_none()
            && let Some(mut adder) = adder.with_subject(subject)
        {
            adder.add_claim(ClaimPredicate::WorkKind(WorkKind::Movie));
        }
    }
}

/// The least a video runs to be a movie's by its duration alone. Of a
/// library's episodes, fewer than 1 in 400 run longer.
const MOVIE_MINUTES: i64 = 75;

/// Claims a video with no kind of its own is a series' if most of the videos
/// near it with one are, e.g. an extra beside a release's season folders, or
/// a special among its episodes: those of the outermost folder above it that
/// holds any, so a whole release's rather than a subfolder's of a few. Not
/// beyond a data source's folders, each a release or a collection of them,
/// into the data source's own, where videos are near only by where they're
/// kept.
///
/// Never a movie's: what's beside a movie is as often another work, e.g. a
/// series' OVAs packed with its movie, or else the movie's own extras.
///
/// After [`kind_of_work`], whose claims it goes by.
pub(super) fn kind_of_siblings(view: HeuristicView, mut adder: AddWithRank) {
    // Each folder's videos of a series' kind less those of a movie's, counted
    // in every near folder above them.
    let mut series_over_movies = HashMap::<(DataSourceUuid, &Path), isize>::new();
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        if !facts.has_video_streams() {
            continue;
        }
        let Some(kind) = kind_of(adder.claims_of(subject)) else {
            continue;
        };
        for dir in near_folders(&path_id.rel_path) {
            *series_over_movies
                .entry((path_id.data_source, dir))
                .or_default() += match kind {
                WorkKind::Movie => -1,
                WorkKind::TvSeries => 1,
            };
        }
    }
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        if !facts.has_video_streams() || kind_of(adder.claims_of(subject)).is_some() {
            continue;
        }
        let near = near_folders(&path_id.rel_path)
            .filter_map(|dir| series_over_movies.get(&(path_id.data_source, dir)))
            .last();
        if near.is_some_and(|&more| more > 0)
            && let Some(mut adder) = adder.with_subject(subject)
        {
            adder.add_claim(ClaimPredicate::WorkKind(WorkKind::TvSeries));
        }
    }
}

/// The folders whose videos a file's are near: each above it, up to the data
/// source's folder it's in, as the data source's own holds independent
/// releases.
fn near_folders(path: &Path) -> impl Iterator<Item = &Path> {
    path.ancestors()
        .skip(1)
        .filter(|dir| !dir.as_os_str().is_empty())
}

/// The kind of work claimed of a file, if any.
fn kind_of(claims: &[RankedClaimPredicate]) -> Option<WorkKind> {
    claims.iter().find_map(|claim| match claim.pred {
        ClaimPredicate::WorkKind(kind) if claim.active => Some(kind),
        _ => None,
    })
}
