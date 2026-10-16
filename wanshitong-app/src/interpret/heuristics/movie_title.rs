//! Heuristics for the titles, editions and roles of movies' files.

use super::{
    AddToSubject, AddWithRank, Done, HeuristicView,
    extras::in_extras_dir,
    names::{ParsedMovieTitle, file_stem, parse_aka, parse_movie_title},
    series_patterns::SeriesPatterns,
};
use crate::{edition, interpret::release_words, title};
use regex::Regex;
use wanshitong_core::domain::{
    ClaimPredicate, ClaimSource, FileFacts, FileRole, Heuristic, PathId, SubjectIndex,
};

/// Claims the main role for a video named like a movie release: a title and
/// then its year, e.g. `Heat.1995.1080p.BluRay.mkv` or `Heat (1995).mkv`.
/// Extras are rarely named that way, but those in an extras directory, and
/// episodes, are left alone.
pub(super) fn main_role_from_release_name(
    movie_title_re: &Regex,
    series_patterns: &SeriesPatterns,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if !facts.has_video_streams() || in_extras_dir(&path_id.rel_path) {
        return None;
    }
    if series_patterns.is_episode(&path_id.rel_path) {
        return None;
    }
    let name = path_id.rel_path.file_stem()?.to_str()?;
    parse_movie_title(movie_title_re, name)?.year?;
    adder.add_claim(ClaimPredicate::FileRole(FileRole::Main));
    None
}

pub(super) fn movie_title_from_container_tags(
    re: &Regex,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if !facts.has_video_streams() {
        return None;
    }

    let tags = facts.media.as_ref()?.container.as_ref()?.tags.as_ref()?;
    let raw_title = tags.get("TITLE").or(tags.get("title"))?;
    let ParsedMovieTitle { title, year } = parse_movie_title(re, raw_title)?;
    if let Some((primary, secondary)) = parse_aka(&title) {
        adder.add_claim(ClaimPredicate::MovieTitle(primary.into()));
        adder.add_claim(ClaimPredicate::MovieTitle(secondary.into()));
    } else {
        adder.add_claim(ClaimPredicate::MovieTitle(title));
    }
    if let Some(year) = year {
        adder.add_claim(ClaimPredicate::MovieReleaseYear(year));
    }
    None
}

/// Claims the title and year a video's file name gives it, e.g. `heat` and
/// 1995 for `Heat.1995.1080p.mkv`, or, for a name made only of tags, those
/// its tags give, e.g. `the glimmer` for `[Group][The Glimmer][1080p].mkv`.
/// Not an extra's or a series' file's tags, which name its series.
pub(super) fn movie_title_from_video_file_name(
    re: &Regex,
    series_patterns: &SeriesPatterns,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if !facts.has_video_streams() {
        return None;
    }

    let path = &path_id.rel_path;
    let name = path.file_stem()?.to_str()?;
    let ParsedMovieTitle { title, year } = parse_movie_title(re, name).or_else(|| {
        let series_file = series_patterns.named_series_file(path) || series_patterns.is_extra(path);
        (!series_file).then(|| series_patterns.tag_movie_title(name))?
    })?;
    if let Some((primary, secondary)) = parse_aka(&title) {
        adder.add_claim(ClaimPredicate::MovieTitle(primary.into()));
        adder.add_claim(ClaimPredicate::MovieTitle(secondary.into()));
    } else {
        adder.add_claim(ClaimPredicate::MovieTitle(title));
    }
    if let Some(year) = year {
        adder.add_claim(ClaimPredicate::MovieReleaseYear(year));
    }
    None
}

/// Claims the file name's title without what its release adds around it:
/// tags before it, e.g. `(アニメ)(映画)`, numbering in a collection, e.g.
/// `M-01` in `M-01 Perry Mason - Perry Mason Returns`, and everything from
/// the first word that's the release's, e.g. `USBD Remux` in
/// `Tokyo Godfathers USBD Remux`, or `tv-dx50`.
///
/// It's a second title, like [`movie_title_without_numbering`]'s, ranked
/// below the file name's: both are looked up, and the better match wins.
/// Only a feature's, as an episode, a special by its name, or an extra has
/// no movie title to clean. A title of one word left of it is returned
/// rather than claimed, for
/// [`title_of_one_word`](super::last_resort_titles::title_of_one_word), as
/// what's taken for noise is sometimes the title's, e.g. `Japanese Blu-ray
/// Promos`.
///
/// After the heuristics that number episodes and find specials, whose claims
/// it goes by.
pub(super) fn movie_title_without_release_noise(
    view: HeuristicView,
    mut adder: AddWithRank,
    movie_title_re: &Regex,
    series_patterns: &SeriesPatterns,
    noise: &ReleaseNoise,
) -> Vec<(SubjectIndex, String)> {
    let mut one_word = Vec::new();
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        let path = &path_id.rel_path;
        if !facts.has_video_streams()
            || series_patterns.named_series_file(path)
            || series_patterns.is_extra(path)
        {
            continue;
        }
        // A special by its own name, e.g. an OVA. Not one only near a series'
        // episodes, e.g. Perry Mason's TV movies, as that's what's left to it
        // if its titles find no movie.
        let of_series = adder.claims_of(subject).iter().any(|claim| {
            matches!(
                claim.pred,
                ClaimPredicate::EpisodeRange(_) | ClaimPredicate::AbsoluteEpisodeRange(_)
            ) || matches!(claim.pred, ClaimPredicate::SpecialOf(_))
                && claim.source == ClaimSource::Heuristic(Heuristic::SeriesSpecial)
        });
        let Some(stem) = file_stem(path).filter(|_| !of_series) else {
            continue;
        };
        let (Some(whole), Some(clean)) = (
            parse_movie_title(movie_title_re, stem),
            parse_movie_title(movie_title_re, &noise.remove(stem)),
        ) else {
            continue;
        };
        let clean = release_words::without_weak(&clean.title);
        if clean == whole.title || !title::has_words(&clean) {
            continue;
        }
        if title::words(&clean).len() == 1 {
            // Not a word a release adds, e.g. `japanese` of `Japanese Blu-ray
            // Promos`, which then names nothing.
            if !release_words::WEAK.contains(&clean.as_str()) {
                one_word.push((subject, clean));
            }
        } else if let Some(mut adder) = adder.with_subject(subject) {
            adder.add_claim(ClaimPredicate::MovieTitle(clean));
        }
    }
    one_word
}

