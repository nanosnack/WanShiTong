//! Building the catalog's works (movies, TV series) from resolved claims.

use super::knowledge_base::KnowledgeBase;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
};
use tracing::{error, warn};
use wanshitong_core::{
    commands::{
        NewCatalog, NewEpisode, NewInterpretationError, NewMovie, NewMovieExpression, NewMovieFile,
        NewTvSeries,
    },
    domain::{
        ClaimPredicate, DataSourceUuid, ExternalId, FileFacts, FileMeasurementRowId, FileRole,
        MainFileLocation, MovieKbData, PathId, Rank, RankedClaimPredicate, SubjectAuditLog,
        SubjectIndex, TvSeriesKbData, manifestations_of,
    },
};

/// Convert subject-indexed works into a catalog keyed by file row IDs.
pub(super) fn new_catalog(
    files: &[(PathId, FileFacts)],
    works: Works,
    audit_logs: HashMap<SubjectIndex, SubjectAuditLog>,
) -> NewCatalog {
    let file_id = |subject: SubjectIndex| files[subject].1.row_id;
    let Works {
        movies,
        tv_series,
        excluded,
    } = works;
    let movies: Vec<NewMovie> = movies
        .into_iter()
        .map(
            |BuiltMovie {
                 files: movie_files,
                 movie,
             }| {
                let Movie {
                    title,
                    tmdb_id,
                    imdb_id,
                    tvdb_id,
                    release_year,
                    kb_data,
                } = movie;
                let MovieManifestations {
                    expressions,
                    realizes,
                    manifestations,
                    parts,
                } = movie_manifestations(files, &movie_files);
                NewMovie {
                    title,
                    tmdb_id,
                    imdb_id,
                    tvdb_id,
                    release_year,
                    kb_data,
                    expressions,
                    files: movie_files
                        .into_iter()
                        .zip(realizes)
                        .zip(manifestations)
                        .zip(parts)
                        .map(|(((file, expression), manifestation), part)| {
                            let MovieFile { subject, role, .. } = file;
                            NewMovieFile {
                                file_id: file_id(subject),
                                role,
                                expression,
                                manifestation,
                                part,
                            }
                        })
                        .collect(),
                }
            },
        )
        .collect();
    let tv_series: Vec<NewTvSeries> = tv_series
        .into_iter()
        .map(|series| {
            let BuiltSeries {
                name,
                original_name,
                tmdb_id,
                imdb_id,
                tvdb_id,
                kb_data,
                files,
            } = series;
            let mut episodes = Vec::new();
            let mut extras = Vec::new();
            let mut discs = Vec::new();
            for (subject, file) in files {
                match file.part {
                    SeriesPart::Episode {
                        season_number,
                        episode_range,
                        episode_title,
                    } => episodes.push(NewEpisode {
                        file_id: file_id(subject),
                        season_number,
                        episode_range,
                        episode_title,
                    }),
                    SeriesPart::Extra => extras.push(file_id(subject)),
                    SeriesPart::Disc => discs.push(file_id(subject)),
                }
            }
            NewTvSeries {
                name,
                original_name,
                tmdb_id,
                imdb_id,
                tvdb_id,
                kb_data,
                episodes,
                extras,
                discs,
            }
        })
        .collect();
    let audit_logs = audit_logs
        .into_iter()
        .map(|(subject, log)| (file_id(subject), log))
        .collect();
    let excluded: HashSet<FileMeasurementRowId> = excluded.into_iter().map(file_id).collect();
    let unidentified_videos = unidentified_videos(files, &movies, &tv_series, &excluded);
    let videos = files
        .iter()
        .map(|(_, facts)| facts)
        .filter(|facts| facts.is_video())
        .map(|facts| facts.row_id)
        .collect();
    NewCatalog {
        movies,
        tv_series,
        audit_logs,
        unidentified_videos,
        videos,
    }
}

/// A movie's manifestations and expressions, by its files. See *Expression* and
/// *Manifestation* in `spec/domain-model.md`.
struct MovieManifestations {
    expressions: Vec<NewMovieExpression>,
    /// For each file, the expression its manifestation realizes, by its position in
    /// `expressions`, or None if it isn't a main file.
    realizes: Vec<Option<usize>>,
    /// For each file, which manifestation it's in, or None if it isn't a main file.
    manifestations: Vec<Option<usize>>,
    /// For each file, which part of its manifestation it is, or None if it
    /// isn't a main file or its manifestation is one file. A title's part,
    /// e.g. `The Godfather Part II`, isn't a part of its manifestation.
    parts: Vec<Option<u32>>,
}

