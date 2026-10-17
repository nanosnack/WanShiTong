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

/// A search result, with what choosing it needs.
#[derive(Clone, Copy)]
pub(super) struct Candidate<'a> {
    /// The title and original title, or for an entry for a cut of another
    /// movie, e.g. `Kingdom Of Heaven (Director's Cut)`, that movie's.
    pub(super) titles: [&'a str; 2],
    pub(super) runtime_minutes: Option<u32>,
    /// How many years it's from the file's, if both are known.
    pub(super) years_off: Option<u32>,
}

#[derive(Debug, PartialEq)]
pub(super) struct Chosen {
    /// The candidate's position in the results.
    pub(super) index: usize,
    pub(super) fit: RuntimeFit,
    pub(super) similarity: f32,
}

/// Of the candidates the file could be, the one that [`match_score`] scores
/// highest, then the first. None if every candidate's runtime rules out the
/// file being it, e.g. an episode's numbers finding features.
///
/// Title, runtime and year compete, rather than any one gating the others:
/// a runtime match titled a little worse loses to a movie titled as searched
/// for that the file is likely another cut of, e.g. a TV edit of "Wumbo" at
/// 96 minutes to TMDB's 103 rather than "Outlaw Wumbo" at 96, but not to one
/// far shorter, e.g. Iñárritu's "Birdman or (The Unexpected Virtue of
/// Ignorance)" at 120 minutes for a 2-hour `Birdman.2014` rather than "The
/// Birdman" of 2014 at 76.
pub(super) fn best_candidate<'a>(
    query: &str,
    file_runtime: Duration,
    candidates: impl IntoIterator<Item = Candidate<'a>>,
) -> Option<Chosen> {
    candidates
        .into_iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let fit = runtime_fit(file_runtime, candidate.runtime_minutes);
            if fit == RuntimeFit::Contradicts {
                return None;
            }
            let similarity = title_similarity(query, candidate.titles);
            let score = match_score(file_runtime, candidate, similarity);
            Some((
                score,
                Chosen {
                    index,
                    fit,
                    similarity,
                },
            ))
        })
        .min_by(|(a_score, a), (b_score, b)| b_score.total_cmp(a_score).then(a.index.cmp(&b.index)))
        .map(|(_, chosen)| chosen)
}

/// How well a candidate fits the file: how alike its title is to what was
/// searched for, how close its runtime is to the file's, and whether it's of
/// the file's year. A year says more than a minute or two, as files run a
/// minute or two off the knowledge base's runtimes, e.g. a "White Snake" of
/// 2019 and 99 minutes for a file of 2019 and 102, rather than one of 2021
/// and 101.
pub(super) fn match_score(file_runtime: Duration, candidate: Candidate, similarity: f32) -> f32 {
    let runtime = match candidate.runtime_minutes {
        Some(minutes) => RUNTIME_WEIGHT * runtime_closeness(file_runtime, minutes),
        // Says nothing against the file being it, nor for, e.g. another "The
        // Covenant" than Guy Ritchie's.
        None => UNKNOWN_RUNTIME_SCORE,
    };
    // A festival premiere and a release can differ by one, e.g.
    // `Split.2016` for Shyamalan's, which TMDB dates 2017.
    let year = match candidate.years_off {
        Some(0 | 1) => YEAR_WEIGHT,
        _ => 0.0,
    };
    TITLE_WEIGHT * similarity + runtime + year
}

/// How close a file's runtime is to `minutes`, from 1 (within
/// [`RUNTIME_SLACK_MINUTES`]) falling off smoothly to 0, by how far off it is
/// as a share of the movie's runtime: a few minutes either way is likely
/// the knowledge base's timing or another cut, where a third more or less is
/// likely another movie.
pub(super) fn runtime_closeness(file_runtime: Duration, minutes: u32) -> f32 {
    let file_minutes = file_runtime.as_secs_f32() / 60.0;
    let off = ((file_minutes - minutes as f32).abs() - RUNTIME_SLACK_MINUTES).max(0.0);
    let relative = off / minutes.max(1) as f32;
    (-(relative / RUNTIME_SPREAD).powi(2)).exp()
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
/// The weight of a candidate's title similarity in its [`match_score`].
pub(super) const TITLE_WEIGHT: f32 = 1.0;
/// The weight of its runtime's closeness, as much as its title's: an exact
/// runtime outweighs a title with a few more words than was searched for.
pub(super) const RUNTIME_WEIGHT: f32 = 1.0;
/// The runtime score of a candidate whose runtime is unknown, halfway.
pub(super) const UNKNOWN_RUNTIME_SCORE: f32 = 0.5;
/// The score of a candidate of the file's year, give or take one.
pub(super) const YEAR_WEIGHT: f32 = 0.2;
/// How far off a runtime may be and still be as close as can be: TMDB times
/// movies in whole minutes.
pub(super) const RUNTIME_SLACK_MINUTES: f32 = 1.0;
/// How far off a runtime is, as a share of the movie's, for its closeness to
/// fall to about a third, e.g. 12 minutes of a 100-minute movie.
pub(super) const RUNTIME_SPREAD: f32 = 0.12;
/// How many of a search's results are checked against the file's runtime.
pub(super) const RUNTIME_CANDIDATES: usize = 5;
