//! Code related to fetching metadata from external knowledge bases like TMDB.

mod search;

use async_trait::async_trait;
use std::fmt;
use std::time::Duration;
use wanshitong_core::domain::{
    ClaimPredicate, ClaimSource, Date, ExternalId, Heuristic, KbQueryEvent, MovieKbData, Rank,
    Ranked, RankedClaimPredicate, TvSeriesKbData, WorkKind,
};

/// What to search a knowledge base for, from a file's `claims`, if anything.
/// `is_release_name` tells a title that's a release's name, with an episode's
/// marker or a release's words in it, from a work's: an episode's are left
/// out, as nothing is titled so (see [`RankedSearchFields::titles`]).
pub fn find_kb_search_fields(
    claims: &[RankedClaimPredicate],
    is_release_name: impl Fn(&str) -> bool,
) -> Option<RankedSearchFields> {
    let mut fields = RankedSearchFields::default();
    let mut release_year_rank = Rank::Number(f32::NEG_INFINITY);
    let mut series_year_rank = Rank::Number(f32::NEG_INFINITY);
    let mut part_rank = Rank::Number(f32::NEG_INFINITY);
    let mut season_rank = Rank::Number(f32::NEG_INFINITY);
    let mut episode_range_rank = Rank::Number(f32::NEG_INFINITY);
    let mut absolute_episode_range_rank = Rank::Number(f32::NEG_INFINITY);
    let mut is_extra = false;
    for &RankedClaimPredicate {
        ref pred,
        rank,
        active,
        source,
        ..
    } in claims
    {
        if !active {
            continue;
        }
        if let ClaimSource::Heuristic(heuristic) = source
            && heuristic.is_last_resort_title()
        {
            let movie_only = source == ClaimSource::Heuristic(Heuristic::TitleOfOneWord);
            match pred {
                ClaimPredicate::MovieTitle(title) if movie_only => {
                    fields.last_resort_movie_titles.push(Ranked {
                        rank,
                        value: title.clone(),
                    })
                }
                ClaimPredicate::MovieTitle(title) => fields.last_resort_titles.push(Ranked {
                    rank,
                    value: title.clone(),
                }),
                ClaimPredicate::TvSeriesName(name) => {
                    fields.last_resort_series_names.push(Ranked {
                        rank,
                        value: name.clone(),
                    })
                }
                _ => (),
            }
            continue;
        }

        match pred {
            ClaimPredicate::MovieExternalId(id) => {
                fields.ids.push(Ranked {
                    rank,
                    value: id.clone(),
                });
            }
            ClaimPredicate::TvSeriesExternalId(id) => {
                fields.series_ids.push(Ranked {
                    rank,
                    value: id.clone(),
                });
            }
            ClaimPredicate::MovieTitle(title) => {
                fields.titles.push(Ranked {
                    rank,
                    value: title.clone(),
                });
            }
            ClaimPredicate::TvSeriesName(name) => {
                fields.series_names.push(Ranked {
                    rank,
                    value: name.clone(),
                });
            }
            &ClaimPredicate::Duration(d) => {
                // We only expect one duration claim, and it should be accurate.
                fields.runtime = Some(d);
            }
            &ClaimPredicate::MovieReleaseYear(year) => {
                if rank > release_year_rank {
                    release_year_rank = rank;
                    fields.release_year = Some(year);
                }
            }
            &ClaimPredicate::Part(part) => {
                if rank > part_rank {
                    part_rank = rank;
                    fields.part = Some(part);
                }
            }
            ClaimPredicate::SpecialOf(series) => {
                if fields
                    .special_of
                    .as_ref()
                    .is_none_or(|special_of| rank > special_of.rank)
                {
                    fields.special_of = Some(Ranked {
                        rank,
                        value: series.clone(),
                    });
                }
            }
            &ClaimPredicate::TvSeriesYear(year) => {
                if rank > series_year_rank {
                    series_year_rank = rank;
                    fields.series_year = Some(year);
                }
            }

            &ClaimPredicate::Season(season) => {
                if rank > season_rank {
                    season_rank = rank;
                    fields.season = Some(season);
                }
            }
            &ClaimPredicate::EpisodeRange(range) => {
                if rank > episode_range_rank {
                    episode_range_rank = rank;
                    fields.episode_range = Some(range);
                }
            }
            &ClaimPredicate::AbsoluteEpisodeRange(range) => {
                if rank > absolute_episode_range_rank {
                    absolute_episode_range_rank = rank;
                    fields.absolute_episode_range = Some(range);
                }
            }

            // Nothing to look up for a file excluded from the catalog.
            ClaimPredicate::Excluded => return None,

            // Part of another file's work, like a disc's streams, which have no
            // names of their own to search by anyway.
            ClaimPredicate::ExtraOf(_) | ClaimPredicate::DiscOf(_) => is_extra = true,

            // Claimed after searching, of what the searches didn't find.
            ClaimPredicate::FileRole(_) | ClaimPredicate::Special => (),

            // Which cut of the work the file is, whichever work it is.
            ClaimPredicate::Edition(_) => (),
            ClaimPredicate::WorkKind(kind) => {
                if fields.kind.as_ref().is_none_or(|known| rank > known.rank) {
                    fields.kind = Some(Ranked { rank, value: *kind });
                }
            }

            ClaimPredicate::EpisodeTitle(title) => {
                if fields
                    .episode_title
                    .as_ref()
                    .is_none_or(|episode_title| rank > episode_title.rank)
                {
                    fields.episode_title = Some(Ranked {
                        rank,
                        value: title.clone(),
                    });
                }
            }
        }
    }
    // An extra's name, e.g. "Deleted Scenes", tends to find some other short on
    // the knowledge base, which then outranks the file being an extra. Only
    // look up IDs, such as asserted ones.
    if is_extra {
        fields.titles.clear();
        fields.series_names.clear();
        fields.last_resort_titles.clear();
        fields.last_resort_movie_titles.clear();
        fields.last_resort_series_names.clear();
    }
    // An episode's file name, or title tag, is its release's name, e.g.
    // `Babylon.Berlin.S02E01.BluRay.1080i.Remux-QuUxTeR`, and the titles
    // taken from it still have the episode's marker or the release's words.
    // No movie or series is titled so, and the series is searched for by its
    // name anyway, so such a title would only cost a search or two. A feature's
    // is kept: its release's words are sometimes a title's.
    if fields.is_episode() {
        fields.titles.retain(|title| !is_release_name(&title.value));
    }
    let searchable = !fields.ids.is_empty()
        || !fields.series_ids.is_empty()
        || !fields.titles.is_empty()
        || !fields.series_names.is_empty()
        || !fields.last_resort_titles.is_empty()
        || !fields.last_resort_movie_titles.is_empty()
        || !fields.last_resort_series_names.is_empty();
    searchable.then_some(fields)
}

