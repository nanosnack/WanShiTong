//! Heuristics: claims about files from their names, their measurements, and
//! the structure within a release, e.g. a season's or an extras directory,
//! a disc's folder, or numbered parts or episodes beside each other.
//!
//! Never from how releases are organized in a data source: a heuristic
//! doesn't go by a file's depth, or by the directory that groups its release,
//! as a data source may hold its releases flat, grouped or nested (see *Data
//! Source* in `spec/domain-model.md`). What the directories above a release
//! are named may be looked up, as a series' name, but only as one claim
//! among others.

mod disc;
mod external_ids;
mod extras;
mod kind;
mod last_resort_titles;
mod movie_directory;
mod movie_title;
mod names;
mod nfo;
mod parts;
pub(super) mod ranks;
mod runs;
mod series;
mod series_patterns;

use kind::{kind_of_duration, kind_of_siblings, kind_of_work};
use regex::Regex;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use wanshitong_core::domain::{
    ClaimPredicate, ClaimSource, FileFacts, FileRole, Heuristic, PathId, RankedClaimPredicate,
    SubjectIndex,
};

use disc::{disc_heuristic, makemkv_cmap_heuristic};
use external_ids::{NAME_ID_REGEX, external_ids_from_names};
use extras::{extra_role, extras_of_nearby_movie};
use last_resort_titles::{
    title_before_subtitle, title_of_one_word, title_split_at_capitals, title_with_short_vowels,
    title_without_and, title_without_group_tag,
};
use movie_directory::movie_directory_heuristic;
use movie_title::{
    ReleaseNoise, edition_from_file_name, main_role_from_release_name,
    movie_title_from_container_tags, movie_title_from_video_file_name, movie_title_without_edition,
    movie_title_without_numbering, movie_title_without_release_noise,
};
use names::{MOVIE_TITLE_REGEX, RELEASE_NUMBERING_REGEX};
use nfo::{NfoPatterns, nfo_heuristic};
use parts::{PART_REGEX, movie_title_before_part, part_from_file_name, parts_in_tags};
use runs::{lone_numbered_episodes, numbered_runs};
use series::{
    OVA_REGEX, absolute_episodes_in_parentheses, episode_from_file_name,
    episode_title_from_file_name, extras_of_season_releases, season_episode_number,
    season_from_directory, series_also_known_as, series_name_before_season_episode_number,
    series_name_from_file_name, series_name_without_release_words, series_of_nearby_episodes,
    special_of_nearby_series, special_of_series, sx_ex_from_file_name,
};
use series_patterns::SeriesPatterns;

pub fn run_heuristics(
    files: &[(PathId, FileFacts)],
) -> HashMap<SubjectIndex, Vec<RankedClaimPredicate>> {
    let BroadPhaseResult {
        claims: broad_claims,
        exclusions,
    } = run_broad_phase_heuristics(files);
    let narrow_claims = run_narrow_phase_heuristics(files, &exclusions);

    // Merge claims from both phases
    let mut claims = broad_claims;
    for (subject, subject_claims) in narrow_claims {
        claims.entry(subject).or_default().extend(subject_claims);
    }
    claims
}

/// What a release writes into a file's name that no title has: an episode's
/// marker as releases write it, e.g. `S02E01` or `E92`, and the release's words,
/// e.g. `1080p BluRay`. A title with either in it is the release's name, not a
/// work's. Not `Episode 1`, which is a title's words as often, e.g. Star Wars'.
pub(super) struct ReleaseMarkers {
    series: SeriesPatterns,
}

impl ReleaseMarkers {
    pub(super) fn new() -> Self {
        Self {
            series: SeriesPatterns::new(),
        }
    }

    /// Whether `title` has an episode's marker or a release's words in it,
    /// e.g. `babylon berlin s02e01 bluray 1080i`. A bare number isn't a
    /// marker, as it's often a title's, e.g. `Mob Psycho 100`.
    pub(super) fn in_title(&self, title: &str) -> bool {
        self.series.has_marker_token(title)
            || crate::interpret::release_words::RELEASE_WORD.is_match(title)
    }
}

