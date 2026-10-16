//! The movie directory heuristic: a movie's directory, its main file and its extras.

use super::{
    HeuristicView,
    extras::{is_extras_dir, is_extras_dir_name},
    names::{
        MOVIE_TITLE_REGEX, ParsedMovieTitle, RELEASE_NUMBERING_REGEX, parse_aka, parse_movie_title,
    },
    parts::{PART_REGEX, PART_TAG_REGEX},
    runs::find_runs,
    series_patterns::SeriesPatterns,
};
use regex::Regex;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Duration,
};
use wanshitong_core::domain::{
    ClaimPredicate, ClaimSource, FileFacts, FileRole, Heuristic, PathId, RankedClaimPredicate,
    SubjectIndex,
};

/// Detects movie directories, wherever they are in a data source: claims the
/// directory's title and the main role for its feature, and that files in its
/// extras subdirectories are extras of it.
///
/// Structure expected:
/// ```text
/// Movie Title (2020)/
///   Movie Title (2020).mkv      <- Main file
///   Extras/
///     Making Of.mkv             <- Extra
///     Deleted Scene.mkv         <- Extra
/// ```
///
/// A directory is a movie's when something says so: its name has a year, as
/// a release's does, or is its video's name, or it has an extras directory.
/// Not a name that says only what kind of videos it holds, e.g. `Specials`,
/// which a special's name may begin with too.
/// And when it holds one movie's worth of video: one feature, or the parts of
/// one, e.g. `CD1` and `CD2`, and not a directory of episodes. A directory of
/// several features, e.g. a box set or a collection of TV movies, is no
/// movie's, whatever its name says.
pub(super) fn movie_directory_heuristic(
    view: HeuristicView,
    claims: &mut HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
    exclusions: &HashSet<SubjectIndex>,
    rank: f32,
    extra_rank: f32,
) {
    let series_patterns = SeriesPatterns::new();
    let part_res = [
        Regex::new(PART_REGEX).unwrap(),
        Regex::new(PART_TAG_REGEX).unwrap(),
    ];
    const SOURCE: ClaimSource = ClaimSource::Heuristic(Heuristic::MovieDirectory);
    // The videos of each directory that could be a movie's: those directly in
    // it, and those in its extras directories.
    let mut movie_groups: HashMap<&Path, MovieDirGroup> = HashMap::new();
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        if exclusions.contains(&subject) {
            continue;
        }
        let path = &path_id.rel_path;
        let (movie_root, is_extra) = match extras_root(path) {
            Some(movie_root) => (movie_root, true),
            None => match path.parent() {
                Some(dir) => (dir, false),
                None => continue,
            },
        };
        movie_groups.entry(movie_root).or_default().add(
            &series_patterns,
            &part_res,
            subject,
            path_id,
            facts,
            is_extra,
        );
    }

    // A directory of a series' episodes isn't a movie's, e.g. `Show/Show S2/
    // Show S2 - 01.mkv` beside `Show/Show S2/Extras/`, whose extras are the
    // series'. Nor is a directory of extras beside them, e.g. `Show S2/NCOP/`,
    // though one of its videos stands out from the rest, e.g. a full opening
    // among creditless ones.
    let (episode_dirs, series_dirs) = episode_dirs(view, &series_patterns);
    let movie_title_re = Regex::new(MOVIE_TITLE_REGEX).unwrap();

    for (movie_root, group) in movie_groups {
        if episode_dirs.contains(movie_root) {
            continue;
        }
        if is_extras_dir(movie_root)
            && movie_root
                .ancestors()
                .skip(1)
                .any(|dir| series_dirs.contains(dir))
        {
            continue;
        }
        let Some(mains) = group.one_movies_worth() else {
            continue;
        };
        let Some(dir_name) = movie_root.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let title = parse_movie_title(&movie_title_re, dir_name);
        let kind_name = series_patterns.is_kind_name(dir_name);
        let named_so = !kind_name && title.as_ref().is_some_and(|title| title.year.is_some());
        let named_as_video = !kind_name
            && mains.iter().any(|main| {
                file_stem(&view.files[main.subject].0.rel_path).is_some_and(|stem| {
                    normalize_for_comparison(stem) == normalize_for_comparison(dir_name)
                })
            });
        if !(named_so || named_as_video || !group.extras.is_empty()) {
            continue;
        }

        let title_claims = title_claims_from_dir(&movie_title_re, movie_root, SOURCE, rank);
        if title_claims.is_empty() {
            continue;
        }
        for main in &mains {
            let main_claims = claims.entry(main.subject).or_default();
            main_claims.extend(title_claims.iter().cloned());
            main_claims.push(RankedClaimPredicate::new(
                SOURCE,
                rank,
                ClaimPredicate::FileRole(FileRole::Main),
            ));
        }

        // Files in an extras directory belong to the main file's work, whatever
        // else they seem to be on their own: its movie, or its series if other
        // claims outrank this directory being a movie's. They're extras even if
        // something outranks that, e.g. asserting which movie they're part of.
        // That role is below a Sample claim, as a `Sample/` directory is an
        // extras one. Of several parts, they're the first's.
        let main_path = view.files[mains[0].subject]
            .0
            .rel_path
            .to_string_lossy()
            .into_owned();
        for extra_subject in group.extras {
            claims.entry(extra_subject).or_default().extend([
                RankedClaimPredicate::new(
                    SOURCE,
                    extra_rank,
                    ClaimPredicate::ExtraOf(main_path.clone()),
                ),
                RankedClaimPredicate::new(SOURCE, rank, ClaimPredicate::FileRole(FileRole::Extra)),
            ]);
        }
    }
}