impl RankedSearchFields {
    /// Whether the file has an episode's numbers. A series is no place for a
    /// file without them, so one found by the file's title would only keep
    /// it out of the catalog.
    pub fn is_episode(&self) -> bool {
        (self.season.is_some() && self.episode_range.is_some())
            || self.absolute_episode_range.is_some()
    }
}

/// The claims from one knowledge base result, e.g. one TMDB movie. They're all
/// about the same work.
pub type KbResult = Vec<Ranked<ClaimPredicate>>;

// TODO: some way to hint if we're searching for movie or TV
//
/// A knowledge base is a claim source that can be queried by a specific set
/// of [`RankedSearchFields`]. Every [`KnowledgeBaseApi`] is one, searched by
/// [`search::search_for_claims`]; a test may answer with fixed claims instead.
#[async_trait]
pub trait KnowledgeBase: Sync {
    async fn search_for_claims(
        &self,
        fields: &RankedSearchFields,
    ) -> Result<(Vec<KbResult>, Vec<KbQueryEvent>), KnowledgeBaseError>;

    async fn get_movie_details(&self, id: u32) -> Result<Option<MovieKbData>, KnowledgeBaseError>;

    async fn get_tv_series_details(
        &self,
        id: u32,
    ) -> Result<Option<TvSeriesKbData>, KnowledgeBaseError>;
}

#[async_trait]
impl<K: KnowledgeBaseApi> KnowledgeBase for K {
    async fn search_for_claims(
        &self,
        fields: &RankedSearchFields,
    ) -> Result<(Vec<KbResult>, Vec<KbQueryEvent>), KnowledgeBaseError> {
        search::search_for_claims(self, fields).await
    }

