//! Comparing titles, e.g. a file's with a knowledge base's.
//!
//! Titles are compared as sets of words, which is robust to what differs
//! between a file name and a knowledge base's title: case, punctuation, word
//! order and extra words, e.g. `code geass - lelouch of the rebellion -
//! initiation` and `Code Geass: Lelouch of the Rebellion I - Initiation`.

use std::collections::BTreeSet;

/// A title's words, lowercased and without punctuation, so that e.g.
/// "'Round Midnight" and "round midnight" have the same words.
pub(crate) fn words(title: &str) -> BTreeSet<String> {
    word_list(title).into_iter().collect()
}

/// A title's words in order, as [`words`] finds them.
pub(crate) fn word_list(title: &str) -> Vec<String> {
    title
        .chars()
        .filter(|c| !matches!(c, '\'' | '’'))
        .collect::<String>()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_lowercase())
        .collect()
}

/// Whether two titles have the same words in the same order, e.g. "Whole Cake
/// Island" and `whole cake island`.
pub(crate) fn same_words(a: &str, b: &str) -> bool {
    word_list(a) == word_list(b)
}

/// A title lowercased, with its long vowels written short, as a romanization
/// of a Japanese title may write them doubled, with a macron or without, e.g.
/// `shinryojo` for `Shinryoujo`, `Shinryōjo` and `Shinryojo`, and a knowledge
/// base may list it another way. For a title, it's another spelling: in
/// English, `Good` and `God` fold alike, so it's only for comparing a title
/// with one found by a romanized title's search.
pub(crate) fn romaji_folded(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'ā' | 'â' => 'a',
            'ē' | 'ê' => 'e',
            'ī' | 'î' => 'i',
            'ō' | 'ô' => 'o',
            'ū' | 'û' => 'u',
            c => c,
        })
        .collect::<String>()
        .replace("ou", "o")
        .replace("uu", "u")
        .replace("oo", "o")
}

/// Whether a title writes a long vowel doubled, e.g. `ou` of `Shinryoujo`,
/// which [`romaji_folded`] writes short.
pub(crate) fn has_doubled_vowels(title: &str) -> bool {
    let title = title.to_lowercase();
    ["ou", "uu", "oo"]
        .iter()
        .any(|vowels| title.contains(vowels))
}

/// Whether a title begins with the words of what was searched for: is it, or
/// is it and more, e.g. a subtitle. "After Life" and "Food Wars! Shokugeki no
/// Soma" do for "after life" and "food wars", but "Life After Life" doesn't.
pub(crate) fn begins_with(title: &str, query: &str) -> bool {
    let (title, query) = (word_list(title), word_list(query));
    !query.is_empty() && title.starts_with(&query)
}

/// Whether a title is what was searched for, or that and a subtitle that
/// punctuation sets apart: "Food Wars! Shokugeki no Soma" and "Alone" are
/// for "food wars" and "alone", but "Alone Together", which goes on to
/// another name, isn't.
pub(crate) fn is_or_has_subtitle(title: &str, query: &str) -> bool {
    let query = word_list(query);
    if query.is_empty() {
        return false;
    }
    let is_word = |c: char| c.is_alphanumeric() || matches!(c, '\'' | '’');
    let mut rest = title;
    for word in &query {
        rest = rest.trim_start_matches(|c: char| !is_word(c));
        let end = rest.find(|c: char| !is_word(c)).unwrap_or(rest.len());
        let found: String = rest[..end]
            .chars()
            .filter(|c| !matches!(c, '\'' | '’'))
            .collect::<String>()
            .to_lowercase();
        if found != *word {
            return false;
        }
        rest = &rest[end..];
    }
    let rest = rest.trim_start();
    rest.is_empty() || rest.starts_with([':', '!', '?', '-', '–', '—', '(', '~', '.', ','])
}

/// Whether two titles are the same but for a near spelling: the same words,
/// without spaces, punctuation and articles, but for one letter, or two in
/// a long title, e.g. "The Alto Knights" and `the alto nights`, or the same
/// letters spaced otherwise, e.g. "Star Cash" and `starcash`. As the same
/// words: not "Zero Dark Thirty" for `zero`, and a title of one word only
/// spaced otherwise, never spelled otherwise, as one word names too little.
pub(crate) fn spelled_nearly(a: &str, b: &str) -> bool {
    let content = |title: &str| -> Vec<String> {
        word_list(title)
            .into_iter()
            .filter(|word| !matches!(word.as_str(), "the" | "a" | "an"))
            .collect()
    };
    let (a_words, b_words) = (content(a), content(b));
    if a_words.is_empty() || b_words.is_empty() {
        return false;
    }
    let (a_letters, b_letters) = (a_words.concat(), b_words.concat());
    if a_letters == b_letters {
        return true;
    }
    if a_words.len() < 2 || a_words.len() != b_words.len() {
        return false;
    }
    let allowed = if a_letters.chars().count() >= 16 {
        2
    } else {
        1
    };
    edit_distance(&a_letters, &b_letters) <= allowed
}

