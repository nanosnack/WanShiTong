//! Heuristics for series' episodes, seasons and specials.

use super::{
    AddToSubject, AddWithRank, Done, HeuristicView,
    movie_title::ReleaseNoise,
    names::{file_stem, parse_aka},
    runs::find_runs,
    series_patterns::{SeriesName, SeriesPatterns},
};
use crate::{interpret::release_words, title};
use regex::Regex;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::Path,
};
use wanshitong_core::domain::{
    ClaimPredicate, ClaimSource, FileFacts, FileRole, PathId, SubjectIndex,
};

/// Claims the season and episodes a file name says the file is, e.g.
/// `S01E02` or `S03E18E19E20E21`.
pub(super) fn sx_ex_from_file_name(
    patterns: &SeriesPatterns,
    path_id: &PathId,
    mut adder: AddToSubject,
) -> Done {
    let marker = patterns.episode_marker(file_stem(&path_id.rel_path)?)?;
    adder.add_claim(ClaimPredicate::Season(marker.season?));
    adder.add_claim(ClaimPredicate::EpisodeRange(marker.episodes));
    None
}

/// Claims the season and episode of a video named with a number that's the
/// season its directory names and an episode, e.g. season 7, episode 30 for
/// `Season 7/Perry.Mason.730.The.Ugly.Duckling`. Also when the name has
/// another marker, e.g. `E138` in `Perry.Mason.515.The.Case.of.the.Roving.River.(E138)`,
/// and ranks decide between them.
pub(super) fn season_episode_number(
    patterns: &SeriesPatterns,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if !facts.has_video_streams() {
        return None;
    }
    let marker = patterns.season_episode_number(&path_id.rel_path)?;
    adder.add_claim(ClaimPredicate::Season(marker.season?));
    adder.add_claim(ClaimPredicate::EpisodeRange(marker.episodes));
    None
}

/// Claims the episodes a video's name numbers without a season, e.g. `E92`,
/// `E93-E95` or `Episode 01`. In a season's directory, e.g. `Season 2`,
/// they're numbered within that season. Otherwise they're numbered through
/// the whole series, e.g. One Piece's `E92`.
pub(super) fn episode_from_file_name(
    patterns: &SeriesPatterns,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if !facts.has_video_streams() {
        return None;
    }
    let path = &path_id.rel_path;
    let marker = patterns.episode_marker(file_stem(path)?)?;
    if marker.season.is_some() {
        return None;
    }
    if patterns.season_of_dirs(path).is_some() {
        adder.add_claim(ClaimPredicate::EpisodeRange(marker.episodes));
    } else {
        adder.add_claim(ClaimPredicate::AbsoluteEpisodeRange(marker.episodes));
    }
    None
}

/// Claims the number through the whole series that a video's name gives in
/// parentheses after its season's episode, as anime releases do, e.g. 130
/// for `Show - S02E72 (130) - Title`. A number in parentheses means many
/// things, e.g. a year or a download's `(2)`, so only where the files beside
/// it agree: at least two videos in the directory whose numbers exceed their
/// episodes' by the same amount, as `S02E72 (130)` and `S02E73 (131)` do by
/// fifty-eight. A knowledge base goes by it, to number the file as it does
/// when the release splits its seasons otherwise.
pub(super) fn absolute_episodes_in_parentheses(
    view: HeuristicView,
    mut adder: AddWithRank,
    patterns: &SeriesPatterns,
) {
    // Each video's number, by its directory and how far past its episode
    // the number is.
    let mut by_offset = HashMap::<(&Path, u32), Vec<(SubjectIndex, u32)>>::new();
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        let path = &path_id.rel_path;
        if !facts.has_video_streams() {
            continue;
        }
        let (Some(dir), Some(stem)) = (path.parent(), file_stem(path)) else {
            continue;
        };
        if let Some((_, episode, number)) = patterns.episode_in_parentheses(stem) {
            by_offset
                .entry((dir, number - episode))
                .or_default()
                .push((subject, number));
        }
    }
    for numbered in by_offset
        .into_values()
        .filter(|numbered| numbered.len() >= 2)
    {
        for (subject, number) in numbered {
            if let Some(mut adder) = adder.with_subject(subject) {
                adder.add_claim(ClaimPredicate::AbsoluteEpisodeRange([number, number]));
            }
        }
    }
}