    async fn get_movie_details(&self, id: u32) -> Result<Option<MovieKbData>, KnowledgeBaseError> {
        KnowledgeBaseApi::get_movie_details(self, id).await
    }

    async fn get_tv_series_details(
        &self,
        id: u32,
    ) -> Result<Option<TvSeriesKbData>, KnowledgeBaseError> {
        KnowledgeBaseApi::get_tv_series_details(self, id).await
    }
}

/// What a knowledge base, e.g. TMDB, can be asked: its movies and series, by
/// ID or by name, and the seasons of its series. How a file's claims are
/// searched for with it, and how its answers are ranked, is up to
/// [`search::search_for_claims`], the same for every knowledge base.
#[async_trait]
pub trait KnowledgeBaseApi: Sync {
    /// The movie that `id` names: one of the knowledge base's own IDs, or
    /// another's that it can look movies up by, e.g. an IMDb ID. Logs the
    /// queries it makes to `log`.
    async fn movie(
        &self,
        id: &ExternalId,
        log: &mut Vec<KbQueryEvent>,
    ) -> Result<Option<MovieRecord>, KnowledgeBaseError>;

    /// The series that `id` names, as [`Self::movie`] finds a movie.
    async fn series(
        &self,
        id: &ExternalId,
        log: &mut Vec<KbQueryEvent>,
    ) -> Result<Option<SeriesRecord>, KnowledgeBaseError>;

    /// A season of a series, by the series' ID on the knowledge base, with
    /// its episodes, e.g. season 0, its specials.
    async fn season(
        &self,
        series: &ExternalId,
        number: u32,
        log: &mut Vec<KbQueryEvent>,
    ) -> Result<Option<Season>, KnowledgeBaseError>;

    /// Movies titled like `query`, and released in `year`, if given, in the
    /// knowledge base's order. Not logged: the search logs the one of its
    /// searches that it goes by.
    async fn search_movies(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> Result<Vec<MovieRecord>, KnowledgeBaseError>;

    /// Series named like `query`, and first aired in `year`, if given, as
    /// [`Self::search_movies`] finds movies.
    async fn search_series(
        &self,
        query: &str,
        year: Option<u32>,
    ) -> Result<Vec<SeriesRecord>, KnowledgeBaseError>;

    /// A movie's other titles, e.g. in other languages or romanized, by
    /// which its search may have found it. None, for a knowledge base that
    /// doesn't list them.
    async fn movie_alternative_titles(
        &self,
        _id: &ExternalId,
        _log: &mut Vec<KbQueryEvent>,
    ) -> Result<Vec<String>, KnowledgeBaseError> {
        Ok(Vec::new())
    }

    /// A series' other names, as [`Self::movie_alternative_titles`] a
    /// movie's.
    async fn series_alternative_titles(
        &self,
        _id: &ExternalId,
        _log: &mut Vec<KbQueryEvent>,
    ) -> Result<Vec<String>, KnowledgeBaseError> {
        Ok(Vec::new())
    }

    async fn get_movie_details(&self, id: u32) -> Result<Option<MovieKbData>, KnowledgeBaseError>;

    async fn get_tv_series_details(
        &self,
        id: u32,
    ) -> Result<Option<TvSeriesKbData>, KnowledgeBaseError>;
}

/// See [`search::probe_last_resorts`].
#[cfg(feature = "corpus")]
pub(crate) async fn probe_last_resorts(
    kb: &dyn KnowledgeBaseApi,
    fields: &RankedSearchFields,
) -> Result<Option<Vec<(String, Option<String>, Vec<KbResult>)>>, KnowledgeBaseError> {
    search::probe_last_resorts(kb, fields).await
}

/// A movie, as a knowledge base lists it.
#[derive(Clone, Debug)]
pub struct MovieRecord {
    /// Its ID on the knowledge base.
    pub id: ExternalId,
    pub title: String,
    pub original_title: String,
    pub release_year: Option<u32>,
    /// How long it is, if the knowledge base says. A search's results may
    /// not, as TMDB's don't, but [`KnowledgeBaseApi::movie`]'s do.
    pub runtime_minutes: Option<u32>,
    /// Its IMDb ID, if the knowledge base gives it.
    pub imdb_id: Option<String>,
}

/// A series, as a knowledge base lists it.
#[derive(Clone, Debug)]
pub struct SeriesRecord {
    /// Its ID on the knowledge base.
    pub id: ExternalId,
    pub name: String,
    pub original_name: String,
    pub first_air_year: Option<u32>,
    /// Its seasons, with its specials as season 0. Empty for a search's
    /// results, which may not list them, as TMDB's don't.
    pub seasons: Vec<SeasonSummary>,
}

/// A season of a series, without its episodes.
#[derive(Clone, Debug)]
pub struct SeasonSummary {
    pub number: u32,
    pub name: String,
    /// As many as the knowledge base lists, which can be 0 for a season it
    /// knows no episodes of yet.
    pub episode_count: u32,
}

/// A season of a series, with its episodes.
#[derive(Clone, Debug)]
pub struct Season {
    pub number: u32,
    pub episodes: Vec<Episode>,
}

#[derive(Clone, Debug)]
pub struct Episode {
    pub number: u32,
    pub name: String,
    pub runtime_minutes: Option<u32>,
    /// The studio's code for the episode, e.g. `DR-19`, which some releases
    /// name their files by.
    pub production_code: Option<String>,
    /// When it first aired, if the knowledge base says.
    pub aired: Option<Date>,
}

/// A knowledge base's failure to answer, over what went wrong beneath, which
/// is its source. `{:#}` adds each cause after a colon, as a report's does,
/// for recording the failure where there's no report.
#[derive(Debug)]
pub struct KnowledgeBaseError(anyhow::Error);

impl KnowledgeBaseError {
    /// The same failure, with `context` as what it was part of.
    pub(crate) fn context(self, context: impl fmt::Display + Send + Sync + 'static) -> Self {
        Self(self.0.context(context))
    }
}

impl From<anyhow::Error> for KnowledgeBaseError {
    fn from(error: anyhow::Error) -> Self {
        Self(error)
    }
}

impl fmt::Display for KnowledgeBaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("knowledge base search failed")?;
        if f.alternate() {
            write!(f, ": {:#}", self.0)?;
        }
        Ok(())
    }
}