/// How many letters must be inserted, deleted or replaced to turn one string
/// into the other.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut current = vec![i + 1];
        for (j, &cb) in b.iter().enumerate() {
            let replace = previous[j] + usize::from(ca != cb);
            current.push(replace.min(previous[j + 1] + 1).min(current[j] + 1));
        }
        previous = current;
    }
    previous[b.len()]
}

/// Whether a title has all the words of another in their order, with others
/// between them or around them, e.g. `Juvenile Division-DR-19` has those of
/// "Juvenile: DR-19". Only for a title of several words: one word is too
/// little to say which.
pub(crate) fn has_words_in_order(title: &str, other: &str) -> bool {
    let (title, other) = (word_list(title), word_list(other));
    if other.len() < 2 {
        return false;
    }
    let mut rest = title.iter();
    other
        .iter()
        .all(|word| rest.by_ref().any(|title_word| title_word == word))
}

/// Whether a title begins with another's first word and has the rest of its
/// words in order, with others between, e.g. "Mermaid Man and Barnacle Boy
/// V" does for `Mermaid and Barnacle Boy V`, but "Homicide: Who Killed Who"
/// doesn't for `Who Killed Who`, which is only its subtitle. Only for several
/// words, as [`has_words_in_order`].
pub(crate) fn has_words_in_order_from_the_start(title: &str, other: &str) -> bool {
    let (title, other) = (word_list(title), word_list(other));
    !title.is_empty() && title.first() == other.first() && {
        let mut rest = title.iter();
        other.len() >= 2
            && other
                .iter()
                .all(|word| rest.by_ref().any(|title_word| title_word == word))
    }
}

/// Whether a title has the words of another together, in their order, e.g.
/// `Juvenile Division-DR-19` has those of "DR-19", but not of "Juvenile 19".
pub(crate) fn has_words_in_a_row(title: &str, other: &str) -> bool {
    let (title, other) = (word_list(title), word_list(other));
    !other.is_empty() && title.windows(other.len()).any(|window| window == other)
}

/// How alike two titles are, from 0 (no words in common) to 1 (the same
/// words): the Sørensen–Dice coefficient of their words.
pub(crate) fn similarity(a: &str, b: &str) -> f32 {
    let (a, b) = (words(a), words(b));
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let common = a.intersection(&b).count();
    2.0 * common as f32 / (a.len() + b.len()) as f32
}

/// How much of a title's words a query has, from 0 (none) to 1 (all), e.g. 1
/// for `tokyo godfathers usbd remux` and "Tokyo Godfathers". Numbers are
/// compared by value, so `01` has "1".
pub(crate) fn coverage(query: &str, title: &str) -> f32 {
    let (query, title) = (
        numbers_by_value(words(query)),
        numbers_by_value(words(title)),
    );
    if title.is_empty() {
        return 0.0;
    }
    title.intersection(&query).count() as f32 / title.len() as f32
}

/// Whether a title has a word that isn't a number, e.g. not `01`, so that it
/// says something of what it's the title of.
pub(crate) fn has_words(title: &str) -> bool {
    words(title)
        .iter()
        .any(|word| !word.chars().all(|c| c.is_ascii_digit()))
}