/// Claims the season named by the directory of a video named like an
/// episode, e.g. 2 for `Season 2/Show Name E05.mkv`. A season in the file
/// name outranks it.
pub(super) fn season_from_directory(
    patterns: &SeriesPatterns,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    let path = &path_id.rel_path;
    if !facts.has_video_streams() || !patterns.is_episode(path) {
        return None;
    }
    adder.add_claim(ClaimPredicate::Season(patterns.season_of_dirs(path)?));
    None
}

/// Claims the series named in a video's file name before what the file is of
/// it: an episode, e.g. `Show Name` in `Show Name - S01E02` or
/// `Show.Name.2019.S01E02`, or a creditless opening or ending, e.g. `Show Name`
/// in `Show Name - S01 NCOP 01`. The name is without a year or season.
///
/// An episode's file without a name there, e.g. `S01E02 - Pilot.mkv`, is named
/// after the directories above it (see [`SeriesPatterns::series_name_from_dirs`]).
/// An extra's is left to [`series_of_nearby_episodes`].
pub(super) fn series_name_from_file_name(
    patterns: &SeriesPatterns,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if !facts.has_video_streams() {
        return None;
    }
    let SeriesName { name, year } = patterns.file_name_series(&path_id.rel_path)?;
    adder.add_claim(ClaimPredicate::TvSeriesName(name));
    if let Some(year) = year {
        adder.add_claim(ClaimPredicate::TvSeriesYear(year));
    }
    None
}

/// Claims the series named in a video's file name before a number that's the
/// season its directory names and an episode, when the name's other marker
/// gives another, e.g. `Perry Mason` and not `Perry Mason 515 The Case of the
/// Roving River (` for `Season 5/Perry.Mason.515.The.Case.of.the.Roving.River.(E138)`.
pub(super) fn series_name_before_season_episode_number(
    patterns: &SeriesPatterns,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if !facts.has_video_streams() {
        return None;
    }
    let path = &path_id.rel_path;
    let marker = patterns.season_episode_number(path)?;
    let series = patterns.series_name(&file_stem(path)?[..marker.start])?;
    let SeriesName { name, year } = patterns.with_year_from_dirs(series, path.parent()?);
    if patterns
        .file_name_series(path)
        .is_some_and(|other| other.name == name)
    {
        return None;
    }
    adder.add_claim(ClaimPredicate::TvSeriesName(name));
    if let Some(year) = year {
        adder.add_claim(ClaimPredicate::TvSeriesYear(year));
    }
    None
}