struct BroadPhaseResult {
    claims: HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
    exclusions: HashSet<SubjectIndex>,
}

fn run_broad_phase_heuristics(files: &[(PathId, FileFacts)]) -> BroadPhaseResult {
    let view = HeuristicView { files };

    let mut claims = HashMap::<SubjectIndex, Vec<RankedClaimPredicate>>::new();
    let mut exclusions = HashSet::<SubjectIndex>::new();

    disc_heuristic(view, &mut claims, ranks::DISC);
    makemkv_cmap_heuristic(view, &mut exclusions);
    movie_directory_heuristic(
        view,
        &mut claims,
        &exclusions,
        ranks::MOVIE_DIRECTORY,
        ranks::EXTRAS_DIRECTORY,
    );

    BroadPhaseResult { claims, exclusions }
}

/// Runs the heuristics, ranked by [`ranks`]. In an order of its own, as some
/// go by the claims before them.
fn run_narrow_phase_heuristics(
    files: &[(PathId, FileFacts)],
    exclusions: &HashSet<SubjectIndex>,
) -> HashMap<SubjectIndex, Vec<RankedClaimPredicate>> {
    let view = HeuristicView { files };

    let mut claims = HashMap::<SubjectIndex, Vec<RankedClaimPredicate>>::new();
    let mut adder = AddToClaims {
        claims: &mut claims,
        exclusions,
    };

    let movie_title_re = Regex::new(MOVIE_TITLE_REGEX).unwrap();
    let sample_re = Regex::new(SAMPLE_REGEX).unwrap();
    let numbering_re = Regex::new(RELEASE_NUMBERING_REGEX).unwrap();
    let series_patterns = SeriesPatterns::new();
    file_heuristic(
        view,
        adder.with(Heuristic::Sample, ranks::SAMPLE),
        |p, f, a| sample_role(&sample_re, p, f, a),
    );
    file_heuristic(
        view,
        adder.with(Heuristic::ContainerDuration, ranks::CONTAINER_DURATION),
        |_p, f, a| duration_from_container(f, a),
    );
    extras_of_season_releases(
        view,
        adder.with(Heuristic::SeasonRelease, ranks::SEASON_RELEASE),
        &series_patterns,
    );
    series_also_known_as(
        view,
        adder.with(Heuristic::SeriesAlsoKnownAs, ranks::SERIES_ALSO_KNOWN_AS),
        &series_patterns,
    );
    file_heuristic(
        view,
        adder.with(Heuristic::FileNameSeriesName, ranks::FILE_NAME_SERIES_NAME),
        |p, f, a| series_name_from_file_name(&series_patterns, p, f, a),
    );
    file_heuristic(
        view,
        adder.with(
            Heuristic::SeriesNameBeforeSeasonEpisodeNumber,
            ranks::SERIES_NAME_BEFORE_SEASON_EPISODE_NUMBER,
        ),
        |p, f, a| series_name_before_season_episode_number(&series_patterns, p, f, a),
    );
    series_of_nearby_episodes(
        view,
        adder.with(
            Heuristic::SeriesOfNearbyEpisodes,
            ranks::SERIES_OF_NEARBY_EPISODES,
        ),
        &series_patterns,
        &numbering_re,
    );
    file_heuristic(
        view,
        adder.with(
            Heuristic::FileNameSeasonEpisode,
            ranks::FILE_NAME_SEASON_EPISODE,
        ),
        |p, _f, a| sx_ex_from_file_name(&series_patterns, p, a),
    );
    file_heuristic(
        view,
        adder.with(Heuristic::SeasonEpisodeNumber, ranks::SEASON_EPISODE_NUMBER),
        |p, f, a| season_episode_number(&series_patterns, p, f, a),
    );
    absolute_episodes_in_parentheses(
        view,
        adder.with(
            Heuristic::EpisodeInParentheses,
            ranks::EPISODE_IN_PARENTHESES,
        ),
        &series_patterns,
    );
    file_heuristic(
        view,
        adder.with(Heuristic::FileNameEpisode, ranks::FILE_NAME_EPISODE),
        |p, f, a| episode_from_file_name(&series_patterns, p, f, a),
    );
    file_heuristic(
        view,
        adder.with(Heuristic::SeasonDirectory, ranks::SEASON_DIRECTORY),
        |p, f, a| season_from_directory(&series_patterns, p, f, a),
    );
    let ova_re = Regex::new(OVA_REGEX).unwrap();
    let part_re = Regex::new(PART_REGEX).unwrap();
    file_heuristic(
        view,
        adder.with(Heuristic::SeriesSpecial, ranks::SERIES_SPECIAL),
        |p, f, a| special_of_series(&series_patterns, &ova_re, p, f, a),
    );
    numbered_runs(
        view,
        &mut adder,
        [ranks::EPISODE_RUN, ranks::FEATURE_RUN],
        &series_patterns,
        &numbering_re,
    );
    lone_numbered_episodes(
        view,
        &mut adder,
        ranks::LONE_NUMBERED_EPISODE,
        &series_patterns,
        &numbering_re,
    );
    file_heuristic(
        view,
        adder.with(Heuristic::FileNamePart, ranks::FILE_NAME_PART),
        |p, f, a| part_from_file_name(&part_re, p, f, a),
    );
    parts_in_tags(
        view,
        adder.with(Heuristic::PartInTag, ranks::PART_IN_TAG),
        &series_patterns,
    );
    // After the parts, which it leaves alone.
    special_of_nearby_series(
        view,
        adder.with(
            Heuristic::SpecialOfNearbySeries,
            ranks::SPECIAL_OF_NEARBY_SERIES,
        ),
        &series_patterns,
        &numbering_re,
    );
    // After the extras' series, and the specials, which it leaves alone.
    extras_of_nearby_movie(
        view,
        adder.with(
            Heuristic::ExtrasOfNearbyMovie,
            ranks::EXTRAS_OF_NEARBY_MOVIE,
        ),
        &series_patterns,
        &movie_title_re,
    );
    file_heuristic(
        view,
        adder.with(Heuristic::ReleaseName, ranks::RELEASE_NAME),
        |p, f, a| main_role_from_release_name(&movie_title_re, &series_patterns, p, f, a),
    );
    file_heuristic(
        view,
        adder.with(
            Heuristic::ExtrasNameOrDirectory,
            ranks::EXTRAS_NAME_OR_DIRECTORY,
        ),
        |p, f, a| extra_role(&series_patterns, p, f, a),
    );
    file_heuristic(
        view,
        adder.with(Heuristic::ContainerTitleTag, ranks::CONTAINER_TITLE_TAG),
        |_p, f, a| movie_title_from_container_tags(&movie_title_re, f, a),
    );
    file_heuristic(
        view,
        adder.with(Heuristic::FileNameTitle, ranks::FILE_NAME_TITLE),
        |p, f, a| movie_title_from_video_file_name(&movie_title_re, &series_patterns, p, f, a),
    );
    file_heuristic(
        view,
        adder.with(
            Heuristic::TitleWithoutNumbering,
            ranks::TITLE_WITHOUT_NUMBERING,
        ),
        |p, f, a| movie_title_without_numbering(&movie_title_re, &numbering_re, p, f, a),
    );
    file_heuristic(
        view,
        adder.with(Heuristic::TitleBeforePart, ranks::TITLE_BEFORE_PART),
        |p, f, a| movie_title_before_part(&movie_title_re, &part_re, p, f, a),
    );
    // After the numbered runs, whose episodes it goes by.
    external_ids_from_names(
        view,
        adder.with(Heuristic::NameExternalId, ranks::NAME_EXTERNAL_ID),
        &series_patterns,
        &Regex::new(NAME_ID_REGEX).unwrap(),
    );
    // After the numbered runs and the samples too.
    nfo_heuristic(
        view,
        &mut adder,
        [ranks::NFO_EXTERNAL_ID, ranks::NFO_EPISODE, ranks::NFO_WORK],
        &series_patterns,
        &NfoPatterns::new(),
    );
    file_heuristic(
        view,
        adder.with(
            Heuristic::FileNameEpisodeTitle,
            ranks::FILE_NAME_EPISODE_TITLE,
        ),
        |p, f, a| episode_title_from_file_name(&series_patterns, p, f, a),
    );
    file_heuristic(
        view,
        adder.with(Heuristic::FileNameEdition, ranks::FILE_NAME_EDITION),
        |p, f, a| edition_from_file_name(&series_patterns, p, f, a),
    );
    file_heuristic(
        view,
        adder.with(Heuristic::TitleWithoutEdition, ranks::TITLE_WITHOUT_EDITION),
        |p, f, a| movie_title_without_edition(&movie_title_re, &series_patterns, p, f, a),
    );
    let noise = ReleaseNoise::new();
    // Last but one, as it goes by the claims before.
    let one_word = movie_title_without_release_noise(
        view,
        adder.with(
            Heuristic::TitleWithoutReleaseNoise,
            ranks::TITLE_WITHOUT_RELEASE_NOISE,
        ),
        &movie_title_re,
        &series_patterns,
        &noise,
    );
    title_of_one_word(
        adder.with(Heuristic::TitleOfOneWord, ranks::TITLE_OF_ONE_WORD),
        one_word,
    );
    series_name_without_release_words(
        view,
        adder.with(
            Heuristic::SeriesNameWithoutReleaseWords,
            ranks::SERIES_NAME_WITHOUT_RELEASE_WORDS,
        ),
        &noise,
    );
    kind_of_work(view, adder.with(Heuristic::KindOfWork, ranks::KIND_OF_WORK));
    kind_of_duration(
        view,
        adder.with(Heuristic::KindOfDuration, ranks::KIND_OF_DURATION),
    );
    kind_of_siblings(
        view,
        adder.with(Heuristic::KindOfSiblings, ranks::KIND_OF_SIBLINGS),
    );
    title_before_subtitle(
        view,
        adder.with(Heuristic::TitleBeforeSubtitle, ranks::TITLE_BEFORE_SUBTITLE),
    );
    title_without_group_tag(
        view,
        adder.with(
            Heuristic::TitleWithoutGroupTag,
            ranks::TITLE_WITHOUT_GROUP_TAG,
        ),
    );
    title_split_at_capitals(
        view,
        adder.with(
            Heuristic::TitleSplitAtCapitals,
            ranks::TITLE_SPLIT_AT_CAPITALS,
        ),
    );
    title_with_short_vowels(
        view,
        adder.with(
            Heuristic::TitleWithShortVowels,
            ranks::TITLE_WITH_SHORT_VOWELS,
        ),
    );
    title_without_and(
        view,
        adder.with(Heuristic::TitleWithoutAnd, ranks::TITLE_WITHOUT_AND),
    );
    claims
}

