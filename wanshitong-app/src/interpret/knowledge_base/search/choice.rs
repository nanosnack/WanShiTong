//! Choosing among a search's results: which is titled as searched for, of the file's year, or otherwise the file's work, without asking the knowledge base anything more.

use super::*;

/// Whether a knowledge base's title is a file's by `same`, e.g.
/// [`title::same_words`], with long vowels written alike: one romanization
/// of a Japanese title may write one doubled, and another short.
pub(super) fn titled_as(same: fn(&str, &str) -> bool, kb_title: &str, title: &str) -> bool {
    same(
        &title::romaji_folded(kb_title),
        &title::romaji_folded(title),
    )
}

/// The movie searched for, when the file's runtime can't say: the one
/// [`titled_of_year`], or else the knowledge base's first.
pub(super) fn choose_movie_result<'a>(
    query: &str,
    year: Option<u32>,
    results: &'a [MovieRecord],
) -> Option<&'a MovieRecord> {
    titled_of_year(query, year, results, |movie| {
        ([&movie.title, &movie.original_title], movie.release_year)
    })
    .or(results.first())
}

/// The first of a search's results titled what was searched for and of the
/// file's year, give or take one, as a festival's and a release's often are:
/// e.g. 2018's "Halloween" for `halloween` of 2018, not 1978's, which a
/// search by year finds first for its re-release, nor "Friday the 13th Part
/// 2" for `friday the 13th` of 1980. Both the title and the year: either
/// alone often belongs to another work too, e.g. a remake's title, or a
/// spin-off's year. None if the file says no year.
pub(super) fn titled_of_year<'a, T>(
    query: &str,
    year: Option<u32>,
    results: &'a [T],
    titles_and_year: impl Fn(&T) -> ([&String; 2], Option<u32>),
) -> Option<&'a T> {
    let year = year?;
    results.iter().find(|result| {
        let (titles, result_year) = titles_and_year(result);
        titles.iter().any(|title| title::same_words(title, query))
            && result_year.is_some_and(|result_year| result_year.abs_diff(year) <= 1)
    })
}

/// The series searched for: the one [`titled_of_year`], or else one titled
/// what was searched for, or else that and more, e.g. a subtitle, over one
/// it's only part of: "After Life" for "after life", not "Life After Life".
/// Otherwise, the knowledge base's first.
pub(super) fn choose_series_result<'a>(
    query: &str,
    year: Option<u32>,
    results: &'a [SeriesRecord],
) -> Option<&'a SeriesRecord> {
    titled_of_year(query, year, results, |series| {
        ([&series.name, &series.original_name], series.first_air_year)
    })
    .or_else(|| titled_series_result(query, results))
    .or(results.first())
}

/// The first series titled what was searched for, or that and a subtitle,
/// e.g. "Food Wars! Shokugeki no Soma" for `food wars`; or else the first
/// titled that and more: "Alone" for `alone`, though "Alone Together", which
/// goes on to another name, comes first.
pub(super) fn titled_series_result<'a>(
    query: &str,
    results: &'a [SeriesRecord],
) -> Option<&'a SeriesRecord> {
    let titled = |same: fn(&str, &str) -> bool| {
        results
            .iter()
            .find(|result| same(&result.name, query) || same(&result.original_name, query))
    };
    titled(title::is_or_has_subtitle).or_else(|| titled(title::begins_with))
}

/// Of several series named as searched for, the first whose other title is
/// another name the file gives its series, e.g. Netflix's "My Family", whose
/// original title is "Storia della mia famiglia", for a file of `My Family
/// AKA Storia della mia famiglia S02`, not the BBC's listed first. Not a
/// name of the same words, e.g. `food wars` beside `food wars!`, which every
/// series named so is known as. Otherwise the one `chosen`.
pub(super) fn known_also_as<'r>(
    name: &str,
    chosen: &'r SeriesRecord,
    results: &'r [SeriesRecord],
    others: &[&str],
) -> &'r SeriesRecord {
    results
        .iter()
        .filter(|result| {
            title::same_words(&result.name, name) || title::same_words(&result.original_name, name)
        })
        .find(|result| {
            others
                .iter()
                .filter(|other| !title::same_words(other, name))
                .any(|other| title::same_words(&result.original_name, other))
        })
        .unwrap_or(chosen)
}

/// How many series named alike are looked up for the file's season: more
/// are rarely the file's.
pub(super) const SAME_NAMED_SERIES: usize = 4;