impl std::error::Error for KnowledgeBaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.0)
    }
}

/// A run of a release's episodes numbered without a season from 1, named
/// alike, e.g. `mahoromatic ~motto utsukushii mono~` 01 to 14.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeRun {
    /// The name its episodes give their series.
    pub name: String,
    /// How many episodes it has.
    pub episodes: u32,
    /// Its last episode's number.
    pub last: u32,
    /// Its release's runs named as another of them and more, with how many
    /// episodes each has, if it's one of them, e.g. `gochuumon wa usagi desu
    /// ka`, `gochuumon wa usagi desu ka ~bloom~` and `gochuumon wa usagi desu
    /// ka??`. Empty if it isn't.
    pub family: Vec<(String, u32)>,
}

#[derive(Clone, Debug, Default)]
pub struct RankedSearchFields {
    /// IDs to look up as movies, and failing that as TV series.
    pub ids: Vec<Ranked<ExternalId>>,
    /// IDs to look up as TV series only. A TMDB ID doesn't say which kind of
    /// work it's of, and a series' is often some movie's too.
    pub series_ids: Vec<Ranked<ExternalId>>,
    /// Titles to search for, as movies and failing that as TV series.
    pub titles: Vec<Ranked<String>>,
    /// Names to search for as TV series only.
    pub series_names: Vec<Ranked<String>>,
    pub runtime: Option<Duration>,
    pub release_year: Option<u32>,
    /// The year the series searched for by name first aired.
    pub series_year: Option<u32>,
    /// Which part of its work the file is, e.g. 2 for `Part Two`.
    pub part: Option<u32>,
    /// The series the file's name marks it as a special of, e.g. an OVA.
    pub special_of: Option<Ranked<String>>,
    /// The season and episodes the file is of, if it's named like an episode.
    pub season: Option<u32>,
    pub episode_range: Option<[u32; 2]>,
    /// The episodes, numbered through the whole series, if the file is named
    /// like an episode without a season.
    pub absolute_episode_range: Option<[u32; 2]>,
    /// The title of the episode the file is, if its name gives one, which
    /// a series' episodes can be found by.
    pub episode_title: Option<Ranked<String>>,
    /// How long the parts of the file's work last together, measured, if the
    /// file is one of its numbered parts, e.g. a concert's `[01]` and `[02]`.
    /// Not a claim: what's claimed is the parts, and this is their durations'
    /// sum.
    pub parts_runtime: Option<Duration>,
    /// How many episodes of the file's season its release has, e.g. 3 for
    /// a season pack of three, to tell a series by from another of its
    /// name, e.g. 2010's "Upstairs Downstairs", whose first season is 3
    /// episodes long, from 1971's, whose is 13. Not a claim either.
    pub season_episodes: Option<u32>,
    /// The run of its release's episodes, numbered without a season from 1,
    /// that the file is in, if it is, to tell a later season named as its
    /// own, e.g. `Mahoromatic ~Motto Utsukushii Mono~ 01` to `14`, from a
    /// run numbered through the whole series. Not a claim either.
    pub run: Option<EpisodeRun>,
    /// For a file among videos in its directory that aren't mostly movies',
    /// e.g. a season's or a series' whole run, whose own titles are episodes'
    /// rather than a name of their series: the words their titles all begin
    /// with, if any, to look the series up by once for them all. Not a claim
    /// either.
    pub batch: Option<Option<String>>,
    /// Titles, and series' names, that are only a last resort, e.g. cut
    /// before their subtitles, looked up only if nothing else finds a work,
    /// and only for a work titled as they begin: as movies and failing that
    /// as series, and as series only.
    pub last_resort_titles: Vec<Ranked<String>>,
    /// Titles that are only a last resort, as movies only, e.g. one cut to a
    /// word from a feature's name, which names too little for a series.
    pub last_resort_movie_titles: Vec<Ranked<String>>,
    pub last_resort_series_names: Vec<Ranked<String>>,
    /// Whether the file is a movie's or a series', as best its name and its
    /// release say, which only shapes how it's searched for.
    pub kind: Option<Ranked<WorkKind>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wanshitong_core::domain::ClaimSource;

