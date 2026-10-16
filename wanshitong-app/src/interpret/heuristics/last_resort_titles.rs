//! Heuristics for titles and series' names that are only a last resort: a
//! file's own cut, split or spelled otherwise, looked up only if nothing
//! else finds a work (see `RankedSearchFields::last_resort_titles`), as
//! they'd as often find another.

use super::{
    AddWithRank, HeuristicView,
    names::{file_stem, split_camel_case},
};
use crate::title;
use regex::Regex;
use std::sync::LazyLock;
use wanshitong_core::domain::{ClaimPredicate, ClaimSource, RankedClaimPredicate, SubjectIndex};

/// Claims the titles of one word that `movie_title_without_release_noise`
/// cut files' names to, e.g. `aankhen` of `Aankhen.2002.1080p.AMZN.WEBDL`,
/// looked up only if nothing else finds a work, and only for a movie titled
/// so and of the file's year, if it says one, as one word names too little
/// for another.
pub(super) fn title_of_one_word(mut adder: AddWithRank, titles: Vec<(SubjectIndex, String)>) {
    for (subject, title) in titles {
        if let Some(mut adder) = adder.with_subject(subject) {
            adder.add_claim(ClaimPredicate::MovieTitle(title));
        }
    }
}

/// Claims a video's title, or its series' name, cut before what seems its
/// subtitle or its episode's, at a separator that sets one apart: ` ~`,
/// ` - `, ` (` or `「`, e.g. `oh my jump!` for `oh my jump! ~shōnen jump
/// chikyū wo sukū~`, as the knowledge base may know the work by its title
/// alone, or by another subtitle. Only a cut that leaves two words, or one
/// in a script without spaces, as one Latin word names too little. A knowledge
/// base looks it up only if nothing else finds a work, and takes only a work
/// titled as it begins (see `RankedSearchFields::last_resort_titles`).
///
/// Not words dropped one at a time: a title's last words are as often its
/// own as a subtitle's, and a few words left find other works.
///
/// After the heuristics that title and name, whose claims it goes by.
pub(super) fn title_before_subtitle(view: HeuristicView, mut adder: AddWithRank) {
    for (subject, (_, facts)) in view.files.iter().enumerate() {
        if !facts.has_video_streams() {
            continue;
        }
        let cuts: Vec<ClaimPredicate> = [false, true]
            .into_iter()
            .filter_map(|series| {
                let title = best_name(adder.claims_of(subject), series)?;
                let cut = cut_before_subtitle(title)?;
                Some(match series {
                    false => ClaimPredicate::MovieTitle(cut),
                    true => ClaimPredicate::TvSeriesName(cut),
                })
            })
            .collect();
        if let Some(mut adder) = adder.with_subject(subject) {
            for cut in cuts {
                adder.add_claim(cut);
            }
        }
    }
}

/// Claims a video's title, or its series' name, without the group's tag
/// that a lowercase scene name begins with, e.g. `red sonja` for
/// `veto-red.sonja.2025.720p.bluray.x264.mkv`. Only a tag of two letters or
/// more, as a title may begin with a letter and a hyphen, e.g. `x-men`. Like
/// [`title_before_subtitle`]'s, it's looked up only if nothing else finds a
/// work, as a title's first word may be joined to its second so too, e.g.
/// `spider-man`.
///
/// After the heuristics that title and name, whose claims it goes by.
pub(super) fn title_without_group_tag(view: HeuristicView, mut adder: AddWithRank) {
    static GROUP_TAG: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^([a-z0-9]{2,10})-[a-z0-9]").unwrap());
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        if !facts.has_video_streams() {
            continue;
        }
        let Some(stem) = file_stem(&path_id.rel_path) else {
            continue;
        };
        let Some(tag) = GROUP_TAG
            .captures(stem)
            .filter(|_| !stem.chars().any(char::is_uppercase))
            .map(|captures| format!("{}-", &captures[1]))
        else {
            continue;
        };
        let best = |series: bool| {
            best_name(adder.claims_of(subject), series)
                .and_then(|title| title.strip_prefix(tag.as_str()))
                .filter(|rest| title::has_words(rest))
                .map(str::to_string)
        };
        let claims: Vec<ClaimPredicate> = [
            best(false).map(ClaimPredicate::MovieTitle),
            best(true).map(ClaimPredicate::TvSeriesName),
        ]
        .into_iter()
        .flatten()
        .collect();
        if let Some(mut adder) = adder.with_subject(subject) {
            for claim in claims {
                adder.add_claim(claim);
            }
        }
    }
}

/// Claims a video's title, or its series' name, with its words written in
/// camel case split, e.g. `fuyu no sakura` for `fuyunosakura` of
/// `FuyuNoSakura.02.avi`, as a knowledge base knows few works by such a name.
/// Like [`title_before_subtitle`]'s, it's looked up only if nothing else
/// finds a work, as a word written so may be a name of its own, e.g.
/// `GaoGaiGar`.
///
/// After the heuristics that title and name, whose claims it goes by.
pub(super) fn title_split_at_capitals(view: HeuristicView, mut adder: AddWithRank) {
    static LETTERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{L}+").unwrap());
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        if !facts.has_video_streams() {
            continue;
        }
        let path = path_id.rel_path.to_string_lossy();
        // Each word in the path written in camel case, and its words.
        let camel: Vec<(String, String)> = LETTERS
            .find_iter(&path)
            .filter_map(|word| {
                let split = split_camel_case(word.as_str());
                (split != word.as_str())
                    .then(|| (word.as_str().to_lowercase(), split.to_lowercase()))
            })
            .collect();
        if camel.is_empty() {
            continue;
        }
        let split = |title: &str| {
            let words: Vec<&str> = title
                .split(' ')
                .map(|word| {
                    camel
                        .iter()
                        .find(|(whole, _)| *whole == word.to_lowercase())
                        .map_or(word, |(_, split)| split.as_str())
                })
                .collect();
            let split = words.join(" ");
            (split != title).then_some(split)
        };
        let mut claims = Vec::new();
        for series in [false, true] {
            let best = best_name(adder.claims_of(subject), series).and_then(split);
            claims.extend(best.map(|title| match series {
                false => ClaimPredicate::MovieTitle(title),
                true => ClaimPredicate::TvSeriesName(title),
            }));
        }
        if let Some(mut adder) = adder.with_subject(subject) {
            for claim in claims {
                adder.add_claim(claim);
            }
        }
    }
}

