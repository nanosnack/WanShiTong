//! The patterns that tell a series' files and directories by their names.

use super::{
    extras::is_extras_dir,
    names::{
        MOVIE_TITLE_REGEX, ParsedMovieTitle, file_stem, parse_movie_title, remove_square_brackets,
        roman_numeral, split_camel_case,
    },
};
use crate::interpret::release_words;
use regex::Regex;
use std::path::Path;

/// Names of files and directories that belong to a TV series.
pub(super) struct SeriesPatterns {
    pub(super) movie_title: Regex,
    sx_ex: Regex,
    n_x_nn: Regex,
    episode: Regex,
    creditless: Regex,
    extra_name: Regex,
    pub(super) season_dir: Regex,
    season_in_dir: Regex,
    season_range: Regex,
    season_suffix: Regex,
    number: Regex,
    parenthesized_number: Regex,
    episode_word: Regex,
    release_word: Regex,
    release_noise: Regex,
    resolution: Regex,
    tag: Regex,
}

impl SeriesPatterns {
    pub(super) fn new() -> Self {
        Self {
            movie_title: Regex::new(MOVIE_TITLE_REGEX).unwrap(),
            sx_ex: Regex::new(SX_EX_REGEX).unwrap(),
            n_x_nn: Regex::new(N_X_NN_REGEX).unwrap(),
            episode: Regex::new(EPISODE_REGEX).unwrap(),
            creditless: Regex::new(CREDITLESS_REGEX).unwrap(),
            extra_name: Regex::new(EXTRA_NAME_REGEX).unwrap(),
            season_dir: Regex::new(SEASON_DIR_REGEX).unwrap(),
            season_in_dir: Regex::new(SEASON_IN_DIR_REGEX).unwrap(),
            season_range: Regex::new(SEASON_RANGE_REGEX).unwrap(),
            season_suffix: Regex::new(SEASON_SUFFIX_REGEX).unwrap(),
            number: Regex::new(NUMBER_REGEX).unwrap(),
            parenthesized_number: Regex::new(PARENTHESIZED_NUMBER_REGEX).unwrap(),
            episode_word: Regex::new(EPISODE_WORD_REGEX).unwrap(),
            release_word: Regex::new(RELEASE_WORD_REGEX).unwrap(),
            release_noise: Regex::new(release_words::NOISE_REGEX).unwrap(),
            resolution: Regex::new(RESOLUTION_REGEX).unwrap(),
            tag: Regex::new(TAG_REGEX).unwrap(),
        }
    }