    fn claim(pred: ClaimPredicate) -> RankedClaimPredicate {
        RankedClaimPredicate::new(ClaimSource::Assertion, 1.0, pred)
    }

    /// A release's name, as [`find_kb_search_fields`] is told it: one with
    /// the release's words or an episode's marker in it.
    fn is_release_name(title: &str) -> bool {
        title.contains("1080i") || title.contains("s02e01")
    }

    fn titles(fields: &RankedSearchFields) -> Vec<&str> {
        fields.titles.iter().map(|t| t.value.as_str()).collect()
    }

    #[test]
    fn an_episode_is_only_searched_for_by_titles_that_are_not_its_releases_name() {
        let claims = [
            claim(ClaimPredicate::TvSeriesName("babylon berlin".into())),
            claim(ClaimPredicate::Season(2)),
            claim(ClaimPredicate::EpisodeRange([1, 1])),
            claim(ClaimPredicate::MovieTitle("babylon berlin s02e01".into())),
            claim(ClaimPredicate::MovieTitle(
                "babylon berlin s02e01 bluray 1080i remux-quuxter".into(),
            )),
            claim(ClaimPredicate::MovieTitle("babylon berlin".into())),
        ];

        let fields = find_kb_search_fields(&claims, is_release_name).unwrap();

        assert_eq!(titles(&fields), ["babylon berlin"]);
        assert_eq!(fields.series_names.len(), 1);
    }

    #[test]
    fn an_episode_with_only_its_releases_name_for_a_title_is_not_searched_for() {
        let claims = [
            claim(ClaimPredicate::AbsoluteEpisodeRange([3, 3])),
            claim(ClaimPredicate::MovieTitle("babylon berlin s02e01".into())),
        ];

        assert!(find_kb_search_fields(&claims, is_release_name).is_none());
    }

    #[test]
    fn a_feature_is_searched_for_by_its_releases_name_too() {
        let claims = [
            claim(ClaimPredicate::MovieTitle("the title 1080i remux".into())),
            claim(ClaimPredicate::MovieTitle("the title".into())),
        ];

        let fields = find_kb_search_fields(&claims, is_release_name).unwrap();

        assert_eq!(titles(&fields), ["the title 1080i remux", "the title"]);
    }
}
