//! Editions named in file names and knowledge base titles, e.g. `Extended` or
//! `Director's Cut`: which expression of a work a file is (see *Expression*
//! in `spec/domain-model.md`).

use std::ops::Range;

/// An edition named in a name, and where.
#[derive(Debug, PartialEq)]
pub(crate) struct NamedEdition {
    /// E.g. `Director's Cut`, or `Unrated Extended` for several together.
    pub label: String,
    /// The bytes of the name that the edition's words span.
    pub span: Range<usize>,
}

/// The edition a file name says its file is: words like `Extended`,
/// `Unrated` or `Director's Cut` after its title, or several together, e.g.
/// `Unrated.Extended`. As a title can start with them, e.g. `Uncut Gems` or
/// `The Final Cut`, they're an edition only after the name's year, e.g.
/// `Brazil (1985) DC`, or just before it, after a title of two words or more,
/// e.g. `Alien.3.Assembly.Cut.1992`.
pub(crate) fn in_file_name(name: &str) -> Option<NamedEdition> {
    let words = words(name);
    let is_year = |word: &Word| {
        word.lower.len() == 4
            && word.lower.bytes().all(|b| b.is_ascii_digit())
            && matches!(&word.lower[..2], "19" | "20")
    };
    let mut start = 0;
    while start < words.len() {
        let Some(end) = run_end(&words, start) else {
            start += 1;
            continue;
        };
        let before = &words[..start];
        let after_title = match before.iter().rposition(is_year) {
            Some(year) => year > 0,
            None => before.len() >= 2 && words.get(end).is_some_and(is_year),
        };
        if after_title {
            return Some(named(&words, start, end));
        }
        start = end;
    }
    None
}

/// The title a knowledge base's title is an edition of, and the edition, when
/// it ends with one, e.g. `Kingdom Of Heaven` and `Director's Cut` for
/// `Kingdom Of Heaven (Director's Cut)`.
pub(crate) fn of_title(title: &str) -> Option<(&str, NamedEdition)> {
    let words = words(title);
    let start = (1..words.len()).find(|&start| run_end(&words, start) == Some(words.len()))?;
    // `The` before an edition is the edition's, e.g. `The Ultimate Cut`.
    let base_end = match start {
        2.. if words[start - 1].lower == "the" => start - 1,
        _ => start,
    };
    let base = title[..words[base_end].span.start]
        .trim_end_matches(|c: char| !c.is_alphanumeric() && !matches!(c, '\'' | '’'));
    Some((base, named(&words, start, words.len())))
}

/// Where the editions that start at the word end, e.g. after `DC Roadshow
/// Version`'s last word: those it names one after another.
fn run_end(words: &[Word], start: usize) -> Option<usize> {
    let mut end = start + phrase(&words[start..])?;
    while let Some(len) = phrase(&words[end..]) {
        end += len;
    }
    Some(end)
}

fn named(words: &[Word], start: usize, end: usize) -> NamedEdition {
    let mut labels = Vec::new();
    let mut at = start;
    while at < end {
        let len = phrase(&words[at..]).expect("a run is of editions");
        labels.push(label(&words[at..]).expect("a run is of editions"));
        at += len;
    }
    NamedEdition {
        label: labels.join(" "),
        span: words[start].span.start..words[end - 1].span.end,
    }
}

/// The length in words of the edition that begins at the first word, e.g. 2
/// for `Extended Cut`, or 1 for `DC` in `DC Roadshow Version`, where
/// `Roadshow Version` is a second edition.
fn phrase(words: &[Word]) -> Option<usize> {
    let (len, _) = edition(words)?;
    // A word for what it is, e.g. `Cut` in `Extended Cut`, is part of it.
    let kind = match words.get(len).map(|word| word.lower.as_str()) {
        Some("edition" | "version") => true,
        Some("cut") => len == 1,
        _ => false,
    };
    Some(if kind { len + 1 } else { len })
}

fn label(words: &[Word]) -> Option<&'static str> {
    edition(words).map(|(_, label)| label)
}

/// The edition the first of the words names, and how many of them it takes
/// to, without a word for what it is.
fn edition(words: &[Word]) -> Option<(usize, &'static str)> {
    let first = words.first()?;
    let second = words.get(1).map(|word| word.lower.as_str());
    Some(match (first.lower.as_str(), second) {
        // Uppercase only, as `dc` is a word in titles, e.g. `Washington DC`.
        _ if first.text == "DC" => (1, "Director's Cut"),
        ("directors", Some("cut")) => (2, "Director's Cut"),
        ("assembly", Some("cut")) => (2, "Assembly Cut"),
        ("final", Some("cut")) => (2, "Final Cut"),
        ("ultimate", Some("cut")) => (2, "Ultimate Cut"),
        ("ultimate", Some("edition")) => (2, "Ultimate Edition"),
        ("extended", _) => (1, "Extended"),
        ("theatrical", _) => (1, "Theatrical"),
        ("unrated", _) => (1, "Unrated"),
        ("uncut", _) => (1, "Uncut"),
        ("roadshow", _) => (1, "Roadshow"),
        ("imax", _) => (1, "IMAX"),
        _ => return None,
    })
}

/// A word in a name, e.g. `Director's` in `The.Iron.Giant.1999.Director's.Cut`.
struct Word<'a> {
    text: &'a str,
    /// Lowercased, without apostrophes, e.g. `directors`.
    lower: String,
    span: Range<usize>,
}

/// A name's words: what's between its separators and brackets.
fn words(name: &str) -> Vec<Word<'_>> {
    let mut words = Vec::new();
    let mut start = None;
    for (i, c) in name.char_indices().chain([(name.len(), ' ')]) {
        let in_word = c.is_alphanumeric() || matches!(c, '\'' | '’');
        match (start, in_word) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                let text = &name[s..i];
                let lower = text
                    .chars()
                    .filter(|c| !matches!(c, '\'' | '’'))
                    .collect::<String>()
                    .to_lowercase();
                words.push(Word {
                    text,
                    lower,
                    span: s..i,
                });
                start = None;
            }
            _ => (),
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_knowledge_base_title_can_be_an_edition_of_another() {
        let of = |title| of_title(title).map(|(base, edition)| (base, edition.label));
        assert_eq!(
            of("Kingdom Of Heaven (Director's Cut)"),
            Some(("Kingdom Of Heaven", "Director's Cut".into()))
        );
        assert_eq!(
            of("Watchmen - The Ultimate Cut"),
            Some(("Watchmen", "Ultimate Cut".into())),
        );
        assert_eq!(
            of("Apocalypse Now: Final Cut"),
            Some(("Apocalypse Now", "Final Cut".into())),
        );
        assert_eq!(of("Kingdom of Heaven"), None);
        // Edition words that are the whole title are of no other.
        assert_eq!(of("Uncut"), None);
        assert_eq!(of("The Final Cut"), Some(("The", "Final Cut".into())));
    }
}
