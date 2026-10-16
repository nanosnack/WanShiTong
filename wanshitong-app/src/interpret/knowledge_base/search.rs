//! Searching a knowledge base for what a file is: which of its movies or
//! series, and which of a series' episodes, and how sure to be of each.

use super::{
    Episode, EpisodeRun, KbResult, KnowledgeBaseApi, KnowledgeBaseError, MovieRecord,
    RankedSearchFields, Season, SeasonSummary, SeriesRecord,
};
use crate::interpret::heuristics::ranks::{KB_EDITION, KB_FILE_ROLE};
use crate::{edition, title};
use regex::Regex;
use std::{collections::HashSet, sync::LazyLock, time::Duration};
use tokio::sync::Semaphore;
use wanshitong_core::domain::{
    ClaimPredicate, ExternalId, FileRole, KB_SEARCH_RESULTS_LOGGED, KbQueryEvent, KbSearchResult,
    Rank, Ranked, WorkKind,
};

type Result<T> = std::result::Result<T, KnowledgeBaseError>;

/// The claims that `kb`'s movies and series make about a file, as found by
/// the file's `fields`, one [`KbResult`] per work, and the queries made to
/// find them.
pub(super) async fn search_for_claims(
    kb: &dyn KnowledgeBaseApi,
    fields: &RankedSearchFields,
) -> Result<(Vec<KbResult>, Vec<KbQueryEvent>)> {
    static SEMAPHORE: Semaphore = Semaphore::const_new(8);
    let _permit = SEMAPHORE
        .acquire()
        .await
        .expect("we never close this semaphore");

    let mut search = Search::new(kb, fields);
    for strategy in STRATEGIES {
        if strategy.is_last_resort() && !search.claims.is_empty() {
            break;
        }
        search.by(strategy).await?;
    }
    Ok((search.claims, search.queries))
}

/// For a file that only the last resorts could find, what each finds on its
/// own: each strategy, and each of the last-resort titles alone, with the
/// title tried. None if a strategy before the last resorts found a work.
/// To measure how the last resorts agree and which to try first.
#[cfg(feature = "corpus")]
pub(super) async fn probe_last_resorts(
    kb: &dyn KnowledgeBaseApi,
    fields: &RankedSearchFields,
) -> Result<Option<Vec<(String, Option<String>, Vec<KbResult>)>>> {
    let mut search = Search::new(kb, fields);
    for strategy in STRATEGIES.into_iter().filter(|s| !s.is_last_resort()) {
        search.by(strategy).await?;
    }
    if !search.claims.is_empty() {
        return Ok(None);
    }
    let mut probes = Vec::new();
    for strategy in STRATEGIES.into_iter().filter(|s| s.is_last_resort()) {
        if strategy != Strategy::LastResortTitles {
            let mut alone = Search::new(kb, fields);
            alone.by(strategy).await?;
            probes.push((format!("{strategy:?}"), None, alone.claims));
            continue;
        }
        let lists = [
            &fields.last_resort_titles,
            &fields.last_resort_movie_titles,
            &fields.last_resort_series_names,
        ];
        for (which, list) in lists.into_iter().enumerate() {
            for title in list {
                let mut one = fields.clone();
                one.last_resort_titles.clear();
                one.last_resort_movie_titles.clear();
                one.last_resort_series_names.clear();
                match which {
                    0 => one.last_resort_titles.push(title.clone()),
                    1 => one.last_resort_movie_titles.push(title.clone()),
                    _ => one.last_resort_series_names.push(title.clone()),
                }
                let mut alone = Search::new(kb, &one);
                alone.by(strategy).await?;
                probes.push((
                    format!("{strategy:?}"),
                    Some(title.value.clone()),
                    alone.claims,
                ));
            }
        }
    }
    Ok(Some(probes))
}

/// The ways a file is searched for, in order: by what it says it is, and
/// then, if nothing found a work, by what it may be.
const STRATEGIES: [Strategy; 9] = [
    Strategy::MovieIds,
    Strategy::SeriesIds,
    Strategy::Titles,
    Strategy::SpecialOf,
    Strategy::SeriesNames,
    Strategy::LastResortTitles,
    Strategy::SeriesAndEpisode,
    Strategy::SeriesAndSeason,
    Strategy::NearSpelling,
];

/// A way a file is searched for. See [`STRATEGIES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Strategy {
    /// The movies that the file's IDs name.
    MovieIds,
    /// The series that the file's IDs name.
    SeriesIds,
    /// Movies, and failing that series, titled as the file.
    Titles,
    /// The series the file's name marks it as a special of.
    SpecialOf,
    /// Series named as the file says.
    SeriesNames,
    /// Titles that are only a last resort, e.g. cut before a subtitle.
    LastResortTitles,
    /// A title as a series' name and an episode's number.
    SeriesAndEpisode,
    /// A title as a series' name and a season.
    SeriesAndSeason,
    /// A movie titled nearly as the file.
    NearSpelling,
}