    /// Whether `text` has an episode's marker as a release writes it, e.g.
    /// `S01E02`, `E92` or `Season 1 Episode 01`. Not `Episode 1` alone, which
    /// is a title's words as often as a marker, e.g. Star Wars'.
    pub(super) fn has_marker_token(&self, text: &str) -> bool {
        if self.sx_ex.is_match(text) || self.n_x_nn.is_match(text) {
            return true;
        }
        let Some(c) = self.episode.captures(text) else {
            return false;
        };
        if c.name("first").is_some() || c.name("kanji_first").is_some() {
            return true;
        }
        let before = text[..c.get(0).unwrap().start()]
            .trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '.' | '_'));
        self.season_suffix_of(before).is_some()
    }

    /// Whether a file is named like an episode, e.g. `Show Name - S01E02.mkv`
    /// or `Show.Name.E92.mkv`.
    pub(super) fn is_episode(&self, path: &Path) -> bool {
        self.any_episode_marker(path).is_some()
    }

    /// Where a file name says which episodes the file is, by a marker, e.g.
    /// `S01E02`, or by a number its season directory corroborates, e.g. `730`
    /// in `Season 7/Show.Name.730.Title`.
    pub(super) fn any_episode_marker(&self, path: &Path) -> Option<EpisodeMarker> {
        self.episode_marker(file_stem(path)?)
            .or_else(|| self.season_episode_number(path))
    }

    /// The season and episode of a file named with a number that's the season
    /// its directory names and a two-digit episode, e.g. season 7, episode 30
    /// for `730` in `Season 7/Perry.Mason.730.The.Ugly.Duckling`. Only the
    /// directory tells it from any other number, e.g. a title's. Not when the
    /// name has a marker with a season, e.g. `S02E06`, which says it outright.
    pub(super) fn season_episode_number(&self, path: &Path) -> Option<EpisodeMarker> {
        let season = self.season_of_dirs(path).filter(|&season| season > 0)?;
        let stem = file_stem(path)?;
        if self
            .episode_marker(stem)
            .is_some_and(|marker| marker.season.is_some())
        {
            return None;
        }
        let prefix = season.to_string();
        self.standalone_numbers(stem).find_map(|(start, digits)| {
            let episode = digits.strip_prefix(&prefix)?;
            (episode.len() == 2).then_some(())?;
            let episode: u32 = episode.parse().ok().filter(|&episode| episode > 0)?;
            Some(EpisodeMarker {
                start,
                end: start + digits.len(),
                season: Some(season),
                episodes: [episode, episode],
            })
        })
    }

    /// A season's episode and a number through the whole series in
    /// parentheses right after it, e.g. (2, 72, 130) for `S02E72 (130)`. Only
    /// a number that could be one: past season 1, where the two are the
    /// same, and more than the episode's. Whether it is one, the files
    /// beside it say (see [`absolute_episodes_in_parentheses`](super::series::absolute_episodes_in_parentheses)).
    pub(super) fn episode_in_parentheses(&self, stem: &str) -> Option<(u32, u32, u32)> {
        let marker = self.episode_marker(stem)?;
        let season = marker.season.filter(|&season| season > 1)?;
        let [first, last] = marker.episodes;
        if first != last {
            return None;
        }
        let c = self.parenthesized_number.captures(&stem[marker.end..])?;
        let number: u32 = c["number"].parse().ok()?;
        (number > first).then_some((season, first, number))
    }

    /// Whether a directory's name says what kind of videos it holds rather
    /// than which, e.g. `Specials`, `Movies` or `OVA`, so that it names no
    /// movie, whatever its video is called.
    pub(super) fn is_kind_name(&self, name: &str) -> bool {
        self.release_word.is_match(name) || self.season_dir.is_match(name)
    }

    /// The numbers in a file name that are words of their own, outside tags,
    /// with where each starts, e.g. `730` in `Perry.Mason.730.The.Ugly.Duckling`,
    /// but not `264` in `x264` or `H.264`, or `720` in `720p`.
    fn standalone_numbers<'s>(&self, stem: &'s str) -> impl Iterator<Item = (usize, &'s str)> {
        let is_word = |c: char| c.is_alphanumeric();
        self.number.captures_iter(stem).filter_map(move |c| {
            let number = c.name("number")?;
            let before = &stem[..number.start()];
            let after = &stem[number.end()..];
            let in_tag = before.matches('[').count() > before.matches(']').count();
            // A codec's number, e.g. `H.264`, goes with the letter before it.
            let codec = before
                .strip_suffix('.')
                .and_then(|b| b.strip_suffix(['H', 'h']))
                .is_some_and(|b| !b.ends_with(is_word));
            (!in_tag && !codec && !before.ends_with(is_word) && !after.starts_with(is_word))
                .then(|| (number.start(), number.as_str()))
        })
    }

    /// Where a file name says which episodes the file is, and of which season,
    /// if it does, e.g. `S01E02`, `E93-E95`, `Episode 01` or
    /// `Season 1 Episode 01`.
    pub(super) fn episode_marker(&self, stem: &str) -> Option<EpisodeMarker> {
        let number = |m: regex::Match| m.as_str().parse::<u32>().ok();
        // A range's end below its start isn't one, e.g. `S01E05-02`.
        let range = |first: u32, last: Option<u32>| [first, last.unwrap_or(first).max(first)];
        if let Some(c) = self.sx_ex.captures(stem) {
            let first = number(c.name("first")?)?;
            let last = c.name("last").or(c.name("last_number")).and_then(number);
            return Some(EpisodeMarker {
                start: c.name("marker")?.start(),
                end: c.name("marker")?.end(),
                season: Some(number(c.name("season")?)?),
                episodes: range(first, last),
            });
        }
        if let Some(c) = self.n_x_nn.captures(stem) {
            let episode = number(c.name("episode")?)?;
            return Some(EpisodeMarker {
                start: c.name("marker")?.start(),
                end: c.name("marker")?.end(),
                season: Some(number(c.name("season")?)?),
                episodes: [episode, episode],
            });
        }
        let c = self.episode.captures(stem)?;
        let first = number(
            c.name("first")
                .or(c.name("number"))
                .or(c.name("kanji_first"))?,
        )?;
        let last = c.name("last").or(c.name("kanji_last")).and_then(number);
        let mut start = c.get(0)?.start();
        // A season just before it is the episodes', e.g. `Season 1` in
        // `Show Season 1 Episode 01` or `S01` in `Show.S01.E02`.
        let before = stem[..start]
            .trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '.' | '_'));
        let season = self.season_suffix_of(before).map(|(suffix_start, season)| {
            start = suffix_start;
            season
        });
        Some(EpisodeMarker {
            start,
            end: c.get(0)?.end(),
            season,
            episodes: range(first, last),
        })
    }

    /// The season named by the nearest of a file's directories that names
    /// one, e.g. 2 for `Show Name/Season 2` or `Show Name S02 1080p`, or 0 for
    /// `Specials`. A range of seasons, e.g. `Show Name S1 - S5`, is no season.
    pub(super) fn season_of_dirs(&self, path: &Path) -> Option<u32> {
        self.season_of_dir(path.parent()?)
    }

    /// The season named by `dir` or the nearest of its ancestors that names
    /// one, as [`Self::season_of_dirs`] finds it.
    fn season_of_dir(&self, dir: &Path) -> Option<u32> {
        dir.ancestors()
            .filter_map(|dir| dir.file_name()?.to_str())
            .find_map(|dir| {
                if dir.eq_ignore_ascii_case("specials") {
                    return Some(0);
                }
                let without_ranges = self.season_range.replace_all(dir, "");
                let c = self.season_in_dir.captures(&without_ranges)?;
                c["season"].parse().ok()
            })
    }

    /// Whether a file is named like an episode or a creditless opening or
    /// ending, so it's a series' file and not a movie's.
    pub(super) fn named_series_file(&self, path: &Path) -> bool {
        self.is_episode(path) || self.is_creditless(path)
    }

    /// Whether a file is named like a creditless opening or ending, e.g. `NCOP`.
    fn is_creditless(&self, path: &Path) -> bool {
        file_stem(path).is_some_and(|stem| self.creditless.is_match(stem))
    }

    /// Whether a file is named like an extra: a creditless opening or ending,
    /// or another extra, e.g. `[CM01]` or `Extra 1`.
    pub(super) fn is_named_extra(&self, path: &Path) -> bool {
        self.is_creditless(path)
            || file_stem(path).is_some_and(|stem| self.extra_name.is_match(stem))
    }

    /// Whether a file is named like an extra, or is in a directory of extras,
    /// e.g. `Extras` or `Openings & Endings`.
    pub(super) fn is_extra(&self, path: &Path) -> bool {
        self.is_named_extra(path)
            || path
                .parent()
                .is_some_and(|dir| dir.ancestors().any(is_extras_dir))
    }

    /// The series a file's name says it's of, as [`series_name_from_file_name`](super::series::series_name_from_file_name)
    /// claims it.
    pub(super) fn file_name_series(&self, path: &Path) -> Option<SeriesName> {
        let stem = file_stem(path)?;
        let dirs = path.parent()?;
        if let Some(marker) = self.any_episode_marker(path) {
            let before = &stem[..marker.start];
            // A marker in a tag of its own after tags, e.g.
            // `[Group][Show][第08话]`, is of the series the tags name, as a
            // number in a tag of its own is.
            let series = if self.only_tags(before) {
                self.tag_series(before).into_iter().next()
            } else {
                self.series_name(before)
            };
            return match series {
                Some(series) => Some(self.with_year_from_dirs(series, dirs)),
                None => self.series_name_from_dirs(dirs),
            };
        }
        let creditless = self.creditless.find(stem)?;
        // A season before it is the series', e.g. `S2` in
        // `One Punch Man S2 - NCED 01`, and not part of its name.
        let before = stem[..creditless.start()]
            .trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '.' | '_'));
        if self.season_dir.is_match(before) {
            return None;
        }
        let series = self.series_name(&self.season_suffix.replace(before, ""))?;
        Some(self.with_year_from_dirs(series, dirs))
    }

    /// The series, with the year a directory above names it with if its own
    /// name has none, e.g. 1955 for `Alfred Hitchcock Presents - S01E01` in
    /// `Alfred Hitchcock Presents (1955)/Season 1`.
    pub(super) fn with_year_from_dirs(&self, series: SeriesName, dirs: &Path) -> SeriesName {
        if series.year.is_some() {
            return series;
        }
        let year = dirs
            .ancestors()
            .filter_map(|dir| dir.file_name()?.to_str())
            .filter_map(|dir| self.series_name(dir))
            .find(|dir| dir.name == series.name && dir.year.is_some())
            .and_then(|dir| dir.year);
        SeriesName { year, ..series }
    }

    /// A series name in part of a file or directory name, e.g. `code geass` in
    /// `[Foo] Code Geass - `, or None if there's no name there. With the
    /// year after it, if any, e.g. 2011 in `Hunter x Hunter (2011) - `.
    pub(super) fn series_name(&self, text: &str) -> Option<SeriesName> {
        let ParsedMovieTitle { title, year } = parse_movie_title(&self.movie_title, text)?;
        let name = title.trim_end_matches([' ', '-']).trim_start();
        (!name.is_empty()).then(|| SeriesName {
            name: name.to_string(),
            year,
        })
    }

    /// The numbers in a file name that could be an episode's in a run, with
    /// where the name before each ends, e.g. 01 in `Show - 01v2 (1080p)`, or
    /// in a tag of its own, e.g. `[Group][Show][01][1080p]`, but not 265 in
    /// `x265`, 1080 in `1080p`, 12 in `12.Angry.Men`, a number in a tag of
    /// more, e.g. `[823-824]`, an OVA's, e.g. `OVA - 01`, or a fraction's,
    /// e.g. `024.5`, which numbers a special between episodes.
    pub(super) fn run_numbers(&self, stem: &str) -> Vec<(usize, u32)> {
        let is_word = |c: char| c.is_alphanumeric();
        self.number
            .captures_iter(stem)
            .filter_map(|c| {
                let m = c.get(0)?;
                let before = &stem[..m.start()];
                let after = &stem[m.end()..];
                // A tag that's only the number, e.g. `[01]` or `[01v2]`, unless
                // the tag before says it's a movie's or a special's, e.g.
                // `[OVA][01]`.
                if before.ends_with('[') && after.starts_with(']') {
                    let kind = self.ends_with_kind_tag(before);
                    return (!kind).then_some((m.start(), c["number"].parse().ok()?));
                }
                // Other tags, e.g. `[1080p]`, a CRC, or One Pace's `[823-824]`.
                if before.matches('[').count() > before.matches(']').count() {
                    return None;
                }
                if before.ends_with(is_word)
                    || after.starts_with(is_word)
                    || self.release_word.is_match(before)
                {
                    return None;
                }
                // A fraction, e.g. `024.5`, numbers a special between episodes.
                let fraction = after.strip_prefix('.').is_some_and(|rest| {
                    rest.starts_with(|c: char| c.is_ascii_digit())
                        && !rest[1..].starts_with(|c: char| c.is_alphanumeric())
                });
                if fraction {
                    return None;
                }
                // No title after it, only a separator, a tag or a version.
                let rest = after.trim_start_matches([' ', '.', '_']);
                let untitled = rest.is_empty()
                    || rest.starts_with(['-', '[', '('])
                    || rest.starts_with(|c: char| c.is_ascii_digit())
                    || rest.len() > 1
                        && rest.starts_with(['v', 'V'])
                        && rest[1..].starts_with(|c: char| c.is_ascii_digit());
                if !untitled {
                    return None;
                }
                Some((m.start(), c["number"].parse().ok()?))
            })
            .collect()
    }

    /// The season at the end of a name, and where it starts: e.g. 1 for ` S1`
    /// in `Show Name S1`, 2 for ` Season 2`, or 3 for ` III` in
    /// `Mob Psycho 100 III`.
    fn season_suffix_of(&self, text: &str) -> Option<(usize, u32)> {
        let c = self.season_suffix.captures(text)?;
        let season = match (c.name("season"), c.name("roman")) {
            (Some(number), _) => number.as_str().parse().ok()?,
            (_, Some(roman)) => roman_numeral(roman.as_str())?,
            _ => return None,
        };
        Some((c.get(0)?.start(), season))
    }

    /// The series and season of a run named `before` its numbers in `dir`:
    /// e.g. `one punch man` and 2 for `[bar] One Punch Man S2 - `, or the
    /// series the directories name for a run without a name, e.g. `01.MP4`.
    pub(super) fn run_series(&self, before: &str, dir: &Path) -> (Option<SeriesName>, Option<u32>) {
        if self.only_tags(before) {
            // The release group's tag, if it's the only one, names the series
            // only if no directory does, e.g. `[Group][01]`.
            let series = match self.tag_series(before).into_iter().next() {
                Some(series) => Some(self.with_year_from_dirs(series, dir)),
                None => self.series_name_from_dirs(dir).or_else(|| {
                    let group = *self.name_tags(before).first()?;
                    self.series_name(group)
                }),
            };
            return (series, self.season_of_dir(dir));
        }
        // Words outside tags before a number in a tag of its own name the
        // series, e.g. `show` for `[Group] Show [01]`.
        let before = remove_square_brackets(before);
        let separators = |c: char| c.is_whitespace() || matches!(c, '-' | '.' | '_' | '#');
        let before = before.trim_end_matches(separators);
        let before = self.episode_word.replace(before, "");
        let before = before.trim_end_matches(separators);
        let suffix = self.season_suffix_of(before);
        let season = suffix.map(|(_, season)| season);
        let name = suffix.map_or(before, |(start, _)| &before[..start]);
        let series = match self.series_name(name) {
            Some(series) => Some(self.with_year_from_dirs(series, dir)),
            None => self.series_name_from_dirs(dir),
        };
        (series, season.or_else(|| self.season_of_dir(dir)))
    }

    /// The series that the tags of a name made only of tags could name, the
    /// nearest first, e.g. `glimmer peaks` with 2020, then `donghua`, for
    /// `[Group][Donghua][Glimmer Peaks][2020][`: a run numbered in a tag of its
    /// own. Not the first of [`Self::name_tags`], which is the release
    /// group's; a year's tag is the series' year.
    pub(super) fn tag_series(&self, before: &str) -> Vec<SeriesName> {
        let year = self
            .tags(before)
            .filter_map(|tag| tag.parse::<u32>().ok())
            .find(|year| (1900..=2099).contains(year));
        self.name_tags(before)
            .into_iter()
            .skip(1)
            .rev()
            .filter_map(|tag| self.series_name(tag))
            .map(|series| SeriesName {
                year: series.year.or(year),
                ..series
            })
            .collect()
    }

    /// The title of a name made only of tags, e.g. `the glimmer` of 1992 for
    /// `[Group][The Glimmer 1992][1080p]`: the first of [`Self::name_tags`]
    /// after the release group's, with the year in it, or in a tag of its
    /// own, e.g. `[Group][The Glimmer][1992]`. None for a name with words
    /// outside its tags, which are its title, or with a tag of a number
    /// that's no year, e.g. `[01]`, which numbers an episode or a part.
    pub(super) fn tag_movie_title(&self, name: &str) -> Option<ParsedMovieTitle> {
        if remove_square_brackets(name)
            .chars()
            .any(char::is_alphanumeric)
        {
            return None;
        }
        let years: Vec<u32> = self
            .tags(name)
            .filter_map(|tag| {
                let number = tag.split(['v', 'V']).next()?;
                number
                    .chars()
                    .all(|c| c.is_ascii_digit())
                    .then(|| number.parse().ok())?
            })
            .collect();
        if years.iter().any(|year| !(1900..=2099).contains(year)) {
            return None;
        }
        let names = self.name_tags(name);
        let tag = names.get(1).or(names.first())?;
        let ParsedMovieTitle { title, year } = parse_movie_title(&self.movie_title, tag)?;
        Some(ParsedMovieTitle {
            title,
            year: year.or(years.first().copied()),
        })
    }

    /// The tag after the release group's that could name what a name made of
    /// tags is, e.g. `Title` for `[Group][4K][Title][menu][2160p]`, or the
    /// only one, as [`Self::tag_movie_title`] picks its title.
    pub(super) fn tag_title<'s>(&self, name: &'s str) -> Option<&'s str> {
        let names = self.name_tags(name);
        names.get(1).or(names.first()).copied()
    }

    /// Whether text names a season, e.g. `S2`, `Season 2` or `S02 1080p`.
    pub(super) fn names_season(&self, text: &str) -> bool {
        self.season_in_dir.is_match(text)
            || self
                .season_suffix_of(text.trim_end_matches(['[', ']']))
                .is_some()
    }

    /// Whether a name, up to a tag, ends with a tag that says what kind of
    /// video it is, e.g. `[SP]` or `[OVA]` in `[Group][Show][SP][`, so that a
    /// number in the next tag numbers specials, not episodes or parts.
    pub(super) fn ends_with_kind_tag(&self, before: &str) -> bool {
        self.release_word
            .is_match(before.trim_end_matches(['[', ']', ' ']))
    }

    /// Whether a name before a number in a tag of its own is all tags, e.g.
    /// `[Group][Show][`, rather than words with tags, e.g. `[Group] Show [`.
    pub(super) fn only_tags(&self, before: &str) -> bool {
        before.ends_with('[')
            && !remove_square_brackets(before)
                .chars()
                .any(char::is_alphanumeric)
    }

    /// The tags of a name that could name something, in order: not one of the
    /// release's, e.g. `[1080p]`, `[848x480]` or `[RAW]`, nor a kind of
    /// video's, e.g. `[SP]`, nor one without a letter, e.g. a year. `RAW` is a
    /// release's word only as a tag of its own: as a word, it's a title's
    /// too, e.g. `Raw.2016`.
    fn name_tags<'s>(&self, name: &'s str) -> Vec<&'s str> {
        self.tags(name)
            .filter(|tag| tag.chars().any(char::is_alphabetic))
            .filter(|tag| !self.release_noise.is_match(tag) && !self.resolution.is_match(tag))
            .filter(|tag| !tag.eq_ignore_ascii_case("raw") && !tag.eq_ignore_ascii_case("raws"))
            .filter(|tag| !self.is_kind_name(tag))
            .collect()
    }

    /// What's in each of a name's tags, e.g. `1080p` for `[1080p]`.
    fn tags<'s>(&self, name: &'s str) -> impl Iterator<Item = &'s str> {
        self.tag
            .captures_iter(name)
            .filter_map(|c| Some(c.get(1)?.as_str().trim()))
    }

    /// The series named by the nearest of `dirs` and its ancestors, e.g.
    /// `Show Name` for `Show Name/Season 1`. Directories of seasons and of
    /// extras are skipped, and a name is cut at its season, e.g. `Show Name`
    /// for `Show Name S1` or `Show Name S02 1080p BD Remux`.
    fn series_name_from_dirs(&self, dirs: &Path) -> Option<SeriesName> {
        dirs.ancestors()
            .filter(|dir| !is_extras_dir(dir))
            .filter_map(|dir| dir.file_name()?.to_str())
            .filter(|dir| !self.season_dir.is_match(dir))
            .find_map(|dir| self.series_name(&self.before_season(dir)))
    }

    /// The series a season release is named for, e.g. `seven worlds one planet`
    /// for `Seven.Worlds.One.Planet.S01.REPACK.2160p`: its name before its
    /// season. None for a directory without a season in its name.
    pub(super) fn season_release_series(&self, dir: &str) -> Option<String> {
        let dir = split_camel_case(dir);
        let without_ranges = self.season_range.replace_all(&dir, "");
        let c = self.season_in_dir.captures(&without_ranges)?;
        let season = c.name("marker")?;
        Some(self.series_name(&without_ranges[..season.start()])?.name)
    }

    /// The series `dir` and each directory above it could name, nearest first
    /// and each once, e.g. `whole cake island` and `one pace` for
    /// `OnePace/WholeCakeIsland`. Directories of seasons and of extras name
    /// none.
    pub(super) fn directory_series(&self, dir: &Path) -> Vec<String> {
        let mut names = Vec::new();
        let dirs = dir
            .ancestors()
            .filter(|dir| !is_extras_dir(dir))
            .filter_map(|dir| dir.file_name()?.to_str())
            .filter(|dir| !self.season_dir.is_match(dir));
        for dir in dirs {
            if let Some(SeriesName { name, .. }) = self.series_name(&self.before_season(dir))
                && !names.contains(&name)
            {
                names.push(name);
            }
        }
        names
    }

    /// A directory's name before its season, if it names one, e.g.
    /// `Show Name ` for `Show Name S02 1080p` or `Show Name III`. A range of
    /// seasons, e.g. `S1 - S5`, is left out, and names no season.
    fn before_season(&self, dir: &str) -> String {
        let dir = split_camel_case(dir);
        let without_ranges = self.season_range.replace_all(&dir, "");
        let season = self
            .season_in_dir
            .captures(&without_ranges)
            .and_then(|c| c.name("marker"));
        match season {
            Some(season) => without_ranges[..season.start()].to_string(),
            None => self.season_suffix.replace(&without_ranges, "").into_owned(),
        }
    }
}