/// A movie's manifestations, by where its main files are and which parts they're
/// numbered as (see [`manifestations_of`]), and its expressions, with which of them
/// each manifestation realizes: the edition its main files agree on, or the
/// unspecified expression if they claim none or disagree. The unspecified
/// expression comes first, then the others by edition.
fn movie_manifestations(
    files: &[(PathId, FileFacts)],
    movie_files: &[MovieFile],
) -> MovieManifestations {
    // In the order of their paths, so that the manifestations are numbered by them,
    // whatever order the claims put the files in.
    let mut mains: Vec<&MovieFile> = movie_files
        .iter()
        .filter(|file| file.role == Some(FileRole::Main))
        .collect();
    mains.sort_by(|a, b| files[a.subject].0.cmp(&files[b.subject].0));
    let locations: Vec<MainFileLocation<DataSourceUuid>> = mains
        .iter()
        .map(|file| {
            let (path_id, facts) = &files[file.subject];
            MainFileLocation {
                data_source: path_id.data_source,
                rel_path: &path_id.rel_path,
                device_inode: facts.file.device.zip(facts.file.inode),
                part: file.part,
            }
        })
        .collect();
    let manifestations = manifestations_of(&locations);
    let manifestation_count = manifestations.iter().max().map_or(0, |last| last + 1);
    let manifestation_editions: Vec<Option<&str>> = (0..manifestation_count)
        .map(|manifestation| {
            let mut editions = mains
                .iter()
                .zip(&manifestations)
                .filter(|&(_, &c)| c == manifestation)
                .map(|(file, _)| file.edition.as_deref());
            let first = editions.next().flatten();
            editions
                .all(|edition| edition == first)
                .then_some(first)
                .flatten()
        })
        .collect();
    let editions: BTreeSet<Option<&str>> = manifestation_editions.iter().copied().collect();
    let expressions = editions
        .iter()
        .map(|edition| NewMovieExpression {
            edition: edition.map(str::to_owned),
        })
        .collect();
    let manifestation_of_main: HashMap<SubjectIndex, usize> = mains
        .iter()
        .zip(manifestations)
        .map(|(file, manifestation)| (file.subject, manifestation))
        .collect();
    let mut sizes = vec![0usize; manifestation_count];
    for &manifestation in manifestation_of_main.values() {
        sizes[manifestation] += 1;
    }
    let mut realizes = Vec::with_capacity(movie_files.len());
    let mut manifestations = Vec::with_capacity(movie_files.len());
    let mut parts = Vec::with_capacity(movie_files.len());
    for file in movie_files {
        let manifestation = manifestation_of_main.get(&file.subject).copied();
        let edition = manifestation.map(|m| manifestation_editions[m]);
        realizes.push(edition.and_then(|edition| editions.iter().position(|e| *e == edition)));
        manifestations.push(manifestation);
        parts.push(manifestation.filter(|&m| sizes[m] > 1).and(file.part));
    }
    MovieManifestations {
        expressions,
        realizes,
        manifestations,
        parts,
    }
}

/// The video files that no identified work includes, and that weren't
/// excluded from the catalog. What a file is, beyond whether it has moving
/// pictures, is only guessed at until it's identified or excluded, so e.g.
/// files claimed to be samples are among them.
fn unidentified_videos(
    files: &[(PathId, FileFacts)],
    movies: &[NewMovie],
    tv_series: &[NewTvSeries],
    excluded: &HashSet<FileMeasurementRowId>,
) -> Vec<FileMeasurementRowId> {
    let identified: HashSet<FileMeasurementRowId> = movies
        .iter()
        .filter(|movie| movie.is_identified())
        .flat_map(|movie| movie.files.iter().map(|file| file.file_id))
        .chain(
            tv_series
                .iter()
                .filter(|series| series.is_identified())
                .flat_map(NewTvSeries::file_ids),
        )
        .collect();
    files
        .iter()
        .map(|(_, facts)| facts)
        .filter(|facts| {
            facts.is_video()
                && !identified.contains(&facts.row_id)
                && !excluded.contains(&facts.row_id)
        })
        .map(|facts| facts.row_id)
        .collect()
}

