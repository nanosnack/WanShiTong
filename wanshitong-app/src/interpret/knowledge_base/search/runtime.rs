//! Matching a file's measured runtime against the runtimes of a search's movies.

use super::*;

/// Whether a file is about as long as an episode the knowledge base says is
/// `minutes` long: within [`SPECIAL_RUNTIME_TOLERANCE_MINUTES`], or a tenth
/// of it.
pub(super) fn runtime_is_close(runtime: Duration, minutes: u32) -> bool {
    let file_minutes = (runtime.as_secs() as f32 / 60.0).round();
    let tolerance = (minutes as f32 / 10.0).max(SPECIAL_RUNTIME_TOLERANCE_MINUTES);
    (file_minutes - minutes as f32).abs() <= tolerance
}

/// The movie that the result at `index` is an entry for a cut of, and the
/// edition, when it's titled as one, e.g. `Kingdom Of Heaven (Director's Cut)`,
/// and the movie is among the results: the one titled so, released closest to
/// it.
pub(super) fn cut_of_another(details: &[MovieRecord], index: usize) -> Option<(usize, String)> {
    let cut = &details[index];
    [&cut.title, &cut.original_title]
        .into_iter()
        .find_map(|cut_title| {
            let (title, edition) = edition::of_title(cut_title)?;
            let movie = details
                .iter()
                .enumerate()
                .filter(|&(i, movie)| i != index && title::same_words(&movie.title, title))
                .min_by_key(|(_, movie)| match (movie.release_year, cut.release_year) {
                    (Some(a), Some(b)) => a.abs_diff(b),
                    _ => u32::MAX,
                })?;
            Some((movie.0, edition.label))
        })
}

/// Whether evidence beyond runtime supports matching the file to a movie of
/// the same length: a year in the file's name within a year of the movie's
/// (a festival premiere and a release can differ by one), or a title that
/// covers most of the movie's title and isn't just a number.
pub(super) fn corroborates_runtime_match(
    query: &str,
    file_year: Option<u32>,
    movie_titles: [&str; 2],
    movie_year: Option<u32>,
) -> bool {
    let years_agree = matches!((file_year, movie_year), (Some(a), Some(b)) if a.abs_diff(b) <= 1);
    let titled = title::has_words(query)
        && movie_titles
            .iter()
            .any(|movie_title| title::coverage(query, movie_title) >= MIN_TITLE_COVERAGE);
    years_agree || titled
}

/// A search result, with what matching it needs.
#[derive(Clone, Copy)]
pub(super) struct Candidate<'a> {
    /// The title and original title.
    pub(super) titles: [&'a str; 2],
    pub(super) runtime_minutes: Option<u32>,
    /// Whether it's of the file's year, give or take one.
    pub(super) year_agrees: bool,
}

#[derive(Debug, PartialEq)]
pub(super) struct RuntimeMatch {
    /// The candidate's position in the results.
    pub(super) index: usize,
    pub(super) minute_diff: i64,
    pub(super) similarity: f32,
}

/// Of the candidates as long as the file, within
/// [`RUNTIME_TOLERANCE_MINUTES`], the one titled most like what was searched
/// for, then of the file's year, then the closest in length, then the first.
/// A year says more than a minute or two, as files run a minute or two off
/// the knowledge base's runtimes, e.g. a "White Snake" of 2019 and 99 minutes
/// for a file of 2019 and 102, rather than one of 2021 and 101.
///
/// Runtime is a *strong* signal, but not enough on its own: a series' movies
/// can be about as long as each other. And TMDB generally doesn't publish
/// metadata about e.g. director's cuts.
pub(super) fn best_runtime_match<'a>(
    query: &str,
    file_runtime: Duration,
    candidates: impl IntoIterator<Item = Candidate<'a>>,
    fits: impl Fn(usize, i64) -> bool,
) -> Option<RuntimeMatch> {
    candidates
        .into_iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let RuntimeFit::Match { minute_diff } =
                runtime_fit(file_runtime, candidate.runtime_minutes)
            else {
                return None;
            };
            if !fits(index, minute_diff) {
                return None;
            }
            Some((
                candidate.year_agrees,
                RuntimeMatch {
                    index,
                    minute_diff,
                    similarity: title_similarity(query, candidate.titles),
                },
            ))
        })
        .min_by(|(a_year, a), (b_year, b)| {
            b.similarity
                .total_cmp(&a.similarity)
                .then(b_year.cmp(a_year))
                .then(a.minute_diff.cmp(&b.minute_diff))
                .then(a.index.cmp(&b.index))
        })
        .map(|(_, matched)| matched)
}