/// Words, with numbers without their leading zeros, e.g. `1` for `01`.
fn numbers_by_value(words: BTreeSet<String>) -> BTreeSet<String> {
    words
        .into_iter()
        .map(|word| match word.trim_start_matches('0') {
            _ if !word.chars().all(|c| c.is_ascii_digit()) => word,
            "" => "0".into(),
            number => number.into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_near_spelling_is_the_same_words_but_for_a_letter_or_two() {
        assert!(spelled_nearly("The Zibble Knights", "the zibble nights"));
        assert!(spelled_nearly("Wumbo Lark", "wumbo larc"));
        assert!(spelled_nearly("Glim Peak", "glimpeak"));
        assert!(spelled_nearly(
            "Yonder on 7th Road",
            "the yonder on 7th road"
        ));
        assert!(spelled_nearly(
            "A Distant Glimmer: Wumbos on Zibble Day",
            "a distant glimmer wumbos of zibble day"
        ));
        // Other words, not other spellings.
        assert!(!spelled_nearly("Zibble Dark Night", "zibble"));
        assert!(!spelled_nearly("Wumbo Together", "wumbo"));
        // One word is too little to spell otherwise.
        assert!(!spelled_nearly("Zibbles", "zibble"));
        // Too far apart.
        assert!(!spelled_nearly("Penny Dreadful", "peni dredful"));
    }

    #[test]
    fn a_subtitle_is_set_apart_by_punctuation() {
        assert!(is_or_has_subtitle("Zibble", "zibble"));
        assert!(is_or_has_subtitle("Zibble! Glorp no Fnord", "zibble"));
        assert!(is_or_has_subtitle("Zibble: The Road", "zibble"));
        assert!(is_or_has_subtitle("Wumbo's Lark (2009)", "wumbos lark"));
        // Words that go on to another name.
        assert!(!is_or_has_subtitle("Zibble Together", "zibble"));
        assert!(!is_or_has_subtitle("The Zibble", "zibble"));
        assert!(!is_or_has_subtitle("Zibble", ""));
    }

    #[test]
    fn titles_with_the_same_words_are_the_same() {
        assert_eq!(similarity("playtime", "PlayTime"), 1.0);
        assert_eq!(similarity("round midnight", "'Round Midnight"), 1.0);
        assert_eq!(similarity("グリム童話 金の鳥", "グリム童話　金の鳥"), 1.0);
        assert_eq!(similarity("heat", "The Help"), 0.0);
        assert_eq!(similarity("", ""), 0.0);
    }

    #[test]
    fn a_query_covers_the_words_of_the_title_it_has() {
        assert_eq!(
            coverage("tokyo godfathers usbd remux", "Tokyo Godfathers"),
            1.0
        );
        assert_eq!(
            coverage(
                "code geass - lelouch of the rebellion - initiation",
                "Code Geass: Lelouch of the Rebellion I - Initiation"
            ),
            7.0 / 8.0
        );
        // Numbers by value.
        assert_eq!(
            coverage(
                "code geass - akito the exiled - 03",
                "Code Geass: Akito the Exiled 3: The Brightness Falls"
            ),
            6.0 / 8.0
        );
        assert_eq!(coverage("01", "Ash 01"), 0.5);
        assert_eq!(coverage("heat", ""), 0.0);

        assert!(has_words("code geass"));
        assert!(!has_words("01"));
        assert!(!has_words("300"));
    }

    #[test]
    fn a_title_has_anothers_words_in_order_or_in_a_row() {
        assert!(has_words_in_order(
            "Juvenile Division-DR-19",
            "Juvenile: DR-19"
        ));
        assert!(has_words_in_order("The Big Kid DR-19", "big kid"));
        assert!(!has_words_in_order(
            "Juvenile Division-DR-19",
            "DR-19 Juvenile"
        ));
        // One word says too little.
        assert!(!has_words_in_order("Pilot Error-DR-5", "Pilot"));

        assert!(has_words_in_order_from_the_start(
            "Mermaid Man and Barnacle Boy V",
            "Mermaid and Barnacle Boy V"
        ));
        assert!(!has_words_in_order_from_the_start(
            "Homicide: Who Killed Who",
            "Who Killed Who"
        ));
        assert!(!has_words_in_order_from_the_start("Pilot Error", "Pilot"));

        assert!(has_words_in_a_row("Juvenile Division-DR-19", "DR-19"));
        assert!(has_words_in_a_row("4F19 The Springfield Files", "4f19"));
        assert!(!has_words_in_a_row(
            "Juvenile Division-DR-19",
            "Juvenile 19"
        ));
        assert!(!has_words_in_a_row("Juvenile Division-DR-19", ""));
    }

    #[test]
    fn the_title_with_more_words_in_common_is_more_alike() {
        let query = "code geass - lelouch of the rebellion - initiation";
        let initiation = similarity(query, "Code Geass: Lelouch of the Rebellion I - Initiation");
        let transgression = similarity(
            query,
            "Code Geass: Lelouch of the Rebellion II - Transgression",
        );
        assert!(initiation > transgression, "{initiation} > {transgression}");
        assert!(similarity("playtime", "PlayTime") > similarity("playtime", "Playtime Towers"));
    }
}