pub(super) struct Works {
    movies: Vec<BuiltMovie>,
    tv_series: Vec<BuiltSeries>,
    /// Files excluded from the catalog, and the files that follow them.
    excluded: HashSet<SubjectIndex>,
}

/// A TV series and the files of its episodes and extras.
struct BuiltSeries {
    name: String,
    original_name: Option<String>,
    tmdb_id: Option<u32>,
    imdb_id: Option<String>,
    tvdb_id: Option<u32>,
    kb_data: TvSeriesKbData,
    files: Vec<(SubjectIndex, SeriesFile)>,
}

struct BuiltMovie {
    files: Vec<MovieFile>,
    movie: Movie,
}

struct MovieFile {
    subject: SubjectIndex,
    role: Option<FileRole>,
    /// The edition the file's claims give it. For a main file, this decides
    /// which expression its manifestation realizes.
    edition: Option<String>,
    /// The part of its work the file's claims number it as, e.g. 1 for
    /// `CD1`. For a main file, this decides which manifestation it's in.
    part: Option<u32>,
}

/// Candidate file for a movie, before final role assignment.
struct MovieFileCandidate {
    subject: SubjectIndex,
    /// The FileRole claim and its rank, if any.
    role_claim: Option<(FileRole, Rank)>,
    edition: Option<String>,
    part: Option<u32>,
    movie: Movie,
}

pub(super) fn build_works(
    files: &[(PathId, FileFacts)],
    claims: HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
) -> Works {
    let mut claimed = Claimed::of(&claims);
    let (movie_candidates, mut series_files) = claimed.build_files(files);
    let mut movies: Vec<BuiltMovie> =
        group_by_identity(movie_candidates, |candidate| candidate.movie.identity())
            .into_iter()
            .filter_map(|candidates| build_movie(files, candidates))
            .collect();
    let followers = place_extras(files, &claimed, &mut movies, &mut series_files);
    let tv_series = group_by_identity(series_files, |(_, file)| file.series_identity())
        .into_iter()
        .filter_map(|series_files| build_tv_series(files, series_files))
        .collect();
    let mut excluded = claimed.excluded;
    excluded.extend(followers);
    Works {
        movies,
        tv_series,
        excluded,
    }
}

/// What the active claims say of each file, sorted by what it's for.
struct Claimed<'a> {
    /// Each file's movie and episode, as its claims build them.
    builders: HashMap<SubjectIndex, ManifestationBuilder>,
    /// Each file's role, and the rank of the claim that says so.
    roles: HashMap<SubjectIndex, (FileRole, Rank)>,
    /// Extras, with the path of their movie's main file.
    extras: HashMap<SubjectIndex, &'a str>,
    /// Files excluded from the catalog, by assertion.
    excluded: HashSet<SubjectIndex>,
    /// The edition each file is, e.g. `Director's Cut`.
    editions: HashMap<SubjectIndex, String>,
    /// The part of its work each file is numbered as, e.g. 1 for `CD1`.
    parts: HashMap<SubjectIndex, u32>,
}

impl<'a> Claimed<'a> {
    fn of(claims: &'a HashMap<SubjectIndex, Vec<RankedClaimPredicate>>) -> Self {
        let mut claimed = Claimed {
            builders: HashMap::new(),
            roles: HashMap::new(),
            extras: HashMap::new(),
            excluded: HashSet::new(),
            editions: HashMap::new(),
            parts: HashMap::new(),
        };
        for (&subject, claims) in claims {
            for RankedClaimPredicate {
                pred, rank, active, ..
            } in claims
            {
                if *active {
                    claimed.add(subject, pred, *rank);
                }
            }
        }
        claimed
    }