/// A series' name, as a file or directory names it, and the year after it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SeriesName {
    pub(super) name: String,
    /// The year it first aired, e.g. 2011 in `Hunter x Hunter (2011)`.
    pub(super) year: Option<u32>,
}

/// What a file name says of the episodes the file is.
pub(super) struct EpisodeMarker {
    /// Where it starts in the name. The series' name is before it.
    pub(super) start: usize,
    /// Where it ends in the name. The episode's title may be after it.
    pub(super) end: usize,
    /// None if the episodes are numbered through the whole series.
    pub(super) season: Option<u32>,
    /// The first and last episode.
    pub(super) episodes: [u32; 2],
}

/// A season and its episodes in a file name: one, e.g. `S01E02`, or a range,
/// e.g. `S01E02E03`, `S01E02-E03` or `S01E02-03`, maybe with a release's
/// version, e.g. `S01E02v2`. Not within a word, though maybe between
/// underscores, e.g. `Show_S01E02_Title`.
const SX_EX_REGEX: &str = r"(?i)(?:^|[^\p{L}\p{N}])(?<marker>S(?<season>\d+)E(?<first>\d+)(?:(?:[-_]?E\d+)*[-_]?E(?<last>\d+)|-(?<last_number>\d+))?(?:v\d+)?)(?:$|[^\p{L}\p{N}])";

