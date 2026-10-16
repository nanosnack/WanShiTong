//! Extras: videos named like one, or in a directory of them.

use super::{
    AddToSubject, AddWithRank, Done, HeuristicView,
    names::{file_stem, parse_movie_title, remove_square_brackets},
    series_patterns::SeriesPatterns,
};
use crate::title;
use regex::Regex;
use std::{collections::HashMap, ffi::OsStr, path::Path};
use wanshitong_core::domain::{
    ClaimPredicate, FileFacts, FileRole, PathId, RankedClaimPredicate, SubjectIndex,
};

/// Claims that a video is an extra: one named like one, e.g. `NCOP` or
/// `[CM01]`, or in a directory of extras, e.g. `Extras` or
/// `Openings & Endings`. Whose extra it is is up to other heuristics.
pub(super) fn extra_role(
    patterns: &SeriesPatterns,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if facts.has_video_streams() && patterns.is_extra(&path_id.rel_path) {
        adder.add_claim(ClaimPredicate::FileRole(FileRole::Extra));
    }
    None
}

/// Claims that an extra no series has, e.g. `[Group][Title][PV][01]` in a
/// movie's release's `PV/`, is the extra of the movie in the nearest
/// directory above it that holds other videos: its only one, or of several,
/// e.g. a trilogy's, the one titled as the extra's name says, e.g. `Title`
/// of `[Group][Title][menu]` beside `[Group][Title][1080p]`. Not where those
/// videos are a series' episodes, whose extras are the series'.
///
/// After the heuristics that give an extra its series or make it a special,
/// whose claims it goes by.
pub(super) fn extras_of_nearby_movie(
    view: HeuristicView,
    mut adder: AddWithRank,
    patterns: &SeriesPatterns,
    movie_title_re: &Regex,
) {
    let of_series = |claims: &[RankedClaimPredicate]| {
        claims.iter().any(|claim| {
            matches!(
                claim.pred,
                ClaimPredicate::TvSeriesName(_)
                    | ClaimPredicate::SpecialOf(_)
                    | ClaimPredicate::EpisodeRange(_)
                    | ClaimPredicate::AbsoluteEpisodeRange(_)
                    | ClaimPredicate::ExtraOf(_)
            )
        })
    };
    // The videos of each directory that aren't extras.
    let mut features = HashMap::<&Path, Vec<SubjectIndex>>::new();
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        let path = &path_id.rel_path;
        if facts.is_video()
            && !patterns.is_extra(path)
            && let Some(dir) = path.parent()
        {
            features.entry(dir).or_default().push(subject);
        }
    }
    // A name's title: the words outside its tags, or else its tag after the
    // release group's.
    let title_of = |subject: SubjectIndex| -> Option<String> {
        let stem = file_stem(&view.files[subject].0.rel_path)?;
        if remove_square_brackets(stem)
            .chars()
            .any(char::is_alphabetic)
        {
            Some(parse_movie_title(movie_title_re, stem)?.title)
        } else {
            Some(patterns.tag_title(stem)?.to_string())
        }
    };
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        let path = &path_id.rel_path;
        if !facts.has_video_streams()
            || !patterns.is_extra(path)
            || of_series(adder.claims_of(subject))
        {
            continue;
        }
        let Some(candidates) = path.ancestors().skip(1).find_map(|dir| features.get(dir)) else {
            continue;
        };
        if candidates
            .iter()
            .any(|&candidate| of_series(adder.claims_of(candidate)))
        {
            continue;
        }
        let main = match candidates.as_slice() {
            [main] => *main,
            _ => {
                let Some(title) = title_of(subject) else {
                    continue;
                };
                let mut titled = candidates.iter().filter(|&&candidate| {
                    title_of(candidate).is_some_and(|other| title::same_words(&title, &other))
                });
                match (titled.next(), titled.next()) {
                    (Some(&main), None) => main,
                    _ => continue,
                }
            }
        };
        let main_path = view.files[main].0.rel_path.to_string_lossy().into_owned();
        if let Some(mut adder) = adder.with_subject(subject) {
            adder.add_claim(ClaimPredicate::ExtraOf(main_path));
            adder.add_claim(ClaimPredicate::FileRole(FileRole::Extra));
        }
    }
}

pub(super) fn in_extras_dir(path: &Path) -> bool {
    path.parent()
        .is_some_and(|dir| dir.components().any(|c| is_extras_dir_name(c.as_os_str())))
}

/// Whether a directory's name is one of [`EXTRAS_DIR_NAMES`], e.g. `SPs`.
pub(super) fn is_extras_dir_name(name: &OsStr) -> bool {
    EXTRAS_DIR_NAMES
        .iter()
        .any(|e| name.eq_ignore_ascii_case(e))
}

/// Whether a directory's name is one of [`EXTRAS_DIR_NAMES`], or has a word
/// for extras, e.g. `Extras`, `Show Name Openings & Endings` or `OP & ED`.
/// Plural words, so that titles like `The Extra` or `Happy Ending` aren't
/// taken for extras. Only for a series' files: a movie's directory is named
/// after it, so its title's words would be taken for extras', e.g.
/// `Happy Endings (2005)`.
///
/// Not what a release's name says it includes besides its episodes or movie:
/// a tag, e.g. `[Extras]` in `Cowboy Bebop [Complete] [Extras] [x264]`, or an
/// extras word joined on, e.g. `+ Extras` in `The Help (2011) + Extras`.
pub(super) fn is_extras_dir(dir: &Path) -> bool {
    let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if is_extras_dir_name(name.as_ref()) {
        return true;
    }
    let mut words = Vec::<String>::new();
    let mut word = String::new();
    // How deep in tags, e.g. `[...]` or `(...)`, the next word is.
    let mut depth = 0usize;
    // Whether the next word is joined on with `+` or `&`.
    let mut joined = false;
    for c in name.chars().chain([' ']) {
        if c.is_alphanumeric() {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            let w = std::mem::take(&mut word).to_lowercase();
            let included = depth > 0 || joined && EXTRAS_DIR_WORDS.contains(&w.as_str());
            if !included {
                words.push(w);
            }
            joined = false;
        }
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            '+' | '&' => joined = true,
            _ => (),
        }
    }
    let has = |word: &str| words.iter().any(|w| w == word);
    EXTRAS_DIR_WORDS.iter().any(|word| has(word)) || (has("op") && has("ed"))
}

/// Whole names of directories of extras, e.g. `Extras` or Zork-Glorp's `SPs`.
/// Unlike [`EXTRAS_DIR_WORDS`], they can be singular, e.g. `Extra`, or
/// otherwise too common to look for in longer names, e.g. `Other`. A movie's
/// and a series' extras are both found by them; see [`is_extras_dir`].
const EXTRAS_DIR_NAMES: &[&str] = &[
    "behind the scenes",
    "bonus",
    "deleted scenes",
    "extra",
    "extras",
    "featurette",
    "featurettes",
    "interviews",
    "other",
    "sample",
    "scenes",
    "shorts",
    "special features",
    "sps",
    "trailers",
];

/// Words in the names of directories of extras. See [`is_extras_dir`].
const EXTRAS_DIR_WORDS: &[&str] = &[
    "bonus",
    "creditless",
    "endings",
    "extras",
    "featurettes",
    "nced",
    "ncop",
    "openings",
];