/// What a heuristic for one file returns: `None` once it has nothing more to
/// go on, whether or not it made a claim, so that `?` ends it early. Nothing
/// reads it.
pub(super) type Done = Option<()>;

/// Runs `h` on every file that is not excluded.
fn file_heuristic(
    view: HeuristicView,
    mut adder: AddWithRank,
    h: impl Fn(&PathId, &FileFacts, AddToSubject) -> Done,
) {
    for (subject, (path, facts)) in view.files.iter().enumerate() {
        if let Some(subject_adder) = adder.with_subject(subject) {
            let _ = h(path, facts, subject_adder);
        }
    }
}

struct AddToClaims<'a> {
    claims: &'a mut HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
    exclusions: &'a HashSet<SubjectIndex>,
}

impl<'a> AddToClaims<'a> {
    fn with(&mut self, heuristic: Heuristic, rank: f32) -> AddWithRank<'_> {
        AddWithRank {
            claims: self.claims,
            exclusions: self.exclusions,
            heuristic,
            rank,
        }
    }
}

struct AddWithRank<'a> {
    claims: &'a mut HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
    exclusions: &'a HashSet<SubjectIndex>,
    heuristic: Heuristic,
    rank: f32,
}

impl<'a> AddWithRank<'a> {
    /// The claims made about a subject so far, by the heuristics before.
    fn claims_of(&self, subject: SubjectIndex) -> &[RankedClaimPredicate] {
        self.claims.get(&subject).map_or(&[], Vec::as_slice)
    }