/// A season and an episode as `1x02`, e.g. in `Show 3x01 Title`. Not a
/// resolution, e.g. `640x480`, whose numbers are longer, nor a fraction's
/// end, e.g. `5.1x264`.
const N_X_NN_REGEX: &str =
    r"(?i)(?:^|[^\p{L}\p{N}.])(?<marker>(?<season>\d{1,2})x(?<episode>\d{2,3}))(?:$|[^\p{L}\p{N}])";

/// An episode number without a season in a file name, e.g. `E92`, a range,
/// e.g. `E93-E95`, or `Episode 01`, maybe with a release's version, e.g.
/// `E05v2`, or as Japanese and Chinese names write it, e.g. `第140話` or
/// `第12集`, or a range, `第01-39話`; not `第70回`, which as often numbers
/// an event of a series of them, e.g. a year's. `S01E02` has no word boundary
/// before its `E`, so it isn't one. Nor is a CRC, e.g. `[D30B6F1A]`, which
/// is longer than any episode number.
const EPISODE_REGEX: &str = r"(?i)\bE(?<first>\d{1,4})(?:-E?(?<last>\d{1,4}))?(?:v\d+)?\b|\b(?:episode|ep)[\s._]*(?<number>\d+)(?:v\d+)?\b|第\s*(?<kanji_first>\d{1,4})(?:\s*[-~～]\s*(?<kanji_last>\d{1,4}))?\s*[話话集]";