    fn add(&mut self, subject: SubjectIndex, pred: &'a ClaimPredicate, rank: Rank) {
        let builder: &mut ManifestationBuilder = self.builders.entry(subject).or_default();
        match pred {
            ClaimPredicate::MovieExternalId(id) => match id {
                ExternalId::Imdb(id) => builder.movie.imdb_id = Some(id.clone()),
                ExternalId::Tmdb(id) => builder.movie.tmdb_id = Some(*id),
                ExternalId::Tvdb(id) => builder.movie.tvdb_id = Some(*id),
            },
            ClaimPredicate::MovieTitle(title) => builder.movie.title = Some(title.clone()),
            ClaimPredicate::MovieReleaseYear(year) => builder.movie.release_year = Some(*year),
            ClaimPredicate::TvSeriesExternalId(id) => match id {
                ExternalId::Imdb(id) => builder.episode.series_imdb_id = Some(id.clone()),
                ExternalId::Tmdb(id) => builder.episode.series_tmdb_id = Some(*id),
                ExternalId::Tvdb(id) => builder.episode.series_tvdb_id = Some(*id),
            },
            ClaimPredicate::TvSeriesName(name) => builder.episode.series_name = Some(name.clone()),
            ClaimPredicate::Season(season_number) => {
                builder.episode.season_number = Some(*season_number)
            }
            ClaimPredicate::EpisodeRange(range) => builder.episode.episode_range = Some(*range),
            ClaimPredicate::AbsoluteEpisodeRange(range) => {
                builder.episode.absolute_episode_range = Some(*range)
            }
            ClaimPredicate::Special => builder.episode.special = true,
            ClaimPredicate::EpisodeTitle(title) => {
                builder.episode.episode_title = Some(title.clone())
            }
            // It shapes how the file's searched for, not which work it's of.
            ClaimPredicate::WorkKind(_) => {}
            ClaimPredicate::FileRole(role) => {
                self.roles.insert(subject, (*role, rank));
            }
            ClaimPredicate::ExtraOf(main_path) => {
                self.extras.insert(subject, main_path);
            }
            ClaimPredicate::Excluded => {
                self.excluded.insert(subject);
            }
            // Which cut the file is, not which work.
            ClaimPredicate::Edition(edition) => {
                self.editions.insert(subject, edition.clone());
            }
            // Which part of its work the file is, not which work.
            ClaimPredicate::Part(part) => {
                self.parts.insert(subject, *part);
            }
            // A disc's other files aren't subjects, as their disc's index
            // file stands for them, so this is only in old audit logs.
            ClaimPredicate::DiscOf(_) => (),
            // Facts to look up by, like the duration.
            ClaimPredicate::Duration(_) | ClaimPredicate::SpecialOf(_) => (),
            // Only for finding the series: once found, it's told apart by its ID.
            ClaimPredicate::TvSeriesYear(_) => (),
        }
    }

    /// Each file's movie, or its series' episode or extra, as a candidate
    /// for a movie or a series. Not the samples, which don't belong in the
    /// catalog; their audit logs still record why they were left out. Nor
    /// the excluded files, nor the extras, which join their main file's
    /// work, a movie or a series, once it's built.
    fn build_files(
        &mut self,
        files: &[(PathId, FileFacts)],
    ) -> (Vec<MovieFileCandidate>, Vec<(SubjectIndex, SeriesFile)>) {
        let mut movie_candidates = Vec::new();
        let mut series_files = Vec::new();
        for (subject, builder) in std::mem::take(&mut self.builders) {
            if self.excluded.contains(&subject)
                || self.is_sample(subject)
                || self.extras.contains_key(&subject)
            {
                continue;
            }
            let role_claim = self.roles.get(&subject).copied();
            let role = role_claim.map(|(role, _)| role);
            let (path_id, facts) = &files[subject];
            match builder.build(&path_id.rel_path, role, facts.disc.is_some()) {
                Some(Work::Movie(movie)) => movie_candidates.push(MovieFileCandidate {
                    subject,
                    role_claim,
                    edition: self.editions.remove(&subject),
                    part: self.parts.remove(&subject),
                    movie,
                }),
                Some(Work::Series(file)) => series_files.push((subject, file)),
                None => (),
            }
        }
        (movie_candidates, series_files)
    }

    fn is_sample(&self, subject: SubjectIndex) -> bool {
        matches!(self.roles.get(&subject), Some((FileRole::Sample, _)))
    }
}

