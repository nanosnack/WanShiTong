//! Runs of videos numbered alike, which are a series' episodes or movies.

use super::{
    AddToClaims, HeuristicView,
    names::{file_stem, parse_movie_title, remove_square_brackets},
    series_patterns::{SeriesName, SeriesPatterns},
};
use regex::Regex;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::Path,
    time::Duration,
};
use wanshitong_core::domain::{ClaimPredicate, FileRole, Heuristic, SubjectIndex};

/// Claims that videos numbered in a run with their siblings are episodes of a
/// series, e.g. `[bar] One Punch Man - 01 [BD 1080p FLAC].mkv`, `- 02` and
/// `- 03` in one directory, Chibi Maruko-chan's `01.MP4`, `02.MP4` and
/// `03.MP4`, or `[Group][Show][01][1080p].mkv`, `[02]` and `[03]`.
///
/// A series' movies are numbered after its name too, e.g. `Toy Story 2`, so
/// a number is only an episode's where it's what varies between at least
/// [`MIN_RUN_LENGTH`] videos of a directory whose names are alike before it,
/// and:
/// - no title follows it, as in `12.Angry.Men` and `21.Jump.Street`: only the
///   end of the name, a separator, a tag or a version, e.g. ` - Title`,
///   ` (1080p)`, ` 1080p` or `v2`,
/// - the videos don't name different years, as `Toy Story 1 1995` and
///   `Toy Story 2 1999` do,
/// - they aren't extras, e.g. `Extras/Shorts 01`, nor a series' numbered
///   movies or OVAs, e.g. `Movie 01`.
///
/// The episodes are of the season named before the number, e.g.
/// `One Punch Man S2 - 01`, or by the directory, e.g. `Season 1/Show - 01`, or
/// else numbered through the whole series. The series is named before the
/// number, e.g. `monster` for `Monster - Chapter 01`, or by the tags before
/// it, e.g. `show` for `[Group][Show][01]`, or else by the directories above.
pub(super) fn numbered_runs(
    view: HeuristicView,
    adder: &mut AddToClaims,
    [episodes_rank, features_rank]: [f32; 2],
    patterns: &SeriesPatterns,
    numbering_re: &Regex,
) {
    for run in find_runs(view, patterns, numbering_re) {
        let is_episodes = run.is_episodes();
        let rank = if is_episodes {
            episodes_rank
        } else {
            features_rank
        };
        let Run {
            dir,
            before,
            members,
            bonus_of,
        } = run;
        // A series' bonus videos, e.g. `[Group][Show][Previews][01]` beside
        // `[Group][Show][01]`: its extras.
        if let Some((series_dir, series_before)) = bonus_of {
            let (series, _) = patterns.run_series(&series_before, series_dir);
            for RunMember { subject, .. } in members {
                let mut adder = adder.with(Heuristic::NumberedRun, rank);
                let Some(mut adder) = adder.with_subject(subject) else {
                    continue;
                };
                adder.add_claim(ClaimPredicate::FileRole(FileRole::Extra));
                if let Some(SeriesName { name, .. }) = &series {
                    adder.add_claim(ClaimPredicate::TvSeriesName(name.clone()));
                }
            }
            continue;
        }
        let (series, season) = patterns.run_series(&before, dir);
        let own_name = series.as_ref().map(|series| series.name.as_str());
        let dir_series = other_series(patterns, &before, dir, own_name);
        for RunMember {
            subject, number, ..
        } in members
        {
            // Each series a directory names, a little below the run's own name,
            // the nearer the higher. They're all looked up, and a knowledge base
            // result that has the run as a season can outrank the others.
            for (level, name) in (1..).zip(&dir_series) {
                let rank = rank - DIRECTORY_SERIES_STEP * level as f32;
                let mut adder = adder.with(Heuristic::NumberedRun, rank);
                if let Some(mut adder) = adder.with_subject(subject) {
                    adder.add_claim(ClaimPredicate::TvSeriesName(name.clone()));
                }
            }
            let mut adder = adder.with(Heuristic::NumberedRun, rank);
            let Some(mut adder) = adder.with_subject(subject) else {
                continue;
            };
            if let Some(SeriesName { name, year }) = &series {
                adder.add_claim(ClaimPredicate::TvSeriesName(name.clone()));
                if let Some(year) = *year {
                    adder.add_claim(ClaimPredicate::TvSeriesYear(year));
                }
            }
            match season {
                Some(season) => {
                    adder.add_claim(ClaimPredicate::Season(season));
                    adder.add_claim(ClaimPredicate::EpisodeRange([number, number]));
                }
                None => adder.add_claim(ClaimPredicate::AbsoluteEpisodeRange([number, number])),
            }
        }
    }
}