    fn with_subject(&mut self, subject: SubjectIndex) -> Option<AddToSubject<'_>> {
        if self.exclusions.contains(&subject) {
            return None;
        }
        Some(AddToSubject {
            claims: self.claims,
            heuristic: self.heuristic,
            rank: self.rank,
            subject,
        })
    }
}

struct AddToSubject<'a> {
    claims: &'a mut HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
    heuristic: Heuristic,
    rank: f32,
    subject: SubjectIndex,
}

impl<'a> AddToSubject<'a> {
    fn add_claim(&mut self, pred: ClaimPredicate) {
        self.claims
            .entry(self.subject)
            .or_default()
            .push(RankedClaimPredicate::new(
                ClaimSource::Heuristic(self.heuristic),
                self.rank,
                pred,
            ));
    }
}

#[derive(Clone, Copy)]
struct HeuristicView<'a> {
    files: &'a [(PathId, FileFacts)],
}

fn duration_from_container(facts: &FileFacts, mut adder: AddToSubject) -> Done {
    // Not for text, which ffprobe reads as ANSI art and gives a duration.
    if !facts.has_video_streams() {
        return None;
    }
    let millis = facts.media.as_ref()?.container.as_ref()?.duration_ms?;
    let duration = Duration::from_millis(millis as u64);
    adder.add_claim(ClaimPredicate::Duration(duration));
    None
}