/// Adds the extras to their main file's movie, or to its series if it's an
/// episode. An extra of a file that isn't in the catalog is left out, and
/// logged, once per file. One of an excluded file is excluded with it: the
/// followers are returned.
fn place_extras(
    files: &[(PathId, FileFacts)],
    claimed: &Claimed,
    movies: &mut [BuiltMovie],
    series_files: &mut Vec<(SubjectIndex, SeriesFile)>,
) -> Vec<SubjectIndex> {
    let path = |subject: SubjectIndex| {
        let path_id = &files[subject].0;
        (path_id.data_source, path_id.rel_path.as_path())
    };
    let movie_by_file: HashMap<(DataSourceUuid, &Path), usize> = movies
        .iter()
        .enumerate()
        .flat_map(|(i, movie)| movie.files.iter().map(move |file| (path(file.subject), i)))
        .collect();
    let series_file_by_file: HashMap<(DataSourceUuid, &Path), usize> = series_files
        .iter()
        .enumerate()
        .map(|(i, (subject, _))| (path(*subject), i))
        .collect();
    let excluded_files: HashSet<(DataSourceUuid, &Path)> = claimed
        .excluded
        .iter()
        .map(|&subject| path(subject))
        .collect();
    let mut followers = Vec::new();
    let mut series_extras = Vec::new();
    // Extras left out, by the file they're of, to log once per file.
    let mut left_out = BTreeMap::<&str, Vec<&Path>>::new();
    for (&subject, &main_path) in &claimed.extras {
        if claimed.is_sample(subject) {
            continue;
        }
        let (data_source, extra_path) = path(subject);
        let main = (data_source, Path::new(main_path));
        if excluded_files.contains(&main) {
            followers.push(subject);
        } else if let Some(&i) = movie_by_file.get(&main) {
            movies[i].files.push(MovieFile {
                subject,
                role: Some(FileRole::Extra),
                edition: None,
                part: None,
            });
        } else if let Some(&i) = series_file_by_file.get(&main) {
            series_extras.push((subject, series_files[i].1.extra_of_series()));
        } else {
            left_out.entry(main_path).or_default().push(extra_path);
        }
    }
    series_files.extend(series_extras);
    for (main_path, extras) in left_out {
        let example = extras.iter().min().expect("an extra was left out of it");
        warn!(
            "{main_path} isn't in the catalog, so its {} extras are left out, e.g. {}",
            extras.len(),
            example.display(),
        );
    }
    followers
}

/// How authoritative a file's IDs for its work are, as a sort key: a TMDB
/// ID first, then an IMDb ID.
fn id_authority(tmdb_id: Option<u32>, imdb_id: Option<&str>) -> (bool, bool) {
    (tmdb_id.is_some(), imdb_id.is_some())
}

/// Warns if the files grouped as one `kind` of work, named `name`, name
/// different works that share an ID, e.g. two TMDB movies with one IMDb ID.
/// They're recorded as one work: the first file's IDs win.
fn warn_of_shared_ids<'a>(
    files: &[(PathId, FileFacts)],
    kind: &str,
    name: &str,
    ids: impl Iterator<Item = (SubjectIndex, Option<u32>, Option<&'a str>)>,
) {
    let mut subjects = Vec::new();
    let mut tmdb_ids = BTreeSet::new();
    let mut imdb_ids = BTreeSet::new();
    for (subject, tmdb_id, imdb_id) in ids {
        subjects.push(subject);
        tmdb_ids.extend(tmdb_id);
        imdb_ids.extend(imdb_id);
    }
    if tmdb_ids.len() > 1 || imdb_ids.len() > 1 {
        let paths: Vec<String> = subjects
            .iter()
            .map(|&subject| files[subject].0.rel_path.display().to_string())
            .collect();
        warn!(
            "These files name different works that share an ID \
             (TMDB {tmdb_ids:?}, IMDB {imdb_ids:?}), so they're recorded as \
             one {kind}, {name:?}: {}",
            paths.join(", "),
        );
    }
}