/// For a file in an extras directory, the directory that the extras are of:
/// the parent of the outermost extras directory above it, e.g.
/// `Box Set/Movie (2001)` for `Box Set/Movie (2001)/Extras/Making Of.mkv`.
fn extras_root(path: &Path) -> Option<&Path> {
    let dir = path.parent()?;
    dir.ancestors()
        .filter(|ancestor| ancestor.file_name().is_some_and(is_extras_dir_name))
        .last()
        .and_then(Path::parent)
}

fn file_stem(path: &Path) -> Option<&str> {
    path.file_stem()?.to_str()
}

#[derive(Default)]
struct MovieDirGroup {
    candidates: Vec<MainCandidate>,
    extras: Vec<SubjectIndex>,
}

impl MovieDirGroup {
    /// Adds a video in the movie's directory: an extra if it's in an extras
    /// directory, or named like one, else a candidate for the main file.
    fn add(
        &mut self,
        series_patterns: &SeriesPatterns,
        part_res: &[Regex; 2],
        subject: SubjectIndex,
        path_id: &PathId,
        facts: &FileFacts,
        is_extra: bool,
    ) {
        // Only videos, until the catalog has a place for audio, e.g. a
        // soundtrack, and images, e.g. artwork.
        if !facts.has_video_streams() {
            return;
        }
        // Beside the main file, a video named like an extra is one too, e.g.
        // `Movie Title/Movie Title - Clean ED.mkv`.
        if is_extra || series_patterns.is_named_extra(&path_id.rel_path) {
            self.extras.push(subject);
        } else if !series_patterns.named_series_file(&path_id.rel_path) {
            // A directory of a series' episodes, e.g.
            // `Show Name S1/Show Name - S01E01.mkv`, isn't a movie's, and none of
            // them is its main file.
            let stem = file_stem(&path_id.rel_path).unwrap_or_default();
            self.candidates.push(MainCandidate {
                subject,
                duration: facts.media.as_ref().and_then(|m| m.duration()),
                is_part: part_res.iter().any(|re| re.is_match(stem)),
                score: score_movie_file_for_main_role(path_id, facts),
            });
        }
    }

    /// The directory's main files, if it holds one movie's worth of video:
    /// its features are the candidates at least half as long as the longest,
    /// or of unmeasured length, and they're one movie's if there's one, or
    /// they're all numbered as parts, e.g. `CD1` and `CD2`. A feature's main
    /// file is the best-scoring candidate; parts are all main files.
    fn one_movies_worth(&self) -> Option<Vec<&MainCandidate>> {
        let longest = self
            .candidates
            .iter()
            .filter_map(|c| c.duration)
            .max()
            .unwrap_or(Duration::ZERO);
        let features: Vec<&MainCandidate> = self
            .candidates
            .iter()
            .filter(|c| c.duration.is_none_or(|d| d * 2 >= longest))
            .collect();
        match features.as_slice() {
            [] => None,
            [_] => self
                .candidates
                .iter()
                .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
                .map(|main| vec![main]),
            parts if parts.iter().all(|part| part.is_part) => Some(features),
            _ => None,
        }
    }
}