/// Claims the title after an episode's marker in its file name, e.g. `Alibi
/// Me` in `Alfred Hitchcock Presents - S02E07 - Alibi Me`, or `Strong x Or x
/// Weak` in `Hunter x Hunter (2011) - S02E61 (119) - Strong x Or x Weak`:
/// without tags, e.g. `(119)` or `[1080p]`, and cut at the first word that's
/// its release's, e.g. `1080p` in `Show.S01E02.Title.1080p.BluRay`.
///
/// Not a file of several episodes, e.g. `S02E12E13`, whose name joins their
/// titles, nor a title that only numbers the episode again, e.g. `Episode 9`,
/// which a knowledge base would know some other episode by.
pub(super) fn episode_title_from_file_name(
    patterns: &SeriesPatterns,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    let path = &path_id.rel_path;
    if !facts.has_video_streams() || patterns.is_extra(path) {
        return None;
    }
    let marker = patterns.any_episode_marker(path)?;
    let [first, last] = marker.episodes;
    if first != last {
        return None;
    }
    let after = &file_stem(path)?[marker.end..];
    let untagged = Regex::new(r"\[[^\]]*\]?|\([^)]*\)?")
        .unwrap()
        .replace_all(after, " ");
    let title = match release_words::RELEASE_WORD.find(&untagged) {
        Some(word) => &untagged[..word.start()],
        None => &untagged[..],
    };
    let title = title
        .replace(['.', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let title =
        title.trim_matches(|c: char| !c.is_alphanumeric() && !matches!(c, '\'' | '’' | '!' | '?'));
    let generic = Regex::new(r"(?i)^(?:episode|ep|chapter|part|vol(?:ume)?)?[\s.]*\d+$").unwrap();
    if !title.chars().any(char::is_alphabetic) || generic.is_match(title) {
        return None;
    }
    adder.add_claim(ClaimPredicate::EpisodeTitle(title.to_owned()));
    None
}

/// Claims that an extra without a series in its own name, e.g.
/// `Show Name - Stage 0.25.mkv`, is of the series of the episodes near it (see
/// [`series_near`]), e.g. `Show Name S1/Show Name - S01E01.mkv` near
/// `Show Name Extras/Show Name - Stage 0.25.mkv`, or Monster's
/// `Monster - Chapter 01` near `Creditless OP&ED/Monster - Opening (Creditless)`.
///
/// Only extras, as a series' movies are near its episodes too. Other videos
/// near them are left to [`special_of_nearby_series`].
pub(super) fn series_of_nearby_episodes(
    view: HeuristicView,
    mut adder: AddWithRank,
    patterns: &SeriesPatterns,
    numbering_re: &Regex,
) {
    let series_under = series_under_dirs(view, patterns, numbering_re);
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        let path = &path_id.rel_path;
        if !facts.has_video_streams()
            || !patterns.is_extra(path)
            || patterns.file_name_series(path).is_some()
        {
            continue;
        }
        if let Some(SeriesName { name, year }) = series_near(&series_under, path)
            && let Some(mut adder) = adder.with_subject(subject)
        {
            adder.add_claim(ClaimPredicate::TvSeriesName(name));
            if let Some(year) = year {
                adder.add_claim(ClaimPredicate::TvSeriesYear(year));
            }
        }
    }
}

/// Claims that a video near a series' episodes (see [`series_near`]) that's
/// neither an episode nor an extra is a special of that series, e.g.
/// `Taskmaster/New Year Treat/Taskmasters.New.Year.Treat.2022.mkv` near
/// `Taskmaster/Series 1/`, or Chibi Maruko-chan's `Special/Special (1991) -
/// Maruko's Wasted Summer.mp4` near its episodes. A knowledge base looks it up
/// as one only if its titles find no movie, so a series' movies near its
/// episodes stay movies. It then finds which special it is by its titles, or
/// leaves it one of the series' extras.
///
/// After the heuristics that number episodes and parts, whose claims it goes
/// by.
pub(super) fn special_of_nearby_series(
    view: HeuristicView,
    mut adder: AddWithRank,
    patterns: &SeriesPatterns,
    numbering_re: &Regex,
) {
    let series_under = series_under_dirs(view, patterns, numbering_re);
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        let path = &path_id.rel_path;
        if !facts.has_video_streams() || patterns.is_extra(path) {
            continue;
        }
        // Already an episode, a special, a sample, or of a series it names.
        // Or a part of its work, which its knowledge base numbers, e.g.
        // `Champion.Of.Champions.Part.1` as the series' first episode.
        let claimed = adder.claims_of(subject).iter().any(|claim| {
            matches!(
                claim.pred,
                ClaimPredicate::EpisodeRange(_)
                    | ClaimPredicate::AbsoluteEpisodeRange(_)
                    | ClaimPredicate::SpecialOf(_)
                    | ClaimPredicate::Part(_)
                    | ClaimPredicate::TvSeriesName(_)
                    | ClaimPredicate::FileRole(FileRole::Sample)
            )
        });
        if claimed {
            continue;
        }
        if let Some(SeriesName { name, .. }) = series_near(&series_under, path)
            && let Some(mut adder) = adder.with_subject(subject)
        {
            adder.add_claim(ClaimPredicate::SpecialOf(name));
        }
    }
}