/// Detects release samples: short clips named like `Sample.mkv` or
/// `movie-sample.mkv`, or placed in a `Sample/` directory.
///
/// A short duration is required, because a real movie can have "sample" in its
/// name (e.g. `The.Sample.mkv`). With nothing to compete against, even a
/// low-ranked Sample claim would exclude such a movie from the catalog.
fn sample_role(re: &Regex, path_id: &PathId, facts: &FileFacts, mut adder: AddToSubject) -> Done {
    if !facts.has_video_streams() {
        return None;
    }
    let duration = facts.media.as_ref()?.duration()?;
    if duration > MAX_SAMPLE_DURATION {
        return None;
    }
    let path = &path_id.rel_path;
    let named_sample = re.is_match(path.file_stem()?.to_str()?);
    let in_sample_dir = path
        .parent()?
        .components()
        .any(|c| c.as_os_str().eq_ignore_ascii_case("sample"));
    if named_sample || in_sample_dir {
        adder.add_claim(ClaimPredicate::FileRole(FileRole::Sample));
    }
    None
}

// TODO: try substrings of the file or directory name (when naming convention is poor)
// TODO: check jellyfin and plex naming rules
// TODO: get external IDs from container tags
// TODO: get season number from directory name

/// A file stem that is, or ends with, the word "sample".
const SAMPLE_REGEX: &str = r"(?i)(?:^|[\s._-])sample$";

/// Samples are usually a minute or two. This leaves plenty of margin while
/// staying far below the length of a feature.
const MAX_SAMPLE_DURATION: Duration = Duration::from_secs(10 * 60);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_marker_is_an_episodes_or_a_releases_word_but_not_a_bare_number() {
        let markers = ReleaseMarkers::new();
        assert!(markers.in_title("babylon berlin s02e01"));
        assert!(
            markers.in_title("babylon berlin s02e01 bluray 1080i dts-hd ma 5 1 avc remux-quuxter")
        );
        assert!(markers.in_title("show name e92"));
        assert!(markers.in_title("curb your enthusiasm season 1 episode 01 - the pants tent"));
        // A title's words as often as a marker.
        assert!(!markers.in_title("show name episode 3"));
        assert!(!markers.in_title("star wars - episode 1 - the phantom menace"));
        assert!(markers.in_title("show name 1080p"));
        assert!(!markers.in_title("mob psycho 100"));
        assert!(!markers.in_title("wano 01"));
        assert!(!markers.in_title("dr slump & arale-chan 024 5 special 1"));
        assert!(!markers.in_title("babylon berlin"));
    }
}