/// Builds a movie from the files that are the same movie.
///
/// Each file keeps the role it has evidence for, if any. A movie may have
/// several main files, e.g. manifestations in different places, or none, e.g. when
/// nothing says which of its files is the feature.
///
/// The movie's metadata comes from the file with the most authoritative ID
/// (TMDB, then IMDB), then the best-evidenced main file, then the first by
/// path, so that it doesn't depend on claim order. IDs that file lacks come
/// from the others in the same order, as they're all the same movie.
fn build_movie(
    files: &[(PathId, FileFacts)],
    mut candidates: Vec<MovieFileCandidate>,
) -> Option<BuiltMovie> {
    let main_rank = |candidate: &MovieFileCandidate| match candidate.role_claim {
        Some((FileRole::Main, rank)) => Some(rank),
        _ => None,
    };
    let authority = |candidate: &MovieFileCandidate| {
        id_authority(candidate.movie.tmdb_id, candidate.movie.imdb_id.as_deref())
    };
    candidates.sort_by(|a, b| {
        authority(b)
            .cmp(&authority(a))
            .then_with(|| main_rank(b).cmp(&main_rank(a)))
            .then_with(|| files[a.subject].0.cmp(&files[b.subject].0))
    });
    warn_of_shared_ids(
        files,
        "movie",
        &candidates.first()?.movie.title,
        candidates.iter().map(|candidate| {
            (
                candidate.subject,
                candidate.movie.tmdb_id,
                candidate.movie.imdb_id.as_deref(),
            )
        }),
    );

    let mut candidates = candidates.into_iter();
    let first = candidates.next()?;
    let mut movie = first.movie;
    let mut movie_files = vec![MovieFile {
        subject: first.subject,
        role: first.role_claim.map(|(role, _)| role),
        edition: first.edition,
        part: first.part,
    }];
    for candidate in candidates {
        movie.tmdb_id = movie.tmdb_id.or(candidate.movie.tmdb_id);
        movie.imdb_id = movie.imdb_id.or(candidate.movie.imdb_id);
        movie.tvdb_id = movie.tvdb_id.or(candidate.movie.tvdb_id);
        movie_files.push(MovieFile {
            subject: candidate.subject,
            role: candidate.role_claim.map(|(role, _)| role),
            edition: candidate.edition,
            part: candidate.part,
        });
    }
    Some(BuiltMovie {
        files: movie_files,
        movie,
    })
}

/// Builds a series from the files of its episodes.
///
/// The series' name comes from the file with the most authoritative ID (TMDB,
/// then IMDB), then the first by path, so that it doesn't depend on claim
/// order. IDs that file lacks come from the others in the same order, as
/// they're all the same series.
fn build_tv_series(
    files: &[(PathId, FileFacts)],
    mut episodes: Vec<(SubjectIndex, SeriesFile)>,
) -> Option<BuiltSeries> {
    let authority =
        |file: &SeriesFile| id_authority(file.series_tmdb_id, file.series_imdb_id.as_deref());
    episodes.sort_by(|(a_subject, a), (b_subject, b)| {
        authority(b)
            .cmp(&authority(a))
            .then_with(|| files[*a_subject].0.cmp(&files[*b_subject].0))
    });
    warn_of_shared_ids(
        files,
        "series",
        &episodes.first()?.1.series_name,
        episodes.iter().map(|(subject, file)| {
            (
                *subject,
                file.series_tmdb_id,
                file.series_imdb_id.as_deref(),
            )
        }),
    );

    let first = &episodes.first()?.1;
    let mut series = BuiltSeries {
        name: first.series_name.clone(),
        original_name: first.series_original_name.clone(),
        tmdb_id: None,
        imdb_id: None,
        tvdb_id: None,
        kb_data: Default::default(),
        files: Vec::new(),
    };
    for (_, episode) in &episodes {
        series.tmdb_id = series.tmdb_id.or(episode.series_tmdb_id);
        series.imdb_id = series.imdb_id.clone().or(episode.series_imdb_id.clone());
        series.tvdb_id = series.tvdb_id.or(episode.series_tvdb_id);
    }
    series.files = episodes;
    Some(series)
}

/// A key that files of the same work share: an external ID, or for files
/// without one, the normalized title and year.
#[derive(Clone, Eq, PartialEq, Hash)]
enum Identity {
    TmdbId(u32),
    ImdbId(String),
    TvdbId(u32),
    TitleYear(String, Option<u32>),
}

impl Identity {
    /// The work's external IDs, or failing those its normalized title and
    /// year.
    fn keys(
        tmdb_id: Option<u32>,
        imdb_id: Option<&str>,
        tvdb_id: Option<u32>,
        title: &str,
        year: Option<u32>,
    ) -> Vec<Self> {
        let ids: Vec<Self> = [
            tmdb_id.map(Self::TmdbId),
            imdb_id.map(|id| Self::ImdbId(id.into())),
            tvdb_id.map(Self::TvdbId),
        ]
        .into_iter()
        .flatten()
        .collect();
        if ids.is_empty() {
            vec![Self::TitleYear(title.to_lowercase(), year)]
        } else {
            ids
        }
    }
}