/// A candidate's score for being a movie directory's main file: longer is
/// more likely the feature, and a name like the directory's is too.
fn score_movie_file_for_main_role(path_id: &PathId, facts: &FileFacts) -> f32 {
    let mut score = 0.0;
    if let Some(millis) = facts
        .media
        .as_ref()
        .and_then(|m| m.container.as_ref())
        .and_then(|c| c.duration_ms)
    {
        // Add score based on duration (1 point per 10 minutes, capped at 20)
        score += (millis as f32 / 600_000.0).min(20.0);
    }
    if let Some(dir_name) = path_id
        .rel_path
        .parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        && let Some(stem) = file_stem(&path_id.rel_path)
    {
        let dir_normalized = normalize_for_comparison(dir_name);
        let file_normalized = normalize_for_comparison(stem);
        if dir_normalized == file_normalized {
            score += 15.0;
        } else if file_normalized.starts_with(&dir_normalized)
            || dir_normalized.starts_with(&file_normalized)
        {
            score += 5.0;
        }
    }
    score
}

/// The directories of a series' episodes: those with videos named like
/// episodes, e.g. `Show - S01E01.mkv`, or numbered in a run as long as
/// episodes, e.g. `Show - 01.mkv`. And with them, those that may be: with a
/// run whose videos weren't measured, which its own claims make a series'.
fn episode_dirs<'a>(
    view: HeuristicView<'a>,
    patterns: &SeriesPatterns,
) -> (HashSet<&'a Path>, HashSet<&'a Path>) {
    let numbering_re = Regex::new(RELEASE_NUMBERING_REGEX).unwrap();
    let named: HashSet<&Path> = view
        .files
        .iter()
        .filter(|(path_id, facts)| {
            facts.has_video_streams() && patterns.is_episode(&path_id.rel_path)
        })
        .filter_map(|(path_id, _)| path_id.rel_path.parent())
        .collect();
    let runs = find_runs(view, patterns, &numbering_re);
    let mut episodes = named.clone();
    let mut maybe = named;
    for run in runs.iter().filter(|run| run.bonus_of.is_none()) {
        if run.is_episodes() {
            episodes.insert(run.dir);
        }
        if run.may_be_episodes() {
            maybe.insert(run.dir);
        }
    }
    (episodes, maybe)
}

struct MainCandidate {
    subject: SubjectIndex,
    duration: Option<Duration>,
    /// Whether its name numbers it as a part of its work, e.g. `CD1`.
    is_part: bool,
    score: f32,
}

/// Normalize a string for comparison by lowercasing and stripping year/parentheses.
fn normalize_for_comparison(s: &str) -> String {
    let re = Regex::new(r"[\(\[\.\-_]+(19|20)\d{2}.*$").unwrap();
    let s = re.replace(s, "");
    s.trim().to_lowercase()
}

/// Title and release year claims parsed from a directory's name: both of
/// its titles, if it names one also known as another, e.g. `the hairdressers
/// husband` and `le mari de la coiffeuse` for `The Hairdresser's Husband AKA
/// Le mari de la coiffeuse 1990`, as a file's name gives them.
pub(super) fn title_claims_from_dir(
    re: &Regex,
    dir: &Path,
    source: ClaimSource,
    rank: f32,
) -> Vec<RankedClaimPredicate> {
    let mut title_claims = Vec::new();
    if let Some(dir_name) = dir.file_name().and_then(|n| n.to_str())
        && let Some(ParsedMovieTitle { title, year }) = parse_movie_title(re, dir_name)
    {
        let titles = match parse_aka(&title) {
            Some((primary, secondary)) => vec![primary.to_string(), secondary.to_string()],
            None => vec![title],
        };
        title_claims.extend(titles.into_iter().map(|title| {
            RankedClaimPredicate::new(source, rank, ClaimPredicate::MovieTitle(title))
        }));
        if let Some(year) = year {
            title_claims.push(RankedClaimPredicate::new(
                source,
                rank,
                ClaimPredicate::MovieReleaseYear(year),
            ));
        }
    }
    title_claims
}