/// A run of videos numbered alike in a directory, e.g. `Show - 01`, `- 02` and
/// `- 03`, as [`numbered_runs`] describes.
pub(super) struct Run<'a> {
    pub(super) dir: &'a Path,
    /// The name before the numbers, without tags, or with them, ending in
    /// `[`, when the numbers are in tags of their own, e.g. `[Group][Show][`.
    pub(super) before: String,
    members: Vec<RunMember>,
    /// The run whose bonus videos it is, by its directory and its name before
    /// its numbers: one numbered in tags whose tags this run's begin with,
    /// in its directory or one above, e.g. `[Group][Show][` for
    /// `[Group][Show][Previews][`. Not if the tags after name a season, e.g.
    /// `[Group][Show][S2][`.
    pub(super) bonus_of: Option<(&'a Path, String)>,
}

impl Run<'_> {
    /// Whether its videos are as long as episodes, rather than a series'
    /// movies.
    pub(super) fn is_episodes(&self) -> bool {
        let mut durations = self.members.iter().filter_map(|m| m.duration).peekable();
        durations.peek().is_some() && durations.all(|duration| duration < MAX_EPISODE_DURATION)
    }

    /// Whether its videos could be episodes: none is as long as a feature,
    /// though none may have been measured.
    pub(super) fn may_be_episodes(&self) -> bool {
        self.members
            .iter()
            .filter_map(|m| m.duration)
            .all(|duration| duration < MAX_EPISODE_DURATION)
    }
}