/// A season in a directory's name, e.g. `Season 2`, `Series 2` or `S02` in
/// `Show Name S02 1080p`. `Seasons 1-2` has no word boundary before its `s`.
/// It starts the name or follows a separator, so that `'s 7` in `Blake's 7` isn't
/// one.
const SEASON_IN_DIR_REGEX: &str =
    r"(?i)(?:^|[\s._\-\[\(])(?<marker>(?:season|series|s)[\s._]*(?<season>\d{1,3}))\b";

/// A range of seasons in a directory's name, e.g. `S1 - S5` or `Seasons 1-2`.
const SEASON_RANGE_REGEX: &str =
    r"(?i)\b(?:seasons?|series|s)[\s._]*\d+\s*-\s*(?:(?:season|series|s)[\s._]*)?\d+\b";

/// A directory of one season's episodes, e.g. `Season 1`, `S01` or `Specials`.
const SEASON_DIR_REGEX: &str = r"(?i)^(?:(?:season|series|s)\s*\d+|specials)$";

/// A creditless opening or ending in a file name, e.g. `NCOP`, `NC ED 2`,
/// `Creditless Opening` or `Textless Ending`.
const CREDITLESS_REGEX: &str = r"(?i)\b(?:nc[\s._-]*|(?:creditless|clean|textless)[\s._-]+)(?:op|ed|openings?|endings?)(?:\d|\b)";

