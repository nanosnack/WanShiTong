//! Heuristics for the parts of a work, e.g. `Part 2`.

use super::{
    AddToSubject, AddWithRank, Done, HeuristicView,
    names::{ParsedMovieTitle, file_stem, parse_movie_title, roman_numeral},
    series_patterns::SeriesPatterns,
};
use regex::Regex;
use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
};
use wanshitong_core::domain::{ClaimPredicate, FileFacts, PathId, SubjectIndex};

/// Claims the part of its work that a number in a tag in a video's name says
/// it is, when the videos beside it are named alike with other numbers, e.g.
/// Zork-Glorp's `[Zork-Glorp] Suzumiya Haruhi no Gensou [01][Wump_1080p]`
/// and `[02]`: a concert's two halves. Like any part, a knowledge base says
/// whether it's a movie's, matching the parts' runtime together, or a series'
/// episode.
pub(super) fn parts_in_tags(
    view: HeuristicView,
    mut adder: AddWithRank,
    series_patterns: &SeriesPatterns,
) {
    let number_tag = Regex::new(PART_TAG_REGEX).unwrap();
    // The numbers of the videos in each directory named alike but for them,
    // by the name with the number taken out.
    let mut sets = HashMap::<(&Path, String), Vec<(SubjectIndex, u32)>>::new();
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        let path = &path_id.rel_path;
        if !facts.has_video_streams() || series_patterns.named_series_file(path) {
            continue;
        }
        let (Some(dir), Some(stem)) = (path.parent(), file_stem(path)) else {
            continue;
        };
        let mut tags = number_tag.captures_iter(stem);
        let (Some(tag), None) = (tags.next(), tags.next()) else {
            continue;
        };
        let (Some(whole), Ok(number)) = (tag.get(0), tag[1].parse::<u32>()) else {
            continue;
        };
        // A special's number, e.g. `[SP][01]` or `[OVA][01]`.
        if series_patterns.ends_with_kind_tag(&stem[..whole.start()]) {
            continue;
        }
        let name = format!("{}[]{}", &stem[..whole.start()], &stem[whole.end()..]);
        sets.entry((dir, name)).or_default().push((subject, number));
    }
    for members in sets.into_values() {
        let numbers: BTreeSet<u32> = members.iter().map(|&(_, number)| number).collect();
        if numbers.len() < 2 {
            continue;
        }
        for (subject, number) in members {
            if let Some(mut adder) = adder.with_subject(subject) {
                adder.add_claim(ClaimPredicate::Part(number));
            }
        }
    }
}

/// Claims the part number a video's name gives it, e.g. 2 for
/// `The.Sorrow.and.the.Pity.1969.Part.Two`. The name alone can't tell a
/// sequel, e.g. `Dune.Part.Two`, from half a movie or an episode, so a
/// knowledge base decides which it is.
pub(super) fn part_from_file_name(
    part_re: &Regex,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if !facts.has_video_streams() {
        return None;
    }
    let c = part_re.captures(file_stem(&path_id.rel_path)?)?;
    let part = c.name("part").or_else(|| c.name("dvd"))?.as_str();
    let number = part
        .parse()
        .ok()
        .or_else(|| part_word(part))
        .or_else(|| roman_numeral(part))?;
    adder.add_claim(ClaimPredicate::Part(number));
    None
}

/// Claims the title of a video named as a part of its work before its part,
/// e.g. `taskmaster champion of champions` for
/// `Taskmaster.Champion.Of.Champions.Part.1.Wiley.Giraffe.Blower.1080p`,
/// which is how knowledge bases title the work. It's a second title, like
/// [`movie_title_without_numbering`](super::movie_title::movie_title_without_numbering)'s: both are looked up, and the better
/// match wins.
pub(super) fn movie_title_before_part(
    movie_title_re: &Regex,
    part_re: &Regex,
    path_id: &PathId,
    facts: &FileFacts,
    mut adder: AddToSubject,
) -> Done {
    if !facts.has_video_streams() {
        return None;
    }
    let stem = file_stem(&path_id.rel_path)?;
    let part = part_re.find(stem)?;
    let ParsedMovieTitle { title, .. } = parse_movie_title(movie_title_re, &stem[..part.start()])?;
    adder.add_claim(ClaimPredicate::MovieTitle(title));
    None
}

/// The value of a number from one to ten written as a word, e.g. `Two`.
fn part_word(word: &str) -> Option<u32> {
    let words = [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    let index = words.iter().position(|w| w.eq_ignore_ascii_case(word))?;
    Some(index as u32 + 1)
}

/// A part of a work in a file name, e.g. `Part Two`, `Part 2`, `Pt. 2` or
/// `Part II`, or a disc of a work ripped disc by disc, e.g. `CD1`, `Disc 2`
/// or `DVD1`, as media servers name the parts of a movie too. Not a DVD's
/// size, e.g. `DVD5` or `DVD9`, which a release's name says of its source.
pub(super) const PART_REGEX: &str = r"(?i)\b(?:(?:part|pt|cd|dis[ck])\.?[\s._-]*(?<part>\d{1,2}|one|two|three|four|five|six|seven|eight|nine|ten|(?-i:I|II|III|IV|V|VI|VII|VIII|IX|X))|dvd[\s._-]*(?<dvd>[1-4]|[6-8]))\b";

/// A tag that's a number of one to three digits, e.g. `[01]`, but not
/// `[1080p]` or a CRC, e.g. `[C0A72F96]`.
pub(super) const PART_TAG_REGEX: &str = r"\[(\d{1,3})\]";