/// Claims that a video numbered as an episode is one, though no others are
/// numbered beside it, e.g. a release of one: `[Group] Show - 19 [720p].mkv`,
/// `Show 02 [720p].mp4` or `[Group][Show][03][1080p].mp4`. Only a number set
/// apart as episodes' are and sequels' aren't: after a dash, in a tag of its
/// own, or with a leading zero. Not `Toy Story 2`, nor a year, e.g.
/// `Concert - 2019`, nor in a code, e.g. `U-571`, nor a decimal, e.g.
/// `- 1.30GB`, nor in a name with a release year, which is a movie's, e.g.
/// `Fear Street - 1666 (2021)`. Weak evidence, below what says it's a movie,
/// e.g. a knowledge base's movie as long as it.
pub(super) fn lone_numbered_episodes(
    view: HeuristicView,
    adder: &mut AddToClaims,
    rank: f32,
    patterns: &SeriesPatterns,
    numbering_re: &Regex,
) {
    let in_runs: HashSet<SubjectIndex> = find_runs(view, patterns, numbering_re)
        .into_iter()
        .flat_map(|run| run.members.into_iter().map(|member| member.subject))
        .collect();
    let groups = numbered_videos(view, patterns, numbering_re);
    // The videos numbered in only one way, and alone in their group.
    let mut lone = HashMap::<SubjectIndex, Vec<(&Path, &str, &RunMember)>>::new();
    for ((dir, before), members) in &groups {
        if let [member] = members.as_slice() {
            lone.entry(member.subject)
                .or_default()
                .push((dir, before, member));
        }
    }
    for (subject, numbers) in lone {
        let [(dir, before, member)] = numbers.as_slice() else {
            continue;
        };
        if in_runs.contains(&subject) || (1900..=2099).contains(&member.number) {
            continue;
        }
        let Some(stem) = file_stem(&view.files[subject].0.rel_path) else {
            continue;
        };
        let digits = &stem[member.start..];
        let after = digits.trim_start_matches(|c: char| c.is_ascii_digit());
        // A decimal, e.g. a size's `1.30GB` or a sound's `5.1`, numbers no
        // episode, and a name with a release year is a movie's.
        let decimal =
            after.starts_with('.') && after[1..].starts_with(|c: char| c.is_ascii_digit());
        if decimal || member.year.is_some() {
            continue;
        }
        // A dash between words and the number, not one in a code, e.g.
        // `U-571`, `HX-01`, `VC-1` or `title-1080`, which no number is set
        // apart in.
        let code = stem[..member.start]
            .strip_suffix('-')
            .is_some_and(|before| before.ends_with(|c: char| c.is_alphanumeric()));
        if code {
            continue;
        }
        let dashed = stem[..member.start]
            .trim_end_matches([' ', '_', '.'])
            .strip_suffix('-')
            .is_some_and(|before| before.ends_with([' ', '_', '.']));
        let set_apart = before.ends_with('[')
            || dashed
            || digits.starts_with('0') && digits[1..].starts_with(|c: char| c.is_ascii_digit());
        if !set_apart {
            continue;
        }
        let (Some(SeriesName { name, year }), season) = patterns.run_series(before, dir) else {
            continue;
        };
        for (level, other) in (1..).zip(other_series(patterns, before, dir, Some(&name))) {
            let rank = rank - DIRECTORY_SERIES_STEP * level as f32;
            let mut adder = adder.with(Heuristic::LoneNumberedEpisode, rank);
            if let Some(mut adder) = adder.with_subject(subject) {
                adder.add_claim(ClaimPredicate::TvSeriesName(other));
            }
        }
        let mut adder = adder.with(Heuristic::LoneNumberedEpisode, rank);
        let Some(mut adder) = adder.with_subject(subject) else {
            continue;
        };
        adder.add_claim(ClaimPredicate::TvSeriesName(name));
        if let Some(year) = year {
            adder.add_claim(ClaimPredicate::TvSeriesYear(year));
        }
        let number = member.number;
        match season {
            Some(season) => {
                adder.add_claim(ClaimPredicate::Season(season));
                adder.add_claim(ClaimPredicate::EpisodeRange([number, number]));
            }
            None => adder.add_claim(ClaimPredicate::AbsoluteEpisodeRange([number, number])),
        }
    }
}