/// Claims a video's title, or its series' name, and those it's cut or split
/// to, with long vowels written short, e.g. `umi no ue no shinryojo` for
/// `Umi no Ue no Shinryoujo`, as a knowledge base may list a romanized title
/// only so, e.g. TMDB "Clinic on the Sea". Like [`title_before_subtitle`]'s,
/// it's looked up only if nothing else finds a work, as an English title
/// with doubled vowels, e.g. `Good`, isn't romanized.
///
/// After the heuristics whose claims it goes by.
pub(super) fn title_with_short_vowels(view: HeuristicView, adder: AddWithRank) {
    spelled_otherwise(view, adder, |title| {
        title::has_doubled_vowels(title).then(|| title::romaji_folded(title))
    });
}

/// Claims a video's title, or its series' name, and those it's cut, split
/// or spelled to, without the words `and` and `plus`, e.g. `billy mandys big
/// boogey adventure` for `Billy.and.Mandys.Big.Boogey.Adventure`, as a file's
/// name writes `&` or `+` so, or a comma, e.g. `tomato pepper and
/// depression` for "Tomato, Pepper, Depression", and TMDB's search then
/// finds nothing. Like [`title_before_subtitle`]'s, it's looked up only if
/// nothing else finds a work.
///
/// After the heuristics whose claims it goes by.
pub(super) fn title_without_and(view: HeuristicView, adder: AddWithRank) {
    spelled_otherwise(view, adder, |title| {
        let words: Vec<&str> = title.split_whitespace().collect();
        let kept: Vec<&str> = words
            .iter()
            .enumerate()
            .filter(|&(i, word)| i == 0 || !matches!(*word, "and" | "plus"))
            .map(|(_, word)| *word)
            .collect();
        (kept.len() < words.len() && kept.len() >= 2).then(|| kept.join(" "))
    });
}

/// Claims each of a video's title and its series' name, and of those it's
/// cut, split or spelled to as a last resort, as `respell` spells it, if it
/// does, as a last resort too.
fn spelled_otherwise(
    view: HeuristicView,
    mut adder: AddWithRank,
    respell: impl Fn(&str) -> Option<String>,
) {
    for (subject, (_, facts)) in view.files.iter().enumerate() {
        if !facts.has_video_streams() {
            continue;
        }
        let claims = adder.claims_of(subject);
        let last_resorts = claims
            .iter()
            .filter(|claim| claim.active && is_last_resort(claim))
            .filter_map(|claim| match &claim.pred {
                ClaimPredicate::MovieTitle(title) => Some((false, title.as_str())),
                ClaimPredicate::TvSeriesName(name) => Some((true, name.as_str())),
                _ => None,
            });
        let names = [false, true]
            .into_iter()
            .filter_map(|series| Some((series, best_name(claims, series)?)))
            .chain(last_resorts);
        let mut respelled: Vec<ClaimPredicate> = Vec::new();
        for (series, name) in names {
            let spelled = respell(name).map(|spelled| match series {
                false => ClaimPredicate::MovieTitle(spelled),
                true => ClaimPredicate::TvSeriesName(spelled),
            });
            if let Some(spelled) = spelled
                && !respelled.contains(&spelled)
            {
                respelled.push(spelled);
            }
        }
        if let Some(mut adder) = adder.with_subject(subject) {
            for claim in respelled {
                adder.add_claim(claim);
            }
        }
    }
}

/// A title cut before the first separator that sets a subtitle apart, as
/// [`title_before_subtitle`] cuts it, if two words are left, or one in a
/// script without spaces.
fn cut_before_subtitle(title: &str) -> Option<String> {
    let at = [" ~", " - ", " (", "「"]
        .iter()
        .filter_map(|separator| title.find(separator))
        .min()?;
    let cut = title[..at]
        .trim_end_matches([' ', '-', '~', '.', ':', '_'])
        .trim();
    let words = crate::title::words(cut);
    let enough = words.len() >= 2
        || words.len() == 1
            && !words
                .iter()
                .any(|word| word.chars().any(|c| c.is_ascii_alphabetic()));
    enough.then(|| cut.to_string())
}

/// A video's best title, or its series' name, of those not a last resort's,
/// to cut, split or spell otherwise.
fn best_name(claims: &[RankedClaimPredicate], series: bool) -> Option<&str> {
    claims
        .iter()
        .filter(|claim| claim.active && !is_last_resort(claim))
        .filter_map(|claim| match (&claim.pred, series) {
            (ClaimPredicate::MovieTitle(title), false) => Some((claim.rank, title)),
            (ClaimPredicate::TvSeriesName(name), true) => Some((claim.rank, name)),
            _ => None,
        })
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, name)| name.as_str())
}

/// Whether a claim is a last resort's title or series' name.
fn is_last_resort(claim: &RankedClaimPredicate) -> bool {
    matches!(claim.source, ClaimSource::Heuristic(heuristic) if heuristic.is_last_resort_title())
}
