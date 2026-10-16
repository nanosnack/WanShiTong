//! What a search found, as claims: a work's and its parts', ranked by how well it was found, and the audit log's record of each search.

use super::*;

/// A series' claims, and if the file is one of its specials, its number among
/// them, outranking the file's title finding a movie, e.g. `The Wire Odyssey`.
pub(super) fn special_claims(rank: Rank, series: &SeriesRecord, special: Option<u32>) -> KbResult {
    let Some(number) = special else {
        return series_claims(rank, series);
    };
    let rank = rank + SPECIAL_NAME_BONUS;
    let mut claims = series_claims(rank, series);
    claims.extend([
        Ranked::new(rank + KB_BONUS, ClaimPredicate::Season(0)),
        Ranked::new(
            rank + KB_BONUS,
            ClaimPredicate::EpisodeRange([number, number]),
        ),
    ]);
    claims
}

/// Without the file's runtime to go by, the claims of the movie that
/// [`choose_movie_result`] chooses.
pub(super) fn chosen_movie_claims(
    query: &str,
    year: Option<u32>,
    rank: Rank,
    results: &[MovieRecord],
) -> Option<KbResult> {
    let result = choose_movie_result(query, year, results)?;
    let bonus = title_match_bonus(query, [&result.title, &result.original_title]);
    Some(movie_claims(rank + bonus, result))
}

/// The claims of the series searched for, as [`choose_series_result`]
/// chooses it.
pub(super) fn series_titled_claims(
    query: &str,
    rank: Rank,
    results: &[SeriesRecord],
) -> Option<KbResult> {
    let result = choose_series_result(query, None, results)?;
    let bonus = title_match_bonus(query, [&result.name, &result.original_name]);
    Some(series_claims(rank + bonus, result))
}

/// A bonus for a result titled like what was searched for, e.g. "PlayTime" for
/// "playtime" over "Playtime Towers". It settles otherwise equal results, but
/// is far below a runtime match.
pub(super) fn title_match_bonus(query: &str, titles: [&str; 2]) -> f32 {
    TITLE_SIMILARITY_BONUS * title_similarity(query, titles)
}

/// How alike a result's title or original title is to what was searched for.
pub(super) fn title_similarity(query: &str, titles: [&str; 2]) -> f32 {
    titles
        .iter()
        .map(|t| title::similarity(query, t))
        .fold(0.0, f32::max)
}

pub(super) fn movie_claims(rank: Rank, movie: &MovieRecord) -> KbResult {
    let mut claims = vec![
        Ranked::new(
            rank + KB_BONUS,
            ClaimPredicate::MovieExternalId(movie.id.clone()),
        ),
        Ranked::new(
            rank + KB_BONUS,
            ClaimPredicate::MovieTitle(movie.title.clone()),
        ),
    ];
    if let Some(imdb_id) = &movie.imdb_id {
        claims.push(Ranked::new(
            rank + KB_BONUS,
            ClaimPredicate::MovieExternalId(ExternalId::Imdb(imdb_id.clone())),
        ));
    }
    if let Some(year) = movie.release_year {
        claims.push(Ranked::new(
            rank + KB_BONUS,
            ClaimPredicate::MovieReleaseYear(year),
        ));
    }
    claims
}

pub(super) fn series_claims(rank: Rank, series: &SeriesRecord) -> KbResult {
    let mut claims = vec![
        Ranked::new(
            rank + KB_BONUS,
            ClaimPredicate::TvSeriesExternalId(series.id.clone()),
        ),
        Ranked::new(
            rank + KB_BONUS,
            ClaimPredicate::TvSeriesName(series.name.clone()),
        ),
        // TODO: # of seasons
        // TODO: # of episodes
    ];
    if let Some(year) = series.first_air_year {
        claims.push(Ranked::new(
            rank + KB_BONUS,
            ClaimPredicate::TvSeriesYear(year),
        ));
    }
    claims
}

/// A movie search for an audit log, with its first results.
pub(super) fn movie_search_event(query: &str, results: &[MovieRecord]) -> KbQueryEvent {
    KbQueryEvent::MovieSearch {
        query: query.to_string(),
        results: results
            .iter()
            .take(KB_SEARCH_RESULTS_LOGGED)
            .map(|movie| KbSearchResult {
                id: movie.id.clone(),
                title: movie.title.clone(),
                year: movie.release_year,
            })
            .collect(),
    }
}

/// A TV series search for an audit log, with its first results.
pub(super) fn series_search_event(query: &str, results: &[SeriesRecord]) -> KbQueryEvent {
    KbQueryEvent::TvSeriesSearch {
        query: query.to_string(),
        results: results
            .iter()
            .take(KB_SEARCH_RESULTS_LOGGED)
            .map(|series| KbSearchResult {
                id: series.id.clone(),
                title: series.name.clone(),
                year: series.first_air_year,
            })
            .collect(),
    }
}

/// The bonus for what a knowledge base says of a file over what its name
/// does.
pub(super) const KB_BONUS: f32 = 2.0;
pub(super) const RELEASE_YEAR_MATCH_BONUS: f32 = 1.0;
/// The bonus for a series with a season named like another name the file
/// gives its series. It outranks the series that name finds, e.g. One Pace,
/// with its season 34 `Reverie`, over the drama *Reverie* for a run named so.
pub(super) const SEASON_NAME_BONUS: f32 = 10.0;
/// The bonus for a series with a special named like the file, and as long.
/// It outranks a movie its title finds, e.g. `The Wire Odyssey` for a
/// featurette of The Wire, which TMDB lists as both.
pub(super) const SPECIAL_NAME_BONUS: f32 = 10.0;
/// The bonus for a series with an episode named like the file's title. It
/// outranks the series alone, and with it the numbers in the file's name.
pub(super) const EPISODE_TITLE_BONUS: f32 = 10.0;
/// The bonus for a result titled exactly what was searched for. Less alike
/// titles get a share of it.
pub(super) const TITLE_SIMILARITY_BONUS: f32 = 1.5;
