//! Movie titles, years and numbers in file and directory names.

use regex::Regex;
use std::{path::Path, sync::LazyLock};

/// A movie's title and year in a name: its year is the first word after a
/// title that could be a release's year, e.g. 2012 in `Berserk Golden Age
/// Arc (2012-2013)`, or the second of two in a row, the first being the
/// title's, e.g. 2020 in `Wonder.Woman.1984.2020`, unless they're a year
/// apart or less, e.g. `The.Amateur.2025.2025`. Not one that's
/// yet to come, which is a title's number, e.g. 2049 in
/// `Blade.Runner.2049.2017`, nor one the name begins with, e.g. 2001 in
/// `2001.A.Space.Odyssey.1968`. A year is a word of its own, after a
/// separator, e.g. not 2020 in `x264-Blorp2020`. Without a year, the whole
/// name is the title.
pub(super) fn parse_movie_title(re: &Regex, file_stem: &str) -> Option<ParsedMovieTitle> {
    static NEXT_YEAR: LazyLock<i16> = LazyLock::new(|| jiff::Zoned::now().year() + 1);
    let clean_stem = clean_file_stem(file_stem);
    let is_separator = |c: char| c.is_whitespace() || matches!(c, '.' | '(' | '-' | '[');
    let is_year = |year: &regex::Match| {
        let before = &clean_stem[..year.start()];
        let rest = &clean_stem[year.end()..];
        let after = rest.chars().next();
        // A width, e.g. 1920 of `1920x1080`.
        let width = rest
            .strip_prefix(['x', 'X', '×'])
            .is_some_and(|height| height.starts_with(|c: char| c.is_ascii_digit()));
        before.ends_with(is_separator)
            && before.chars().any(char::is_alphanumeric)
            && !after.is_some_and(|c| c.is_ascii_digit())
            && !width
            && year
                .as_str()
                .parse::<i16>()
                .is_ok_and(|year| year <= *NEXT_YEAR)
    };
    let mut years = re.find_iter(&clean_stem).filter(is_year);
    let first = years.next();
    // Of two years in a row, the second, and the first is the title's, e.g.
    // `Wonder.Woman.1984.2020`. Not of a range, e.g. `(2012-2013)`, nor the
    // year twice, or a year and the next, e.g. a film's and its release's:
    // `The.Amateur.2025.2025` or `Million.Dollar.Baby.2004.2005`.
    let value = |year: &regex::Match| year.as_str().parse::<u32>().unwrap_or_default();
    let year = match (first, years.next()) {
        (Some(first), Some(second))
            if clean_stem[first.end()..second.start()]
                .chars()
                .all(|c| c.is_whitespace() || matches!(c, '.' | '_' | '(' | ')'))
                && value(&first).abs_diff(value(&second)) > 1 =>
        {
            Some(second)
        }
        _ => first,
    };
    let (raw_title, year) = match year {
        Some(year) => (
            clean_stem[..year.start()].trim_end_matches(is_separator),
            year.as_str().parse().ok(),
        ),
        None => (clean_stem.as_str(), None),
    };
    (!raw_title.is_empty()).then(|| ParsedMovieTitle {
        title: normalize_title(raw_title),
        year,
    })
}

/// A name without its tags in square brackets, but for a year alone in one
/// after a title, e.g. 2002 of `Aankhen.[2002].1080p`, which is the movie's.
/// Not one after tags alone, e.g. `[Group][Title][2019]`, which names its
/// movie in its tags.
fn clean_file_stem(stem: &str) -> String {
    static BRACKETED_YEAR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\[((?:19|20)\d{2})\]").unwrap());
    let kept = BRACKETED_YEAR.replace_all(stem, |captures: &regex::Captures| {
        let before = &stem[..captures.get(0).unwrap().start()];
        if remove_square_brackets(before)
            .chars()
            .any(char::is_alphabetic)
        {
            format!(" {} ", &captures[1])
        } else {
            captures[0].to_string()
        }
    });
    remove_square_brackets(&kept)
}

/// A name written in camel case, without separators, split into words, e.g.
/// `One Pace` for `OnePace` or `Whole Cake Island` for `WholeCakeIsland`.
/// Other names are left as they are.
pub(super) fn split_camel_case(name: &str) -> String {
    if name.contains(|c: char| c.is_whitespace() || matches!(c, '.' | '_' | '-')) {
        return name.to_string();
    }
    let mut words = String::with_capacity(name.len() + 4);
    let mut previous = None;
    for c in name.chars() {
        if previous.is_some_and(char::is_lowercase) && c.is_uppercase() {
            words.push(' ');
        }
        words.push(c);
        previous = Some(c);
    }
    words
}

/// Removes tags in square brackets, e.g. `[1080p]`, including one left open
/// at the end, e.g. `[` in `Show Name [` when a name is cut at `[NCOP]`.
pub(super) fn remove_square_brackets(name: &str) -> String {
    let re = Regex::new(r"\[[^\]]*(?:\]|$)").unwrap();
    re.replace_all(name, "").trim().to_string()
}

fn normalize_title(raw_title: &str) -> String {
    let raw_title = raw_title.trim();
    let mut last_was_space = false;
    let mut out = String::with_capacity(raw_title.len());

    for c in raw_title.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else if matches!(c, '.' | '_' | ':' | ',') {
            out.push(' ');
            last_was_space = true;
        } else if matches!(c, '\'') {
            continue;
        } else {
            out.push(c.to_ascii_lowercase());
            last_was_space = false;
        }
    }

    out
}

pub(super) struct ParsedMovieTitle {
    pub(super) title: String,
    pub(super) year: Option<u32>,
}

pub(super) fn parse_aka(title: &str) -> Option<(&str, &str)> {
    title
        .split_once(" AKA ")
        .or_else(|| title.split_once(" aka "))
}

pub(super) fn file_stem(path: &Path) -> Option<&str> {
    path.file_stem()?.to_str()
}

/// The value of a roman numeral from I to X.
pub(super) fn roman_numeral(numeral: &str) -> Option<u32> {
    let value = match numeral {
        "I" => 1,
        "II" => 2,
        "III" => 3,
        "IV" => 4,
        "V" => 5,
        "VI" => 6,
        "VII" => 7,
        "VIII" => 8,
        "IX" => 9,
        "X" => 10,
        _ => return None,
    };
    Some(value)
}

/// A year-like number in a movie's name, of which [`parse_movie_title`] takes
/// its year. Not past 2099.
pub(super) const MOVIE_TITLE_REGEX: &str = r"(?:19|20)\d{2}";

/// A series' movie or OVA number in a file name, e.g. `Movie 01` or `OVA02`.
pub(super) const RELEASE_NUMBERING_REGEX: &str = r"(?i)\b(?:movie|ova|oad|ona)[\s._]*\d+\b";