impl Strategy {
    /// Whether it's tried only if nothing before found a work, as what it
    /// finds is weaker evidence than what the file says it is.
    fn is_last_resort(self) -> bool {
        matches!(
            self,
            Self::LastResortTitles
                | Self::SeriesAndEpisode
                | Self::SeriesAndSeason
                | Self::NearSpelling
        )
    }
}

/// One file's search of a knowledge base: the works found so far, one
/// [`KbResult`] each, and the queries made to find them.
struct Search<'a> {
    kb: &'a dyn KnowledgeBaseApi,
    fields: &'a RankedSearchFields,
    /// [`RankedSearchFields::is_episode`], once.
    is_episode: bool,
    /// Whether the file is a movie's by its name and its release, and not an
    /// episode: then a series found only by a fallback isn't its work unless
    /// it's of the file's year. See [`Search::fits_kind`].
    is_movie: bool,
    claims: Vec<KbResult>,
    queries: Vec<KbQueryEvent>,
}

impl<'a> Search<'a> {
    /// Whether a series found only by a fallback, by a title, may be the
    /// file's: not a movie's, by its name and its release, unless it first
    /// aired the file's year, give or take one, e.g. a special TMDB lists as
    /// a series. Not "Spooks" of 2002 for `Spooks! 1953`.
    fn fits_kind(&self, series: &SeriesRecord) -> bool {
        !self.is_movie
            || matches!((self.fields.release_year, series.first_air_year),
                (Some(year), Some(aired)) if year.abs_diff(aired) <= 1)
    }

    /// Searches for the file by `strategy`.
    async fn by(&mut self, strategy: Strategy) -> Result<()> {
        match strategy {
            Strategy::MovieIds => self.by_movie_ids().await,
            Strategy::SeriesIds => self.by_series_ids().await,
            Strategy::Titles => self.by_titles().await,
            Strategy::SpecialOf => self.by_special_of().await,
            Strategy::SeriesNames => self.by_series_names().await,
            Strategy::LastResortTitles => self.by_last_resort_titles().await,
            Strategy::SeriesAndEpisode => self.by_series_and_episode().await,
            Strategy::SeriesAndSeason => self.by_series_and_season().await,
            Strategy::NearSpelling => self.by_near_spelling().await,
        }
    }

    fn new(kb: &'a dyn KnowledgeBaseApi, fields: &'a RankedSearchFields) -> Self {
        Search {
            kb,
            fields,
            is_episode: fields.is_episode(),
            is_movie: !fields.is_episode()
                && fields
                    .kind
                    .as_ref()
                    .is_some_and(|kind| kind.value == WorkKind::Movie),
            claims: Vec::new(),
            queries: Vec::new(),
        }
    }

    /// The movies that the file's IDs name. A movie's IDs, which are only
    /// ever asserted, are looked up as a movie's: one that names no movie
    /// finds nothing, rather than a series that would contradict the
    /// assertion.
    async fn by_movie_ids(&mut self) -> Result<()> {
        let fields = self.fields;
        for Ranked { rank, value: id } in &fields.ids {
            if let Some(movie) = self.kb.movie(id, &mut self.queries).await? {
                self.claims.push(movie_claims(*rank, &movie));
            }
        }
        Ok(())
    }

    /// The series that the file's series IDs name, and which of their
    /// episodes it is.
    async fn by_series_ids(&mut self) -> Result<()> {
        let fields = self.fields;
        for Ranked { rank, value: id } in &fields.series_ids {
            if let Some(series) = self.kb.series(id, &mut self.queries).await? {
                let mut claims = series_claims(*rank, &series);
                if self.is_episode {
                    claims.extend(
                        episode_claims(self.kb, &series.id, fields, None, *rank, &mut self.queries)
                            .await?,
                    );
                }
                self.claims.push(claims);
            }
        }
        Ok(())
    }

    /// The movies that the file's titles find, or failing that, the series
    /// that an episode's or a part's title does, if nothing names its series.
    /// The work a file is by each of its titles: as a movie, then, by a title
    /// that found none, as a series, only if no other title found a movie,
    /// e.g. `code geass - nunnally in wonderland` beside `code geass ova03 -
    /// nunnally in wonderland`, whose series' search finds the series it's an
    /// OVA of, which a knowledge base may list it in too.
    async fn by_titles(&mut self) -> Result<()> {
        let fields = self.fields;
        let mut outcomes = Vec::new();
        for Ranked { rank, value: title } in &fields.titles {
            let (results, year_bonus) = self.search_movies(title, fields.release_year).await?;
            let found = self
                .movie_from_results(title, *rank + year_bonus, &results)
                .await?;
            outcomes.push((*rank, title.as_str(), found));
        }
        let found_a_movie = self.found_a_movie();
        for (rank, title, found) in outcomes {
            match (found, self.is_episode) {
                (MovieSearchOutcome::RuntimeMatch, _) => {}
                // Only when nothing names the series: a name's search finds it,
                // and an episode's title, e.g. `romance dawn 01 - the dawn of
                // adventure` from a title tag, finds it worse or not at all.
                (_, true) => {
                    if fields.series_names.is_empty() {
                        let (results, _) = self.search_series(title, None).await?;
                        self.claims
                            .extend(series_titled_claims(title, rank, &results));
                    }
                }
                (MovieSearchOutcome::Nothing, false) if !found_a_movie => match fields.part {
                    Some(part) => self.part_of_series_titled(rank, title, part).await?,
                    None => self.series_titled_like(rank, title).await?,
                },
                (MovieSearchOutcome::Nothing, false) | (MovieSearchOutcome::TitledOnly, false) => {}
            }
        }
        Ok(())
    }