/// The other series a numbered video named `before` its number in `dir`
/// could be of, besides `own_name`, nearest first: the other tags before a
/// number in a tag of its own, e.g. `donghua` for `[Group][Donghua][Glimmer
/// Peaks][01]`, or all of them past the group's if words outside them name
/// it, e.g. `tokyo ghoul-re s2` for `【Note】◆Words◆[Tokyo Ghoul-re S2][09]`;
/// then the series the directories above could name, e.g. `One Pace` for
/// `OnePace/WholeCakeIsland`, of which a run would be an arc.
fn other_series(
    patterns: &SeriesPatterns,
    before: &str,
    dir: &Path,
    own_name: Option<&str>,
) -> Vec<String> {
    let tags = if !before.ends_with('[') {
        Vec::new()
    } else if patterns.only_tags(before) {
        patterns.tag_series(before).into_iter().skip(1).collect()
    } else {
        patterns.tag_series(before)
    };
    let mut names: Vec<String> = Vec::new();
    for name in tags
        .into_iter()
        .map(|series| series.name)
        .chain(patterns.directory_series(dir))
    {
        if Some(name.as_str()) != own_name && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// The runs of the videos in view, as [`numbered_runs`] describes them, in
/// order, each video in only the first run it's in.
pub(super) fn find_runs<'a>(
    view: HeuristicView<'a>,
    patterns: &SeriesPatterns,
    numbering_re: &Regex,
) -> Vec<Run<'a>> {
    let runs = numbered_videos(view, patterns, numbering_re);

    // In order, so that a video in several runs is always in the same one.
    let mut runs = Vec::from_iter(runs);
    runs.sort_by(|a, b| a.0.cmp(&b.0));
    runs.retain(|(_, members)| {
        let numbers: BTreeSet<u32> = members.iter().map(|m| m.number).collect();
        let years: BTreeSet<u32> = members.iter().filter_map(|m| m.year).collect();
        numbers.len() >= MIN_RUN_LENGTH && years.len() <= 1
    });
    let tagged: Vec<(&Path, String)> = runs
        .iter()
        .map(|(key, _)| key.clone())
        .filter(|(_, before)| before.ends_with('['))
        .collect();
    let mut numbered = HashSet::new();
    runs.into_iter()
        .map(|((dir, before), members)| {
            let bonus_of = before
                .ends_with('[')
                .then(|| {
                    tagged.iter().find(|(series_dir, series_before)| {
                        before.len() > series_before.len()
                            && before.starts_with(series_before.as_str())
                            && dir.starts_with(series_dir)
                            && !patterns.names_season(&before[series_before.len()..])
                    })
                })
                .flatten()
                .cloned();
            let members = members
                .into_iter()
                .filter(|m| numbered.insert(m.subject))
                .collect();
            Run {
                dir,
                before,
                members,
                bonus_of,
            }
        })
        .collect()
}

/// The videos of each directory numbered as [`numbered_runs`] numbers them,
/// by what's before their number: without tags, e.g. `Whole Cake Island` for
/// `[823-824] Whole Cake Island 01`, or with them for a number in a tag of
/// its own, e.g. `[Group][Show][` for `[Group][Show][2020][01]`, so that its
/// run isn't the year's.
fn numbered_videos<'a>(
    view: HeuristicView<'a>,
    patterns: &SeriesPatterns,
    numbering_re: &Regex,
) -> HashMap<(&'a Path, String), Vec<RunMember>> {
    let mut runs = HashMap::<(&Path, String), Vec<RunMember>>::new();
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        let path = &path_id.rel_path;
        if !facts.has_video_streams() || patterns.is_extra(path) {
            continue;
        }
        let (Some(dir), Some(stem)) = (path.parent(), file_stem(path)) else {
            continue;
        };
        if numbering_re.is_match(stem) {
            continue;
        }
        let year = parse_movie_title(&patterns.movie_title, stem).and_then(|t| t.year);
        for (start, number) in patterns.run_numbers(stem) {
            let before = &stem[..start];
            let before = if before.ends_with('[') {
                before.to_string()
            } else {
                remove_square_brackets(before)
            };
            runs.entry((dir, before)).or_default().push(RunMember {
                subject,
                number,
                start,
                year,
                duration: facts.media.as_ref().and_then(|m| m.duration()),
            });
        }
    }
    runs
}

/// A video numbered like an episode, before its run is known.
struct RunMember {
    subject: SubjectIndex,
    number: u32,
    /// Where its number starts in its name.
    start: usize,
    /// The year its name says, if any.
    year: Option<u32>,
    duration: Option<Duration>,
}

/// Longer than any episode, e.g. an OVA's hour, but shorter than most
/// features.
const MAX_EPISODE_DURATION: Duration = Duration::from_secs(75 * 60);

/// How much the rank of a directory's series-name claim drops for each level
/// the directory sits above the run.
const DIRECTORY_SERIES_STEP: f32 = 0.1;

/// How many videos, with different numbers, make a run of episodes. Fewer
/// could be a series' movies, e.g. a trilogy.
const MIN_RUN_LENGTH: usize = 3;