/// Another extra's tag or number in a file name: a Zork-Glorp tag, e.g.
/// `[CM01]`, `[PV]`, `[Making02]` or `[Preview Nagato]`, or `Extra 1`, e.g. in
/// `Monster - Extra 1 - Origin`. Not untagged words like `Trailer` or
/// `Making`, which titles have, e.g. `Trailer Park Boys` or `Making a Murderer`.
const EXTRA_NAME_REGEX: &str = r"(?i)\[(?:announce|cm|making|menu|preview|pv|teaser|trailer)(?:[\s_]*\d+|\s[^\]]*)?\]|(?:^|[\s._-])extra[\s._]*\d+(?:$|[\s._-])";

/// A season number at the end of a name, e.g. ` S1` in `Show Name S1`,
/// ` Season 2` in `Show Name Season 2`, or a roman numeral, e.g. ` III` in
/// `Mob Psycho 100 III`. Numerals are uppercase, as release names write
/// them, from II to IX: `I` is often a word, and `X` ends titles, e.g.
/// `Gundam X`.
const SEASON_SUFFIX_REGEX: &str =
    r"(?i)[\s._-]+(?:(?:season|s)\s*(?<season>\d+)|(?-i:(?<roman>II|III|IV|V|VI|VII|VIII|IX)))$";