/// The names of the series of the episodes under each directory, and the
/// years they say those first aired. Episodes are those named so, and those
/// in runs no longer than episodes.
fn series_under_dirs<'a>(
    view: HeuristicView<'a>,
    patterns: &SeriesPatterns,
    numbering_re: &Regex,
) -> HashMap<&'a Path, (BTreeSet<String>, BTreeSet<u32>)> {
    let mut series_under = HashMap::<&Path, (BTreeSet<String>, BTreeSet<u32>)>::new();
    for (path_id, facts) in view.files {
        let path = &path_id.rel_path;
        if !facts.has_video_streams() || !patterns.is_episode(path) {
            continue;
        }
        let Some(SeriesName { name, year }) = patterns.file_name_series(path) else {
            continue;
        };
        for dir in path.ancestors().skip(1) {
            let (names, years) = series_under.entry(dir).or_default();
            names.insert(name.clone());
            years.extend(year);
        }
    }
    // Runs that may be episodes: a run unmeasured is a series' as its own
    // claims have it, so its extras are too.
    let runs = find_runs(view, patterns, numbering_re);
    for run in runs
        .iter()
        .filter(|run| run.may_be_episodes() && run.bonus_of.is_none())
    {
        let (Some(SeriesName { name, year }), _) = patterns.run_series(&run.before, run.dir) else {
            continue;
        };
        for dir in run.dir.ancestors() {
            let (names, years) = series_under.entry(dir).or_default();
            names.insert(name.clone());
            years.extend(year);
        }
    }
    series_under
}

/// The series of the episodes near a file: those under the nearest directory
/// above it that has any, if they're all of one series, with the year they
/// first aired, if they agree on it. If they're of several, the one the
/// directory is named for, e.g. `taskmaster` for
/// `Taskmaster.S01-S16+Specials`, which has episodes of a spin-off too.
fn series_near(
    series_under: &HashMap<&Path, (BTreeSet<String>, BTreeSet<u32>)>,
    path: &Path,
) -> Option<SeriesName> {
    let (dir, (names, years)) = path
        .ancestors()
        .skip(1)
        .find_map(|dir| Some((dir, series_under.get(dir)?)))?;
    if let [name] = Vec::from_iter(names).as_slice() {
        return Some(SeriesName {
            name: (*name).clone(),
            year: (years.len() == 1).then(|| years.first().copied()).flatten(),
        });
    }
    let dir_name = dir.file_name()?.to_str()?;
    let own: Vec<_> = names
        .iter()
        .filter(|name| title::begins_with(dir_name, name))
        .collect();
    match own.as_slice() {
        [name] => Some(SeriesName {
            name: (*name).clone(),
            year: None,
        }),
        _ => None,
    }
}

/// Claims that a video in a season release of a series whose own name gives
/// another series is an extra of the release's series, e.g.
/// `On Location - E01 - Antarctica.mkv` in
/// `Seven.Worlds.One.Planet.S01.REPACK.2160p…/On Location/`.
///
/// Only in a release that holds episodes whose names give its series too, e.g.
/// `Seven.Worlds.One.Planet.S01E01…`. They agree that it's that series'
/// release. Where all of its episodes give another name, that's the release's
/// series under another name, e.g. `Hajime.no.Ippo.The.Fighting.S01E01` in
/// `Hajime.no.Ippo.S01.The.Fighting…`.
pub(super) fn extras_of_season_releases(
    view: HeuristicView,
    mut adder: AddWithRank,
    patterns: &SeriesPatterns,
) {
    // A file's season release: the nearest directory named as a series and a
    // season, e.g. not `Season 1`, and that series.
    fn release_of<'a>(patterns: &SeriesPatterns, path: &'a Path) -> Option<(&'a Path, String)> {
        path.ancestors().skip(1).find_map(|dir| {
            let name = dir.file_name()?.to_str()?;
            if patterns.season_dir.is_match(name) {
                return None;
            }
            Some((dir, patterns.season_release_series(name)?))
        })
    }
    // The releases that hold episodes named after their series, and the videos
    // named after other series in releases.
    let mut named_releases = HashSet::<&Path>::new();
    let mut others = Vec::new();
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        let path = &path_id.rel_path;
        if !facts.has_video_streams() {
            continue;
        }
        let (Some((release, series)), Some(own)) =
            (release_of(patterns, path), patterns.file_name_series(path))
        else {
            continue;
        };
        if own.name == series {
            named_releases.insert(release);
        } else {
            others.push((subject, release, series));
        }
    }
    for (subject, release, series) in others {
        if named_releases.contains(release)
            && let Some(mut adder) = adder.with_subject(subject)
        {
            adder.add_claim(ClaimPredicate::TvSeriesName(series));
            adder.add_claim(ClaimPredicate::FileRole(FileRole::Extra));
        }
    }
}