#[derive(Debug, PartialEq)]
pub(super) struct Unmatched {
    /// The candidate's position in the results.
    pub(super) index: usize,
    pub(super) fit: RuntimeFit,
    pub(super) similarity: f32,
}

/// Of the candidates that aren't as long as the file, the one titled most like
/// what was searched for, then the first, unless its runtime rules out the
/// file being it. None if every candidate's does.
pub(super) fn best_unmatched_candidate<'a>(
    query: &str,
    file_runtime: Duration,
    candidates: impl IntoIterator<Item = Candidate<'a>>,
) -> Option<Unmatched> {
    candidates
        .into_iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let fit = runtime_fit(file_runtime, candidate.runtime_minutes);
            matches!(fit, RuntimeFit::OtherCut | RuntimeFit::Unknown).then(|| Unmatched {
                index,
                fit,
                similarity: title_similarity(query, candidate.titles),
            })
        })
        .min_by(|a, b| {
            b.similarity
                .total_cmp(&a.similarity)
                .then(a.index.cmp(&b.index))
        })
}

/// How a movie's runtime on the knowledge base compares with a file's.
#[derive(Debug, PartialEq)]
pub(super) enum RuntimeFit {
    /// As long as the file, within [`RUNTIME_TOLERANCE_MINUTES`].
    Match { minute_diff: i64 },
    /// Not as long, but the file could be another cut of the movie, e.g. an
    /// extended edition, whose runtime knowledge bases generally don't list.
    OtherCut,
    /// The file is far too short or long to be the movie, e.g. an episode or
    /// a trailer.
    Contradicts,
    /// The knowledge base doesn't say how long the movie is.
    Unknown,
}

pub(super) fn runtime_fit(file_runtime: Duration, runtime_minutes: Option<u32>) -> RuntimeFit {
    let Some(minutes) = runtime_minutes else {
        return RuntimeFit::Unknown;
    };
    let file_minutes = file_runtime.as_secs().div_ceil(60) as i64;
    let minute_diff = (file_minutes - i64::from(minutes)).abs();
    let ratio = file_minutes as f32 / minutes as f32;
    if minute_diff < RUNTIME_TOLERANCE_MINUTES {
        RuntimeFit::Match { minute_diff }
    } else if (MIN_CUT_RATIO..=MAX_CUT_RATIO).contains(&ratio) {
        RuntimeFit::OtherCut
    } else {
        RuntimeFit::Contradicts
    }
}

/// How much a file's runtime may differ from a movie's, in minutes, for it to
/// match, if the file's title or year agrees with the movie's too.
pub(super) const RUNTIME_TOLERANCE_MINUTES: i64 = 5;
/// How much it may differ to match on its own: in a library, 88% of files
/// are within two minutes of TMDB's runtime, and most of the rest are other
/// cuts or TV movies timed with their ads.
pub(super) const RUNTIME_CLOSE_MINUTES: i64 = 2;
/// The bonus for a runtime match that something else corroborates, e.g. the
/// year. It outranks the strongest evidence that a file is an episode.
pub(super) const CORROBORATED_RUNTIME_BONUS: f32 = 60.0;
/// The bonus for a runtime match that nothing else does. It outranks the
/// search's other results, but not evidence that the file is an episode.
pub(super) const UNCORROBORATED_RUNTIME_BONUS: f32 = 3.0;
/// The bonus for each minute within [`RUNTIME_TOLERANCE_MINUTES`] that a
/// movie is closer to the file's runtime.
pub(super) const RUNTIME_CLOSENESS_BONUS: f32 = 0.2;
/// How much of a movie's title a file's title needs to have to corroborate a
/// runtime match, e.g. 7 of the 8 words of
/// `Code Geass: Lelouch of the Rebellion I - Initiation`.
pub(super) const MIN_TITLE_COVERAGE: f32 = 0.75;
/// How much shorter than the movie on the knowledge base a file may be, as a
/// share of its runtime, and still be a cut of it, e.g. a TV edit or a PAL
/// speed-up. An episode or an extra is far shorter.
pub(super) const MIN_CUT_RATIO: f32 = 0.75;
/// How much longer, e.g. Das Boot's director's cut is 1.4 times its
/// theatrical cut.
pub(super) const MAX_CUT_RATIO: f32 = 2.0;
/// How many of a search's results are checked against the file's runtime.
pub(super) const RUNTIME_CANDIDATES: usize = 5;