    /// The work a file is by a title, or a series' name, that's only a last
    /// resort, e.g. cut before its subtitle, only if nothing else found one: a
    /// series named as it is, e.g. "Kamen Rider Blorp" for `kamen rider blorp`
    /// of `Kamen Rider Blorp - 17SD`, a movie titled as it is, unless the
    /// file's an episode, or a series named as it begins, e.g. "Oh My Jump!
    /// Shōnen Jump Chikyū wo Sukū" for `oh my jump!`. A series first, as a name
    /// with a subtitle after it is more often an episode's than a movie's,
    /// unless it's a movie's file by its name and release: then a movie first.
    /// A movie of the file's year only, if it says one. Not a movie titled as
    /// it begins, as a series' name begins its movies' titles, e.g. "Kamen
    /// Rider Blorp: Final Stage", nor any work titled otherwise, which a search
    /// for fewer words finds too often. Its long vowels may be written
    /// otherwise, e.g. "Shinryōjo" for `shinryoujo`, and failing all those, the
    /// first work found is the file's if one of its other titles is.
    async fn by_last_resort_titles(&mut self) -> Result<()> {
        let fields = self.fields;
        // Each title, whether it may be a movie's and whether a series'.
        let as_either = fields
            .last_resort_titles
            .iter()
            .map(|title| (title, true, true));
        let as_movies = fields
            .last_resort_movie_titles
            .iter()
            .map(|title| (title, true, false));
        let as_series = fields
            .last_resort_series_names
            .iter()
            .map(|name| (name, false, true));
        let mut searched = HashSet::new();
        for (Ranked { rank, value: title }, may_be_movie, may_be_series) in
            as_either.chain(as_movies).chain(as_series)
        {
            if !searched.insert(title) {
                continue;
            }
            // A movie first, for a movie's file by its name and release.
            let as_movie = may_be_movie && !self.is_episode;
            let movie_first = as_movie && self.is_movie;
            let mut movies = None;
            if movie_first {
                let (found, results) = self.last_resort_movie(*rank, title).await?;
                if found {
                    return Ok(());
                }
                movies = Some(results);
            }
            let series = match may_be_series {
                true => self.search_series(title, None).await?.0,
                false => Vec::new(),
            };
            let named = |same: fn(&str, &str) -> bool| {
                series.iter().find(|result| {
                    titled_as(same, &result.name, title)
                        || titled_as(same, &result.original_name, title)
                })
            };
            if let Some(result) = named(title::same_words).filter(|r| self.fits_kind(r)) {
                self.claims.push(series_claims(*rank, result));
                return Ok(());
            }
            if as_movie && !movie_first {
                let (found, results) = self.last_resort_movie(*rank, title).await?;
                if found {
                    return Ok(());
                }
                movies = Some(results);
            }
            if let Some(result) = named(title::begins_with).filter(|r| self.fits_kind(r)) {
                self.claims.push(series_claims(*rank, result));
                return Ok(());
            }
            // The first work found, if one of its other titles is the file's:
            // a search finds a work by those too, e.g. "Clinic on the Sea" by
            // `umi no ue no shinryojo`.
            if let Some(result) = series.first().filter(|r| self.fits_kind(r)) {
                let names = self
                    .kb
                    .series_alternative_titles(&result.id, &mut self.queries)
                    .await?;
                if names
                    .iter()
                    .any(|name| titled_as(title::same_words, name, title))
                {
                    self.claims.push(series_claims(*rank, result));
                    return Ok(());
                }
            }
            let of_year = |movie: &&MovieRecord| {
                fields.release_year.is_none_or(|year| {
                    movie
                        .release_year
                        .is_some_and(|release_year| release_year.abs_diff(year) <= 1)
                })
            };
            if let Some((results, year_bonus)) = movies
                && let Some(result) = results.first().filter(of_year)
            {
                let titles = self
                    .kb
                    .movie_alternative_titles(&result.id, &mut self.queries)
                    .await?;
                if titles
                    .iter()
                    .any(|other| titled_as(title::same_words, other, title))
                {
                    self.claims.push(movie_claims(*rank + year_bonus, result));
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// The series and episode a file is by a title that's a series' name and
    /// an episode's number, e.g. `samurai sentai blorp 22 hd` or `wumbo lark
    /// 06sd`, if nothing else found a work: a single episode named without
    /// the tag, dash or leading zero that tells a lone video's number for an
    /// episode's (see `lone_numbered_episodes`). Only a series named as the
    /// words before it, or as them and a subtitle, or by another of its
    /// titles, as a movie's title may end in a number too, e.g. "Apollo 13",
    /// though one would have been found as the movie. Its number counts
    /// through the whole series.
    async fn by_series_and_episode(&mut self) -> Result<()> {
        if self.is_movie {
            return Ok(());
        }
        let fields = self.fields;
        let mut titles: Vec<&Ranked<String>> = fields.titles.iter().collect();
        titles.sort_by_key(|title| std::cmp::Reverse(title.rank));
        let mut searched = HashSet::new();
        for Ranked { rank, value: title } in titles {
            for split in series_and_episode(title) {
                if !searched.insert(split.name.clone()) {
                    continue;
                }
                if searched.len() > SERIES_AND_EPISODE_SEARCHES {
                    return Ok(());
                }
                let Some(series) = self.series_named(&split.name).await? else {
                    continue;
                };
                let episodes = [split.episode, split.episode];
                let mut claims = series_claims(*rank, &series);
                match split.season {
                    Some(season) => claims.extend([
                        Ranked::new(*rank + KB_BONUS, ClaimPredicate::Season(season)),
                        Ranked::new(*rank + KB_BONUS, ClaimPredicate::EpisodeRange(episodes)),
                    ]),
                    None => claims.push(Ranked::new(
                        *rank + KB_BONUS,
                        ClaimPredicate::AbsoluteEpisodeRange(episodes),
                    )),
                }
                self.claims.push(claims);
                return Ok(());
            }
        }
        Ok(())
    }

    /// The series and season a file is by a title, or a series' name, that's
    /// a series' name and a season's, e.g. `wumbo lark 2nd season`, `zibble
    /// s02` or `glimmer iv`, if nothing else found a work, as a knowledge
    /// base lists a later season under its series. Only a series named as
    /// the words before it, or as them and a subtitle, or by another of its
    /// titles. The file's episodes, if numbered through a run without a
    /// season, are the season's.
    async fn by_series_and_season(&mut self) -> Result<()> {
        if self.is_movie {
            return Ok(());
        }
        let fields = self.fields;
        let mut names: Vec<&Ranked<String>> =
            fields.series_names.iter().chain(&fields.titles).collect();
        names.sort_by_key(|name| std::cmp::Reverse(name.rank));
        let mut searched = HashSet::new();
        for Ranked { rank, value: name } in names {
            let Some((series_name, season)) = series_and_season(name) else {
                continue;
            };
            if !searched.insert(series_name.clone()) || searched.len() > SERIES_AND_EPISODE_SEARCHES
            {
                continue;
            }
            let Some(series) = self.series_named(&series_name).await? else {
                continue;
            };
            let mut claims = series_claims(*rank, &series);
            claims.push(Ranked::new(
                *rank + KB_BONUS,
                ClaimPredicate::Season(season),
            ));
            if let Some(episodes) = fields.absolute_episode_range {
                claims.push(Ranked::new(
                    *rank + KB_BONUS,
                    ClaimPredicate::EpisodeRange(episodes),
                ));
            }
            self.claims.push(claims);
            return Ok(());
        }
        Ok(())
    }

    /// The movie a file is by a title spelled near its, e.g. "The Alto
    /// Knights" for `the alto nights`, if nothing else found a work: as a
    /// knowledge base's search finds nothing for a word spelled otherwise, a
    /// search for each title without one of its words, and only a result
    /// [`title::spelled_nearly`], of the file's year, give or take one, and
    /// as long as the file, if it's measured and the knowledge base says. Not
    /// for a file that says no year, nor an episode.
    async fn by_near_spelling(&mut self) -> Result<()> {
        let fields = self.fields;
        let Some(year) = fields.release_year.filter(|_| !self.is_episode) else {
            return Ok(());
        };
        let mut titles: Vec<&Ranked<String>> = fields.titles.iter().collect();
        titles.sort_by_key(|title| std::cmp::Reverse(title.rank));
        let mut searched = HashSet::new();
        for Ranked { rank, value: title } in titles.into_iter().take(2) {
            let words: Vec<&str> = title.split_whitespace().collect();
            if !(2..=NEAR_SPELLING_MAX_WORDS).contains(&words.len()) {
                continue;
            }
            for skipped in 0..words.len() {
                let query = words
                    .iter()
                    .enumerate()
                    .filter(|&(i, _)| i != skipped)
                    .map(|(_, word)| *word)
                    .collect::<Vec<_>>()
                    .join(" ");
                if !title::has_words(&query) || !searched.insert(query.clone()) {
                    continue;
                }
                if searched.len() > NEAR_SPELLING_SEARCHES {
                    return Ok(());
                }
                let (results, _) = self.search_movies(&query, Some(year)).await?;
                for result in &results {
                    let of_year = result
                        .release_year
                        .is_some_and(|release_year| release_year.abs_diff(year) <= 1);
                    let near = title::spelled_nearly(&result.title, title)
                        || title::spelled_nearly(&result.original_title, title);
                    if !of_year || !near {
                        continue;
                    }
                    if let Some(runtime) = fields.runtime {
                        let Some(movie) = self.kb.movie(&result.id, &mut self.queries).await?
                        else {
                            continue;
                        };
                        let fit = runtime_fit(runtime, movie.runtime_minutes);
                        if !matches!(fit, RuntimeFit::Match { .. } | RuntimeFit::Unknown) {
                            continue;
                        }
                    }
                    self.claims.push(movie_claims(*rank, result));
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// The series named `name` as a last resort: the first of a search's
    /// results named so, or so and a subtitle, or else the first if another
    /// of its titles is.
    async fn series_named(&mut self, name: &str) -> Result<Option<SeriesRecord>> {
        let (results, _) = self.search_series(name, None).await?;
        let named = results.iter().find(|result| {
            titled_as(title::is_or_has_subtitle, &result.name, name)
                || titled_as(title::is_or_has_subtitle, &result.original_name, name)
        });
        if let Some(series) = named {
            return Ok(Some(series.clone()));
        }
        let Some(first) = results.first() else {
            return Ok(None);
        };
        let others = self
            .kb
            .series_alternative_titles(&first.id, &mut self.queries)
            .await?;
        Ok(others
            .iter()
            .any(|other| titled_as(title::same_words, other, name))
            .then(|| first.clone()))
    }

    /// The movie titled `title`, as a last resort: one of the file's year,
    /// give or take one, if it says one, as one of another year is another
    /// work titled so, e.g. a remake. Whether it found one, and the search's
    /// results.
    async fn last_resort_movie(
        &mut self,
        rank: Rank,
        title: &str,
    ) -> Result<(bool, (Vec<MovieRecord>, f32))> {
        let year = self.fields.release_year;
        let (results, year_bonus) = self.search_movies(title, year).await?;
        let titled: Vec<MovieRecord> = results
            .iter()
            .filter(|result| {
                titled_as(title::same_words, &result.title, title)
                    || titled_as(title::same_words, &result.original_title, title)
            })
            .cloned()
            .collect();
        let chosen = match year {
            Some(_) => titled_of_year(title, year, &titled, |movie| {
                ([&movie.title, &movie.original_title], movie.release_year)
            }),
            None => titled.first(),
        };
        let found = chosen.is_some();
        if let Some(result) = chosen {
            self.claims.push(movie_claims(rank + year_bonus, result));
        }
        Ok((found, (results, year_bonus)))
    }

    /// The series that the file is a part of, by its title: a part of a work
    /// that isn't a movie, e.g. The Sorrow and the Pity, which TMDB lists as
    /// a series of its two parts. Only a series titled so, as a movie's parts
    /// would find any other.
    async fn part_of_series_titled(&mut self, rank: Rank, title: &str, part: u32) -> Result<()> {
        let (results, _) = self.search_series(title, None).await?;
        if let Some(result) = titled_series_result(title, &results) {
            let rank = rank + title_match_bonus(title, [&result.name, &result.original_name]);
            let mut claims = series_claims(rank, result);
            let episodes = ClaimPredicate::AbsoluteEpisodeRange([part, part]);
            claims.push(Ranked::new(rank + KB_BONUS, episodes));
            self.claims.push(claims);
        }
        Ok(())
    }

    /// The series a file is, or is a special of, by a title that found no
    /// movie: a series' whole season named without its episodes' numbers,
    /// e.g. `Toji no Miko`, whose search finds "Katana Maidens: Toji no Miko"
    /// by another of its titles, or a special named for its series. As the
    /// series an episode's title finds, the knowledge base's first, unless
    /// one is titled so. Not for a title of nothing but numbered words, e.g.
    /// a disc's `day1 disc1`, which finds any series. A file of a batch, whose
    /// title is an episode's, e.g. each of a series' hundreds named by their
    /// episodes' titles, is looked up by what its siblings' titles begin with
    /// instead, once for them all, if anything.
    async fn series_titled_like(&mut self, rank: Rank, title: &str) -> Result<()> {
        let title = match &self.fields.batch {
            None => title,
            Some(Some(shared)) => shared.as_str(),
            Some(None) => return Ok(()),
        };
        let names_something = title::words(title)
            .iter()
            .any(|word| !word.chars().any(|c| c.is_ascii_digit()));
        if !names_something {
            return Ok(());
        }
        let (results, _) = self.search_series(title, None).await?;
        let Some(result) =
            choose_series_result(title, None, &results).filter(|result| self.fits_kind(result))
        else {
            return Ok(());
        };
        let rank = rank + title_match_bonus(title, [&result.name, &result.original_name]);
        let special =
            special_named_like(self.kb, &result.id, self.fields, &mut self.queries).await?;
        self.claims.push(special_claims(rank, result, special));
        Ok(())
    }

    /// The series that a special, e.g. an OVA, is one of, if the file's
    /// titles found no movie: TMDB lists some specials as movies, e.g. Code
    /// Geass'.
    async fn by_special_of(&mut self) -> Result<()> {
        let fields = self.fields;
        let Some(Ranked {
            rank,
            value: series,
        }) = &fields.special_of
        else {
            return Ok(());
        };
        if self.found_a_movie() {
            return Ok(());
        }
        let (results, _) = self.search_series(series, None).await?;
        let Some(result) = titled_series_result(series, &results) else {
            return Ok(());
        };
        let rank = *rank + title_match_bonus(series, [&result.name, &result.original_name]);
        let special = match self.is_episode {
            true => None,
            false => special_named_like(self.kb, &result.id, fields, &mut self.queries).await?,
        };
        let mut claims = special_claims(rank, result, special);
        claims.push(Ranked::new(rank + KB_BONUS, ClaimPredicate::Special));
        self.claims.push(claims);
        Ok(())
    }

    /// Whether any search so far found a movie, even one only titled like
    /// the file.
    fn found_a_movie(&self) -> bool {
        self.claims
            .iter()
            .flatten()
            .any(|claim| matches!(claim.value, ClaimPredicate::MovieExternalId(_)))
    }

    /// The series that the file's series names find, and which of their
    /// seasons, specials or episodes it is.
    async fn by_series_names(&mut self) -> Result<()> {
        let fields = self.fields;
        for Ranked { rank, value: name } in &fields.series_names {
            self.by_series_name(*rank, name).await?;
        }
        Ok(())
    }

    async fn by_series_name(&mut self, rank: Rank, name: &str) -> Result<()> {
        let fields = self.fields;
        // A series' name is often another's too, e.g. a remake's, and the
        // year tells them apart.
        let (results, year_bonus) = self.search_series(name, fields.series_year).await?;
        let Some(result) = choose_series_result(name, fields.series_year, &results) else {
            return Ok(());
        };
        let others: Vec<&str> = fields
            .series_names
            .iter()
            .map(|other| other.value.as_str())
            .filter(|other| *other != name)
            .collect();
        let result = known_also_as(name, result, &results, &others);
        let result = self.series_with_season(name, result, &results).await?;
        let rank =
            rank + year_bonus + title_match_bonus(name, [&result.name, &result.original_name]);
        if let Some((season, episodes)) = self
            .season_named_like_another_name(&result.id, name)
            .await?
        {
            let rank = rank + SEASON_NAME_BONUS;
            let mut claims = series_claims(rank, result);
            claims.extend([
                Ranked::new(rank + KB_BONUS, ClaimPredicate::Season(season)),
                Ranked::new(rank + KB_BONUS, ClaimPredicate::EpisodeRange(episodes)),
            ]);
            self.claims.push(claims);
        } else if !self.is_episode {
            // A file that isn't an episode may be one of the series'
            // specials, e.g. a featurette in its release.
            let special =
                special_named_like(self.kb, &result.id, fields, &mut self.queries).await?;
            self.claims.push(special_claims(rank, result, special));
        } else {
            let mut claims = series_claims(rank, result);
            claims.extend(
                episode_claims(
                    self.kb,
                    &result.id,
                    fields,
                    Some(name),
                    rank,
                    &mut self.queries,
                )
                .await?,
            );
            self.claims.push(claims);
        }
        Ok(())
    }

    /// Of several series named as searched for, e.g. a show and its remake,
    /// the one `chosen`, or else the first, whose file's season is as many
    /// episodes long as the file's release has of it, e.g. 2010's "Upstairs
    /// Downstairs" for a first season of three, not 1971's of thirteen.
    /// Failing that, as a release may hold only some of a season, or number
    /// a finale in two parts that the knowledge base lists as one, the one
    /// chosen, or else the first, that has the season at all, as a release's
    /// season may be one the other never had, e.g. `S04` of a show that ran
    /// two. The one chosen, if the file says no season, its series' year
    /// says which, or none has it.
    async fn series_with_season<'r>(
        &mut self,
        name: &str,
        chosen: &'r SeriesRecord,
        results: &'r [SeriesRecord],
    ) -> Result<&'r SeriesRecord> {
        let fields = self.fields;
        let Some(season) = fields.season.filter(|_| fields.series_year.is_none()) else {
            return Ok(chosen);
        };
        let named: Vec<&SeriesRecord> = results
            .iter()
            .filter(|result| {
                title::same_words(&result.name, name)
                    || title::same_words(&result.original_name, name)
            })
            .take(SAME_NAMED_SERIES)
            .collect();
        if named.len() < 2 || !named.iter().any(|result| result.id == chosen.id) {
            return Ok(chosen);
        }
        let others = named.into_iter().filter(|result| result.id != chosen.id);
        let mut with_season = None;
        for candidate in std::iter::once(chosen).chain(others) {
            let Some(series) = self.kb.series(&candidate.id, &mut self.queries).await? else {
                continue;
            };
            let Some(of_file) = series.seasons.iter().find(|s| s.number == season) else {
                continue;
            };
            if fields
                .season_episodes
                .is_none_or(|episodes| of_file.episode_count == episodes)
            {
                return Ok(candidate);
            }
            with_season.get_or_insert(candidate);
        }
        Ok(with_season.unwrap_or(chosen))
    }

    /// The season of the series, found by the file's series name `name`,
    /// that's named like another name the file gives its series, e.g. One
    /// Pace, whose season 33 is the arc `Whole Cake Island` a run is named
    /// after, and the run's numbers as episodes of it. Only for a run
    /// numbered through the whole series.
    async fn season_named_like_another_name(
        &mut self,
        series: &ExternalId,
        name: &str,
    ) -> Result<Option<(u32, [u32; 2])>> {
        let fields = self.fields;
        let Some(episodes) = fields.absolute_episode_range else {
            return Ok(None);
        };
        let other_names: Vec<&str> = fields
            .series_names
            .iter()
            .map(|other| other.value.as_str())
            .filter(|other| *other != name)
            .collect();
        if other_names.is_empty() {
            return Ok(None);
        }
        let Some(series) = self.kb.series(series, &mut self.queries).await? else {
            return Ok(None);
        };
        Ok(season_named_like(&series.seasons, &other_names, episodes)
            .map(|season| (season, episodes)))
    }

    /// Movies titled `title`, as [`with_year_then_without`] searches them by
    /// `year`, logged.
    async fn search_movies(
        &mut self,
        title: &str,
        year: Option<u32>,
    ) -> Result<(Vec<MovieRecord>, f32)> {
        let (results, bonus) =
            with_year_then_without(self.kb, title, year, KnowledgeBaseApi::search_movies).await?;
        self.queries.push(movie_search_event(title, &results));
        Ok((results, bonus))
    }

    /// Series named `name`, as [`with_year_then_without`] searches them by
    /// `year`, logged.
    async fn search_series(
        &mut self,
        name: &str,
        year: Option<u32>,
    ) -> Result<(Vec<SeriesRecord>, f32)> {
        let (results, bonus) =
            with_year_then_without(self.kb, name, year, KnowledgeBaseApi::search_series).await?;
        self.queries.push(series_search_event(name, &results));
        Ok((results, bonus))
    }

    /// The movie, of a title search's `results` for `query`, that the file
    /// is: the one as long as it, if any, or else one named like it. Its
    /// claims are ranked from `rank`.
    async fn movie_from_results(
        &mut self,
        query: &str,
        rank: Rank,
        results: &[MovieRecord],
    ) -> Result<MovieSearchOutcome> {
        let &RankedSearchFields {
            runtime,
            parts_runtime,
            release_year,
            ..
        } = self.fields;
        // A part of a work, e.g. half of a concert, is matched by how long the
        // parts last together. Two measurements adding up to the movie's runtime
        // corroborate it, as its year would.
        let Some(claim_runtime) = parts_runtime.or(runtime) else {
            return Ok(
                match chosen_movie_claims(query, release_year, rank, results) {
                    Some(claims) => {
                        self.claims.push(claims);
                        MovieSearchOutcome::TitledOnly
                    }
                    None => MovieSearchOutcome::Nothing,
                },
            );
        };
        // A search's results may not say how long their movies are, so each
        // movie is looked up.
        let mut details = Vec::new();
        for result in results.iter().take(RUNTIME_CANDIDATES) {
            // None is very strange if this happens.
            if let Some(movie) = self.kb.movie(&result.id, &mut self.queries).await? {
                details.push(movie);
            }
        }
        let candidates: Vec<_> = details
            .iter()
            .map(|details| Candidate {
                titles: [&details.title, &details.original_title],
                runtime_minutes: details.runtime_minutes,
                year_agrees: matches!((release_year, details.release_year),
                    (Some(file), Some(movie)) if file.abs_diff(movie) <= 1),
            })
            .collect();

        // As long as the file give or take a little, or a few minutes more if
        // its title or year agrees too: a file's runtime is mostly within a
        // minute or two of the knowledge base's, and a few minutes either way
        // fits many movies.
        // A result as long as the file, titled as well as any result it may
        // be another cut of, of the file's year as nearly: a file a few
        // minutes off its own title's movie, e.g. a TV edit, is as long as
        // some other movie, e.g. "Outlaw Justice" for `Justice`'s. Not as
        // well as one whose runtime is unknown, which says nothing against a
        // runtime match, e.g. another "The Covenant" for Guy Ritchie's; nor
        // one further from the file's year, e.g. 2013's "The Taking" for
        // `The.Taking.2014` as long as "The Taking of Deborah Logan" of 2014.
        let years_off = |year: Option<u32>| match (release_year, year) {
            (Some(file), Some(year)) => file.abs_diff(year),
            (Some(_), None) => u32::MAX,
            (None, _) => 0,
        };
        let best_titled = |within: u32| {
            candidates
                .iter()
                .zip(&details)
                .filter(|(candidate, movie)| {
                    matches!(
                        runtime_fit(claim_runtime, candidate.runtime_minutes),
                        RuntimeFit::Match { .. } | RuntimeFit::OtherCut
                    ) && years_off(movie.release_year) <= within
                })
                .map(|(candidate, _)| title_similarity(query, candidate.titles))
                .fold(0.0, f32::max)
        };
        // As long as the file give or take a little, or a few minutes more if
        // its title or year agrees too: a file's runtime is mostly within a
        // minute or two of the knowledge base's, and a few minutes either way
        // fits many movies.
        let fits = |index: usize, minute_diff: i64| {
            let movie = &details[index];
            // An entry for a cut of another movie is that movie's.
            let movie_index = cut_of_another(&details, index).map_or(index, |(movie, _)| movie);
            let within = years_off(details[movie_index].release_year);
            if title_similarity(query, candidates[movie_index].titles) < best_titled(within) {
                return false;
            }
            minute_diff <= RUNTIME_CLOSE_MINUTES
                || parts_runtime.is_some()
                || corroborates_runtime_match(
                    query,
                    release_year,
                    [&movie.title, &movie.original_title],
                    movie.release_year,
                )
        };
        if let Some(RuntimeMatch {
            index,
            minute_diff,
            similarity,
        }) = best_runtime_match(query, claim_runtime, candidates.iter().copied(), fits)
        {
            // An entry for a cut of another movie, e.g. `Kingdom Of Heaven
            // (Director's Cut)`, is that movie's work in an edition, as it's for
            // the file of a cut that the knowledge base doesn't time.
            let (index, edition) = match cut_of_another(&details, index) {
                Some((movie, edition)) => (movie, Some(edition)),
                None => (index, None),
            };
            // Matching runtime is a strong indicator, if something else says it's
            // this movie too: many are as long as an episode, e.g. a short titled
            // `Ash 01` for `01.MP4`.
            let movie = details.swap_remove(index);
            let titles = [movie.title.as_str(), movie.original_title.as_str()];
            let runtime_bonus = if parts_runtime.is_some()
                || corroborates_runtime_match(query, release_year, titles, movie.release_year)
            {
                CORROBORATED_RUNTIME_BONUS
            } else {
                UNCORROBORATED_RUNTIME_BONUS
            };
            let closeness =
                RUNTIME_CLOSENESS_BONUS * (RUNTIME_TOLERANCE_MINUTES - minute_diff) as f32;
            let bonus = runtime_bonus + closeness + TITLE_SIMILARITY_BONUS * similarity;
            let mut claims = movie_claims(rank + bonus, &movie);
            // Lasting as long as the movie is evidence that the file is the
            // feature rather than an extra. Like the edition, it's judged
            // once this result stands, against what the file's name says of
            // its role, so it ranks as that kind of evidence does, not as
            // the title that found the movie.
            claims.push(Ranked::new(
                rank.fixed(KB_FILE_ROLE),
                ClaimPredicate::FileRole(FileRole::Main),
            ));
            if let Some(edition) = edition {
                claims.push(Ranked::new(
                    rank.fixed(KB_EDITION),
                    ClaimPredicate::Edition(edition),
                ));
            }
            self.claims.push(claims);
            return Ok(MovieSearchOutcome::RuntimeMatch);
        }

        // With no result as long as the file, one named like it will do, unless
        // its runtime says the file can't be it, e.g. an episode named like a
        // feature. If they all do, there's no result.
        let Some(Unmatched {
            index,
            fit,
            similarity,
        }) = best_unmatched_candidate(query, claim_runtime, candidates.iter().copied())
        else {
            return Ok(MovieSearchOutcome::Nothing);
        };
        let bonus = TITLE_SIMILARITY_BONUS * similarity;
        let mut claims = movie_claims(rank + bonus, &details[index]);
        if fit == RuntimeFit::OtherCut {
            // The file is feature length, only another cut than the one the
            // knowledge base times, e.g. an extended edition. It's still the
            // feature of its own manifestation.
            claims.push(Ranked::new(
                rank.fixed(KB_FILE_ROLE),
                ClaimPredicate::FileRole(FileRole::Main),
            ));
        }
        self.claims.push(claims);
        Ok(MovieSearchOutcome::TitledOnly)
    }
}

/// What a search of the file's title for movies found.
enum MovieSearchOutcome {
    /// A movie as long as the file. It outranks the file being an episode.
    RuntimeMatch,
    /// Only a movie titled like the file, which an episode's numbers outrank.
    TitledOnly,
    /// No movie the file could be.
    Nothing,
}

/// The results of searching `kb` for `query` with `search`, by `year`, if
/// it's known, or else without a year, in case it was throwing off results,
/// and the bonus for finding them by the year.
async fn with_year_then_without<'k, T, Fut>(
    kb: &'k dyn KnowledgeBaseApi,
    query: &'k str,
    year: Option<u32>,
    search: impl Fn(&'k dyn KnowledgeBaseApi, &'k str, Option<u32>) -> Fut,
) -> Result<(Vec<T>, f32)>
where
    Fut: Future<Output = Result<Vec<T>>>,
{
    if year.is_some() {
        let results = search(kb, query, year).await?;
        if !results.is_empty() {
            return Ok((results, RELEASE_YEAR_MATCH_BONUS));
        }
    }
    Ok((search(kb, query, None).await?, 0.0))
}

mod choice;
mod claims;
mod episodes;
mod runtime;
mod variants;

use choice::*;
use claims::*;
use episodes::*;
use runtime::*;
use variants::*;

#[cfg(test)]
mod tests;