/// Claims the series a video's name marks it as an OVA of, e.g. `food wars!`
/// for `Food Wars! S01 - OVA1`: the series named before the OVA, without a
/// season, or by the directories above. A knowledge base says whether it's a
/// movie, as TMDB lists Code Geass' OVAs, or a special of that series.
pub(super) fn special_of_series(
    patterns: &SeriesPatterns,
    ova_re: &Regex,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if !facts.has_video_streams() {
        return None;
    }
    let path = &path_id.rel_path;
    let (dir, stem) = (path.parent()?, file_stem(path)?);
    let ova = ova_re.captures(stem)?.name("ova")?;
    let before = stem[..ova.start()]
        .trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '.' | '_'));
    // A season alone, e.g. `S02` in `S02OVA`, names no series.
    let before = if patterns.season_dir.is_match(before) {
        ""
    } else {
        before
    };
    let (Some(SeriesName { name, .. }), _) = patterns.run_series(before, dir) else {
        return None;
    };
    adder.add_claim(ClaimPredicate::SpecialOf(name));
    None
}

/// An OVA, OAD or ONA in a file name, maybe numbered, e.g. `OVA1`, `OVA - 01`
/// or `OVA` in `S02OVA`.
pub(super) const OVA_REGEX: &str =
    r"(?i)(?:^|[^\p{Alphabetic}])(?<ova>ova|oad|ona)(?:[\s._-]*\d+)?(?:[^\p{Alphabetic}]|$)";

/// Claims both names a season release gives its series, e.g. `my family` and
/// `storia della mia famiglia` for a video in `My Family AKA Storia della mia
/// famiglia S02 1080p`, as its videos may be named by one, e.g.
/// `My.Family.S02E01`, which another series has too, or by neither, e.g.
/// `S01E24 Document 34.mkv`. A knowledge base tells which series the name
/// shared is by the other.
pub(super) fn series_also_known_as(
    view: HeuristicView,
    mut adder: AddWithRank,
    patterns: &SeriesPatterns,
) {
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        if !facts.has_video_streams() {
            continue;
        }
        let names = path_id.rel_path.ancestors().skip(1).find_map(|dir| {
            let series = patterns.season_release_series(dir.file_name()?.to_str()?)?;
            let (one, other) = parse_aka(&series)?;
            Some([one, other].map(|name| name.trim().to_string()))
        });
        let Some(names) = names else {
            continue;
        };
        if let Some(mut adder) = adder.with_subject(subject) {
            for name in names.into_iter().filter(|name| title::has_words(name)) {
                adder.add_claim(ClaimPredicate::TvSeriesName(name));
            }
        }
    }
}

/// Claims each of a video's series' names without the words a release adds
/// to it: from the first that's surely a release's, e.g. `480p xvid mp3` of
/// `deadliest warrior complete 480p xvid mp3`, as [`ReleaseNoise::cut`]
/// cuts a title, and then those at its end that titles have too, e.g.
/// `complete` of what's left, `remastered` of `Detective Conan Remastered -
/// 045` or `english dub` of `Ultraman Max English Dub` (see
/// `release_words::without_weak`), as a knowledge base knows the series by
/// its name alone.
/// A second name, ranked below the name it's cut from: both are looked up,
/// as such words are sometimes a series' own.
///
/// After the heuristics that name series, whose claims it goes by.
pub(super) fn series_name_without_release_words(
    view: HeuristicView,
    mut adder: AddWithRank,
    noise: &ReleaseNoise,
) {
    for (subject, (_, facts)) in view.files.iter().enumerate() {
        if !facts.has_video_streams() {
            continue;
        }
        let mut cut: Vec<String> = Vec::new();
        for claim in adder.claims_of(subject).iter().filter(|claim| claim.active) {
            let ClaimPredicate::TvSeriesName(name) = &claim.pred else {
                continue;
            };
            if matches!(claim.source, ClaimSource::Heuristic(heuristic) if heuristic.is_last_resort_title())
            {
                continue;
            }
            let without = release_words::without_weak(&noise.cut(name));
            if without != *name && title::has_words(&without) && !cut.contains(&without) {
                cut.push(without);
            }
        }
        if let Some(mut adder) = adder.with_subject(subject) {
            for name in cut {
                adder.add_claim(ClaimPredicate::TvSeriesName(name));
            }
        }
    }
}
