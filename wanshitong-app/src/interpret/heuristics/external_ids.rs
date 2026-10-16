//! IDs written in names, e.g. `{tmdb-8077}`.

use super::{
    AddToSubject, AddWithRank, Done, HeuristicView, extras::in_extras_dir, names::file_stem,
    series_patterns::SeriesPatterns,
};
use regex::Regex;
use std::path::Path;
use wanshitong_core::domain::{ClaimPredicate, ExternalId, FileFacts, PathId};

/// Claims the IDs written in a video's name, or failing that in the nearest
/// of its directories' names that has one, as media servers and their
/// managers write them, e.g. `{tmdb-8077}` (Plex, Radarr) or `[tvdbid-81797]`
/// (Jellyfin). A disc's name is its directory's.
///
/// An episode's, by a marker in its name or as one of a run of its siblings,
/// e.g. `Berserk 01`, are its series'. Another video's are a movie's, but only
/// in its own name or its directory's, as a series' directory, e.g. `One Piece
/// (1999) [tvdbid-81797]`, holds its movies and OVAs too. A TVDB ID is only
/// ever a series'. Not an extra's, which is of its main file's work.
///
/// After the numbered runs, whose episodes it goes by.
pub(super) fn external_ids_from_names(
    view: HeuristicView,
    mut adder: AddWithRank,
    patterns: &SeriesPatterns,
    id_re: &Regex,
) {
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        let is_episode = patterns.is_episode(&path_id.rel_path)
            || adder.claims_of(subject).iter().any(|claim| {
                matches!(
                    claim.pred,
                    ClaimPredicate::EpisodeRange(_) | ClaimPredicate::AbsoluteEpisodeRange(_)
                )
            });
        if let Some(adder) = adder.with_subject(subject) {
            let _ = external_ids_in_names(patterns, id_re, path_id, facts, is_episode, adder);
        }
    }
}

/// The IDs that one video's names say, as [`external_ids_from_names`] claims
/// them.
fn external_ids_in_names(
    patterns: &SeriesPatterns,
    id_re: &Regex,
    path_id: &PathId,
    facts: &FileFacts,
    is_episode: bool,
    mut adder: AddToSubject,
) -> Done {
    let path = &path_id.rel_path;
    if !facts.has_video_streams() || patterns.is_extra(path) || in_extras_dir(path) {
        return None;
    }
    // Its own name, then its directories' up to its data source's root.
    let (own, directory) = match &facts.disc {
        Some(disc) => {
            let root = Path::new(&disc.root);
            (root.file_name()?.to_str()?, root.parent())
        }
        None => (file_stem(path)?, path.parent()),
    };
    let names = std::iter::once(own).chain(
        directory
            .into_iter()
            .flat_map(Path::ancestors)
            .filter_map(|dir| dir.file_name()?.to_str()),
    );
    for (depth, name) in names.enumerate() {
        let ids = ids_in_name(id_re, name);
        if ids.is_empty() {
            continue;
        }
        for id in ids {
            match (is_episode, &id) {
                (true, _) => adder.add_claim(ClaimPredicate::TvSeriesExternalId(id)),
                (false, ExternalId::Tvdb(_)) => (),
                (false, _) if depth <= 1 => adder.add_claim(ClaimPredicate::MovieExternalId(id)),
                (false, _) => (),
            }
        }
        return None;
    }
    None
}

/// The IDs written in a name, e.g. TMDB's 8077 in `Alien³ (1992) {tmdb-8077}`.
fn ids_in_name(id_re: &Regex, name: &str) -> Vec<ExternalId> {
    id_re
        .captures_iter(name)
        .filter_map(|c| {
            if let Some(imdb) = c.name("imdb") {
                return Some(ExternalId::Imdb(imdb.as_str().to_lowercase()));
            }
            let id = c["id"].parse().ok()?;
            match c["db"].to_lowercase().as_str() {
                "tmdb" => Some(ExternalId::Tmdb(id)),
                "tvdb" => Some(ExternalId::Tvdb(id)),
                _ => None,
            }
        })
        .collect()
}

/// An ID in a name, e.g. `{tmdb-8077}` or `{imdb-tt0103644}` as Plex and Radarr
/// write them, or `[tvdbid-81797]` as Jellyfin does. An IMDb ID is `tt` and
/// digits, the others digits.
pub(super) const NAME_ID_REGEX: &str =
    r"(?i)[\[{](?:(?<db>tmdb|tvdb)(?:id)?-(?<id>\d+)|imdb(?:id)?-(?<imdb>tt\d+))[\]}]";