impl Movie {
    /// The keys that files of this movie share.
    fn identity(&self) -> Vec<Identity> {
        Identity::keys(
            self.tmdb_id,
            self.imdb_id.as_deref(),
            self.tvdb_id,
            &self.title,
            self.release_year,
        )
    }
}

impl SeriesFile {
    /// The keys that files of this file's series share.
    fn series_identity(&self) -> Vec<Identity> {
        Identity::keys(
            self.series_tmdb_id,
            self.series_imdb_id.as_deref(),
            self.series_tvdb_id,
            &self.series_name,
            None,
        )
    }
}

/// Groups the files that are the same work: those that share any external ID,
/// directly or through other files, and those without IDs that share a title
/// and year. So no two works share an ID.
fn group_by_identity<T>(candidates: Vec<T>, keys: impl Fn(&T) -> Vec<Identity>) -> Vec<Vec<T>> {
    // Union-find over the candidates.
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut parent: Vec<usize> = (0..candidates.len()).collect();
    let mut first_with_key = HashMap::<Identity, usize>::new();
    for (i, candidate) in candidates.iter().enumerate() {
        for key in keys(candidate) {
            let first = *first_with_key.entry(key).or_insert(i);
            let (a, b) = (root(&mut parent, first), root(&mut parent, i));
            parent[b] = a;
        }
    }

    let mut groups = HashMap::<usize, Vec<T>>::new();
    for (i, candidate) in candidates.into_iter().enumerate() {
        groups
            .entry(root(&mut parent, i))
            .or_default()
            .push(candidate);
    }
    groups.into_values().collect()
}

/// Fills in each identified work's details from the knowledge base. A work
/// whose details couldn't be fetched keeps its catalog entry without them,
/// and the failure is noted in `errors`.
pub(super) async fn enrich_with_kb_data(
    kb: &dyn KnowledgeBase,
    works: &mut Works,
    errors: &mut Vec<NewInterpretationError>,
) {
    let Works {
        movies, tv_series, ..
    } = works;
    for BuiltMovie { movie, .. } in movies {
        let Some(id) = movie.tmdb_id else { continue };
        match kb.get_movie_details(id).await {
            Ok(Some(details)) => movie.kb_data = details,
            Ok(None) => {}
            Err(e) => {
                let subject = format!(
                    "Fetching the details of movie \"{}\" (TMDB {id})",
                    movie.title
                );
                warn!("{subject} failed: {e:#}");
                errors.push(NewInterpretationError {
                    file_id: None,
                    subject,
                    error: format!("{e:#}"),
                });
            }
        }
    }
    for series in tv_series {
        let Some(id) = series.tmdb_id else { continue };
        match kb.get_tv_series_details(id).await {
            Ok(Some(details)) => series.kb_data = details,
            Ok(None) => {}
            Err(e) => {
                let subject = format!(
                    "Fetching the details of series \"{}\" (TMDB {id})",
                    series.name
                );
                warn!("{subject} failed: {e:#}");
                errors.push(NewInterpretationError {
                    file_id: None,
                    subject,
                    error: format!("{e:#}"),
                });
            }
        }
    }
}

#[derive(Default)]
struct ManifestationBuilder {
    movie: MovieBuilder,
    episode: EpisodeBuilder,
}
#[derive(Debug)]
enum Work {
    Movie(Movie),
    Series(SeriesFile),
}

impl ManifestationBuilder {
    fn build(self, path: &Path, role: Option<FileRole>, is_disc: bool) -> Option<Work> {
        let Self { movie, episode } = self;
        let movie = movie.build();
        let episode = episode.build(role, is_disc);
        if let (Some(movie), Some(episode)) = (&movie, &episode) {
            error!(
                "{}: failed to choose work:\nMovie = {movie:?}\nEpisode = {episode:?}",
                path.display()
            );
        }
        movie.map(Work::Movie).or(episode.map(Work::Series))
    }
}

#[derive(Default)]
struct MovieBuilder {
    title: Option<String>,
    tmdb_id: Option<u32>,
    imdb_id: Option<String>,
    tvdb_id: Option<u32>,
    release_year: Option<u32>,
}

#[derive(Debug, Default)]
struct Movie {
    title: String,
    tmdb_id: Option<u32>,
    imdb_id: Option<String>,
    tvdb_id: Option<u32>,
    release_year: Option<u32>,
    kb_data: MovieKbData,
}