/// What releases add to a file name around its title. See
/// [`movie_title_without_release_noise`].
pub(super) struct ReleaseNoise {
    leading_tags: Regex,
    numbering: Regex,
}

impl ReleaseNoise {
    pub(super) fn new() -> Self {
        Self {
            leading_tags: Regex::new(LEADING_TAGS_REGEX).unwrap(),
            numbering: Regex::new(COLLECTION_NUMBERING_REGEX).unwrap(),
        }
    }

    /// The name without its leading tags and numbering, and cut at the first
    /// word that's its release's, if a title is left before it: or at the
    /// bracket it's in, e.g. at `(` in `Show - 20180208 (THK 1280x720 x264)`,
    /// as what's with it there is the release's too.
    pub(super) fn remove(&self, name: &str) -> String {
        let name = self.leading_tags.replace(name, "");
        let name = self.numbering.replace(&name, "");
        self.cut(&name)
    }

    /// The name cut at the first word that's its release's. See
    /// [`release_words::cut`].
    pub(super) fn cut(&self, name: &str) -> String {
        release_words::cut(name)
    }
}

/// Claims the edition the file name says the file is, e.g. `Extended` in
/// `A.Knights.Tale.2001.Extended.2160p`, or `Director's Cut` for `DC` in
/// `Brazil (1985) DC Criterion`. See [`edition::in_file_name`].
///
/// Not an extra's, e.g. `Deleted Scenes - Extended`, which is no cut of its
/// work.
pub(super) fn edition_from_file_name(
    series_patterns: &SeriesPatterns,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    let path = &path_id.rel_path;
    if !facts.has_video_streams() || in_extras_dir(path) || series_patterns.is_extra(path) {
        return None;
    }
    let edition = edition::in_file_name(file_stem(path)?)?;
    adder.add_claim(ClaimPredicate::Edition(edition.label));
    None
}

/// Claims the file name's title without the edition named before its year,
/// e.g. `kingdom of heaven` for `Kingdom.of.Heaven.DC.Roadshow.Version.2005`,
/// whose title would otherwise take in the edition's words. A second title,
/// like [`movie_title_without_release_noise`]'s: both are looked up.
pub(super) fn movie_title_without_edition(
    movie_title_re: &Regex,
    series_patterns: &SeriesPatterns,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    let path = &path_id.rel_path;
    if !facts.has_video_streams()
        || series_patterns.named_series_file(path)
        || series_patterns.is_extra(path)
    {
        return None;
    }
    let stem = file_stem(path)?;
    let edition = edition::in_file_name(stem)?;
    let without = format!(
        "{} {}",
        &stem[..edition.span.start],
        &stem[edition.span.end..]
    );
    let whole = parse_movie_title(movie_title_re, stem)?;
    let clean = parse_movie_title(movie_title_re, &without)?;
    if clean.title != whole.title && title::has_words(&clean.title) {
        adder.add_claim(ClaimPredicate::MovieTitle(clean.title));
    }
    None
}

/// Claims the title of a video named like a series' movie or OVA, without its
/// number, e.g. `code geass - lelouch of the rebellion - initiation` for
/// `Code Geass Movie 01 - Lelouch of the Rebellion - Initiation`, which is how
/// knowledge bases title it.
///
/// It's a second title, ranked below the file name's, because the number is
/// sometimes part of the title, e.g. `Scary Movie 2`. Both are looked up, and
/// the better match wins.
pub(super) fn movie_title_without_numbering(
    movie_title_re: &Regex,
    numbering_re: &Regex,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if !facts.has_video_streams() {
        return None;
    }
    let stem = path_id.rel_path.file_stem()?.to_str()?;
    let numbering = numbering_re.find(stem)?;
    // e.g. `Movie 43`, which is only numbering if it follows a title.
    if !stem[..numbering.start()].chars().any(char::is_alphanumeric) {
        return None;
    }
    let without = numbering_re.replace_all(stem, "");
    let ParsedMovieTitle { title, .. } = parse_movie_title(movie_title_re, &without)?;
    adder.add_claim(ClaimPredicate::MovieTitle(title));
    None
}

/// Tags before a title, e.g. `(アニメ)(映画)` or `[Group]`, in ASCII or
/// full-width brackets.
const LEADING_TAGS_REGEX: &str = r"^(?:\s*[(\[（【][^)\]）】]*[)\]）】])+\s*";

/// A number before a title in a collection, e.g. `M-01 ` or `01 - `, but not
/// a number a title starts with, e.g. `12 Angry Men` or `2.Fast.2.Furious`.
const COLLECTION_NUMBERING_REGEX: &str = r"^(?:[A-Za-z]{1,2}-\d{1,3}\s+|\d{1,2}\s*[-.]\s+)";