/// A number in a file name, maybe with a release's version, e.g. `01v2`.
const NUMBER_REGEX: &str = r"(?<number>\d{1,4})(?:[vV]\d+)?";

/// A video's width by its height, e.g. `1920x1080`.
const RESOLUTION_REGEX: &str = r"(?i)\b\d{3,4}\s*[x×]\s*\d{3,4}\b";

/// A tag in a name, e.g. `[1080p]`, and what's in it.
const TAG_REGEX: &str = r"\[([^\[\]]*)\]";

/// A number in parentheses at the start, after separators, e.g. ` (130)`.
const PARENTHESIZED_NUMBER_REGEX: &str = r"^[\s._-]*\((?<number>\d{1,4})\)";

/// A word for an episode at the end of a run's name before its numbers, e.g.
/// ` - Chapter` in `Monster - Chapter 01`.
const EPISODE_WORD_REGEX: &str = r"(?i)[\s._-]*\b(?:chapter|episode|ep)$";

/// A word for a series' movie, OVA or special just before a number, e.g.
/// `OVA - ` in `One Punch Man S2 - OVA - 01`, or `SP` in `[Group][Show][SP][01]`.
const RELEASE_WORD_REGEX: &str = r"(?i)\b(?:movie|ova|oad|ona|sp|specials?)[\s._-]*$";