impl MovieBuilder {
    fn build(self) -> Option<Movie> {
        let Self {
            title,
            tmdb_id,
            imdb_id,
            tvdb_id,
            release_year,
        } = self;
        title.map(|title| Movie {
            title,
            release_year,
            tvdb_id,
            tmdb_id,
            imdb_id,
            kb_data: Default::default(),
        })
    }
}

#[derive(Default)]
struct EpisodeBuilder {
    series_name: Option<String>,
    series_original_name: Option<String>,
    series_tvdb_id: Option<u32>,
    series_tmdb_id: Option<u32>,
    series_imdb_id: Option<String>,
    season_number: Option<u32>,
    episode_range: Option<[u32; 2]>,
    absolute_episode_range: Option<[u32; 2]>,
    /// Whether the file is a special of the series, e.g. an OVA.
    special: bool,
    episode_title: Option<String>,
}
/// A file of a TV series: one of its episodes, or an extra.
#[derive(Debug)]
struct SeriesFile {
    // PERF: should we register series elsewhere and refer to them via index?
    series_name: String,
    series_original_name: Option<String>,
    series_tvdb_id: Option<u32>,
    series_tmdb_id: Option<u32>,
    series_imdb_id: Option<String>,
    part: SeriesPart,
}

impl SeriesFile {
    /// A file that's an extra of this file's series.
    fn extra_of_series(&self) -> Self {
        Self {
            series_name: self.series_name.clone(),
            series_original_name: self.series_original_name.clone(),
            series_tvdb_id: self.series_tvdb_id,
            series_tmdb_id: self.series_tmdb_id,
            series_imdb_id: self.series_imdb_id.clone(),
            part: SeriesPart::Extra,
        }
    }
}

#[derive(Debug)]
enum SeriesPart {
    Episode {
        /// None when nothing says the season, and `episode_range` numbers the
        /// episodes through the whole series.
        season_number: Option<u32>,
        episode_range: [u32; 2],
        episode_title: Option<String>,
    },
    /// Part of the series without being an episode of it, e.g. a creditless
    /// opening.
    Extra,
    /// A disc kept with the series, whose titles aren't its episodes.
    Disc,
}

impl EpisodeBuilder {
    /// A file of a named series with the Extra role is an extra of it, and one
    /// with a season and episodes is an episode. So is one with episodes
    /// numbered through the whole series, whose season is unknown. A special
    /// without episode numbers, e.g. an OVA, is kept with its extras.
    fn build(self, role: Option<FileRole>, is_disc: bool) -> Option<SeriesFile> {
        let part = match (
            role,
            self.season_number,
            self.episode_range,
            self.absolute_episode_range,
        ) {
            // A disc is the series' disc, as its titles aren't cataloged
            // separately, whatever it was taken for.
            _ if is_disc => SeriesPart::Disc,
            (Some(FileRole::Extra), ..) => SeriesPart::Extra,
            (_, Some(season_number), Some(episode_range), _) => SeriesPart::Episode {
                season_number: Some(season_number),
                episode_range,
                episode_title: self.episode_title,
            },
            (_, _, _, Some(episode_range)) => SeriesPart::Episode {
                season_number: None,
                episode_range,
                episode_title: self.episode_title,
            },
            _ if self.special => SeriesPart::Extra,
            // Of an identified series, but numbered as no episode of it, e.g.
            // as its numberings lost their episodes to other files' (see
            // *Belief Policy* in `spec/domain-model.md`): it's kept with the
            // series rather than left out of the catalog.
            _ if self.series_tmdb_id.is_some()
                || self.series_tvdb_id.is_some()
                || self.series_imdb_id.is_some() =>
            {
                SeriesPart::Extra
            }
            _ => return None,
        };
        Some(SeriesFile {
            series_name: self.series_name?,
            series_original_name: self.series_original_name,
            series_tvdb_id: self.series_tvdb_id,
            series_tmdb_id: self.series_tmdb_id,
            series_imdb_id: self.series_imdb_id,
            part,
        })
    }
}

// Two files can both be episode 2 of one show: they're two manifestations of
// it (see *Manifestation* in `spec/domain-model.md`). Which a consumer should
// prefer is a policy of its own, not the catalog's.
