//! Other queries a file's titles may be searched for by as a last resort, e.g. split into a series' name and an episode's number. What only shapes a search, not what the file is claimed to be.

use super::*;
use crate::interpret::release_words;

/// How many series' names a file's titles are looked up as, split at an
/// episode's number: its best title's splits, and a cleaner one's.
pub(super) const SERIES_AND_EPISODE_SEARCHES: usize = 3;

/// A title split as a series' name and an episode, e.g. `samurai sentai
/// blorp` and 22 for `samurai sentai blorp 22 hd`, or `wumbo lark` and 6 for
/// `wumbo lark 06sd`. The splits it could be, likeliest first:
///
/// - At a number at its end, after any release's words, e.g. `zibble e01
///   720p` or `zibble episode 1`.
/// - At the first number with words before it, followed by the episode's
///   own title, e.g. `glimmer cx 260 majuuou` or `wumbo lark 16 - the
///   bright road (dvd x264 aac)`. A movie's file isn't split, as its
///   sequel is named so, e.g. `zibble 5: the final night (1994)`: see
///   `Search::is_movie`.
///
/// A season written just before the number, e.g. `zibble s1 _ 13`, is the
/// episode's. Not a number after a word that says it's another's, e.g. a
/// volume's, a day's or a part's, as in `vol 1` or `day 2`, nor a movie's,
/// as in `movie 3`, nor a year.
pub(super) fn series_and_episode(title: &str) -> Vec<SeriesEpisode> {
    static RELEASE_WORD: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(release_words::AFTER_EPISODE_REGEX).unwrap());
    static NUMBER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(?:e|ep|#)?(\d{1,4})(?:v\d)?(?:sd|hd)?$").unwrap());
    static SEASON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^s(\d{1,2})$").unwrap());
    let mut words: Vec<&str> = title
        .split_whitespace()
        .filter(|word| !(word.starts_with(['(', '[']) || word.ends_with([')', ']'])))
        .collect();
    while words.last().is_some_and(|word| RELEASE_WORD.is_match(word)) {
        words.pop();
    }
    // The number's index: at the end, then the first with words before.
    let at_end = words.len().checked_sub(1);
    let first = (1..words.len()).find(|&i| NUMBER.is_match(words[i]));
    let mut splits = Vec::new();
    for at in at_end.into_iter().chain(first) {
        if let Some(split) = split_at_number(&words[..at], words[at], &NUMBER, &SEASON)
            && !splits.contains(&split)
        {
            splits.push(split);
        }
    }
    splits
}

/// A series' name and an episode, by a number in a title and the words
/// before it, as [`series_and_episode`] splits it.
pub(super) fn split_at_number(
    before: &[&str],
    number: &str,
    number_re: &Regex,
    season_re: &Regex,
) -> Option<SeriesEpisode> {
    let episode: u32 = number_re.captures(number)?[1].parse().ok()?;
    let mut words = before.to_vec();
    let pop_while = |words: &mut Vec<&str>, pop: &dyn Fn(&str) -> bool| {
        while words.last().is_some_and(|word| pop(word)) {
            words.pop();
        }
    };
    let separator = |word: &str| matches!(word, "-" | "–" | "~" | "#" | "_");
    let episode_word = |word: &str| {
        matches!(
            word.to_lowercase().trim_end_matches('.'),
            "episode" | "ep" | "e" | "folge" | "episodio" | "épisode" | "capitulo" | "capítulo"
        )
    };
    pop_while(&mut words, &|word| episode_word(word) || separator(word));
    let season = words
        .last()
        .and_then(|word| season_re.captures(word))
        .and_then(|captures| captures[1].parse::<u32>().ok());
    if season.is_some() {
        words.pop();
        pop_while(&mut words, &separator);
    }
    let not_an_episode = |word: &str| {
        matches!(
            word.to_lowercase().trim_end_matches('.'),
            "vol"
                | "volume"
                | "day"
                | "part"
                | "disc"
                | "disk"
                | "cd"
                | "dvd"
                | "bd"
                | "season"
                | "chapter"
                | "movie"
                | "film"
                | "ova"
                | "oad"
                | "ona"
                | "sp"
                | "special"
                | "live"
                | "tour"
                | "no"
                | "number"
                | "box"
        )
    };
    let year = (1900..=2099).contains(&episode);
    if episode == 0 || year || words.last().is_none_or(|word| not_an_episode(word)) {
        return None;
    }
    let name = words.join(" ");
    title::has_words(&name).then_some(SeriesEpisode {
        name,
        season,
        episode,
    })
}

/// A series' name, and an episode of it: through the whole series, or of
/// a season if the title says one.
#[derive(Debug, PartialEq)]
pub(super) struct SeriesEpisode {
    pub(super) name: String,
    pub(super) season: Option<u32>,
    pub(super) episode: u32,
}

/// A title as a series' name and a season's at its end, after any release's
/// words, e.g. `wumbo lark` and 2 for `wumbo lark 2nd season`, `zibble s02`,
/// `zibble season 2`, `glimmer ii`, or `glimmer fourth stage`. Not a part's
/// or a cour's, which a knowledge base lists otherwise.
pub(super) fn series_and_season(title: &str) -> Option<(String, u32)> {
    static SEASON: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?ix)^(?P<name>.+?)[\s:_-]+(?:
                s(?P<s>\d{1,2})
              | season[\s._-]*(?P<season>\d{1,2})
              | (?P<nth>\d{1,2})(?:st|nd|rd|th)[\s._-]+(?:season|stage)
              | (?P<word>second|third|fourth|fifth|sixth)[\s._-]+(?:season|stage)
              | (?P<roman>ii|iii|iv|v|vi)
            )(?:[\s._-]+(?:hd|sd|fhd|\d{3,4}p|bdrip|bluray|web-?dl|vostfr|raw|bd|tv))*\s*$",
        )
        .unwrap()
    });
    let captures = SEASON.captures(title.trim())?;
    let number = |name: &str| {
        captures
            .name(name)
            .and_then(|m| m.as_str().parse::<u32>().ok())
    };
    let season = number("s")
        .or_else(|| number("season"))
        .or_else(|| number("nth"))
        .or_else(|| {
            captures
                .name("word")
                .map(|word| match word.as_str().to_lowercase().as_str() {
                    "second" => 2,
                    "third" => 3,
                    "fourth" => 4,
                    "fifth" => 5,
                    _ => 6,
                })
        })
        .or_else(|| {
            captures
                .name("roman")
                .map(|roman| match roman.as_str().to_lowercase().as_str() {
                    "ii" => 2,
                    "iii" => 3,
                    "iv" => 4,
                    "v" => 5,
                    _ => 6,
                })
        })?;
    let name = captures["name"]
        .trim()
        .trim_end_matches(['-', ':', '~'])
        .trim();
    (season >= 1 && title::has_words(name)).then(|| (name.to_string(), season))
}

/// How many words a title spelled near a movie's may have: a longer one's
/// searches without a word are too many, and it's rarely only misspelled.
pub(super) const NEAR_SPELLING_MAX_WORDS: usize = 6;
/// How many searches a file's titles without a word get at most.
pub(super) const NEAR_SPELLING_SEARCHES: usize = 6;
