//! The keys movies and TV series are listed in order of.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// How many digits a number is padded to, so numbers sort by their values.
const NUMBER_WIDTH: usize = 10;

/// The key `title` is listed in order of: its words, in lowercase, without
/// accents, punctuation or a leading English article. So "The Matrix" sorts
/// among the Ms, "'Round Midnight" among the Rs, "mother!" among the Ms rather
/// than after every capitalized title, and "Léon" as if it were "Leon".
/// Numbers sort by their values, so "5 Centimeters per Second" comes before
/// "12 Angry Men". Titles are English until we support localization.
pub(crate) fn sort_key(title: &str) -> String {
    // Decomposing compatibility characters separates accents from their
    // letters, and spells out characters like "³" as "3".
    let folded: String = title
        .nfkd()
        .filter(|&c| !is_combining_mark(c))
        .flat_map(char::to_lowercase)
        .collect();
    // An apostrophe is part of its word, so "Schindler's" sorts as
    // "Schindlers", but other punctuation separates words, so "WALL·E" sorts
    // as "WALL E". Words are found before punctuation is dropped, so the "A" of
    // "A.I." isn't taken for an article.
    let mut words: Vec<String> = folded
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|&c| !matches!(c, '\'' | '‘' | '’' | 'ʼ'))
                .map(|c| if c.is_alphanumeric() { c } else { ' ' })
                .collect()
        })
        .filter(|word: &String| !word.trim().is_empty())
        .collect();
    // A title that's only an article has nothing else to sort by.
    if words.len() > 1 && matches!(words[0].as_str(), "the" | "a" | "an") {
        words.remove(0);
    }
    let words: Vec<&str> = words
        .iter()
        .flat_map(|word| word.split_whitespace())
        .collect();
    pad_numbers(&words.join(" "))
}

/// `key` with every number padded to `NUMBER_WIDTH` digits.
fn pad_numbers(key: &str) -> String {
    let mut padded = String::with_capacity(key.len());
    let mut rest = key;
    while let Some(start) = rest.find(|c: char| c.is_ascii_digit()) {
        padded.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        let number = rest[..end].trim_start_matches('0');
        padded.push_str(&format!("{number:0>NUMBER_WIDTH$}"));
        rest = &rest[end..];
    }
    padded.push_str(rest);
    padded
}
