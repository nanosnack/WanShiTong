//! The words releases add to their files' names, in the sets that tell them
//! apart from titles' words in different places. They overlap, and differ
//! on purpose: what's cut from a title isn't what's dropped after an
//! episode's number, and a word a title may have too is a release's only
//! beside others.

use regex::Regex;
use std::sync::LazyLock;

/// A word that's a release's rather than its title's: a resolution, a
/// source, a streaming service, a codec, a video or audio format, or a flag,
/// e.g. `1080p`, `USBD`, `Remux`, `AMZN`, `x265`, `HDR10+`, `DDP5.1`, `AAC`,
/// `tv-dx50` or `REPACK2`. Not `web` alone, a title's word in `Charlotte's
/// Web`. Words that are titles' too, e.g. `Max` in `Mad Max` or `Hybrid`, are
/// a release's only in uppercase, as releases write them, e.g. `MAX` or `DV`.
pub(super) const NOISE_REGEX: &str = r"(?:^|[\s._({\-])(?:(?i:\d{3,4}[pi]|4k|uhd|blu-?ray|bd-?rip|br-?rip|bd-?remux|(?:us|uk|jp|eu|de|fr)bd|remux|web-?dl|web-?rip|hdtv(?:-?rip)?|hd-?rip|dvd-?rip|dvd[59r]|x26[45]|h\.?26[45]|hevc|(?:tv-)?(?:xvid|divx|dx50)|aac|ac-?3|dts|flac|10-?bit|amzn|dsnp|hmax|atvp|pcok|hulu|atmos|truehd|dd(?:p|\+)?[\s.]?[257]\.[01]|ddp|e-?ac-?3|dts-?hd|dts-?x|l?pcm|hdr10(?:\+|plus)?|hdr|dovi|sdr|repack\d*|proper|rerip|dual[\s._-]?audio)|NF|CR|DV|MAX|HYBRID|DUAL|MULTI|INTERNAL)(?:$|[\s._)}\]\-])";

/// A word that's a release's after its title, like those of [`NOISE_REGEX`],
/// though a title's tag may have it too, so a tag with it isn't the release's: a frame size, a bit depth,
/// a fansub's mark of quality, or its subtitles' language, e.g. `1280x720`,
/// `8 bits`, `Hi10P`, `FHD`, `BD`, `VOSTFR`, `VF` or `Sub ITA`, but also `16bit`
/// of `[16bit Sensation]` and `BD` of `[Show BD-BOX]`.
pub(super) const AFTER_TITLE_REGEX: &str = r"(?:^|[\s._({\-])(?:(?i:\d{1,2}-?bits?|(?:hi|ma)10p?|fhd|bd|\d{3,4}x\d{3,4}|vostfr|sub[\s-]?(?:ita|esp|eng)|eng[\s-]?subs?)|VF)(?:$|[\s._)}\]\-])";

/// Words a release adds after its title, which titles have too, so that
/// they're a release's only against its other words: editions, collections
/// and languages. See [`without_weak`].
pub(super) const WEAK: &[&str] = &[
    "unrated",
    "uncut",
    "uncensored",
    "censored",
    "extended",
    "remastered",
    "restored",
    "theatrical",
    "imax",
    "3d",
    "edition",
    "version",
    "premiere",
    "anniversary",
    "collection",
    "signature",
    "diamond",
    "complete",
    "dub",
    "dubbed",
    "subbed",
    "spanish",
    "castellano",
    "latino",
    "german",
    "deutsch",
    "french",
    "italian",
    "japanese",
    "korean",
    "hindi",
    "tamil",
    "telugu",
    "russian",
    "portuguese",
];

/// A title, or a series' name, without the words at its end that a release
/// adds to it, where it ends against a release's words or year, or an
/// episode's number: an edition, e.g. `unrated` of
/// `Beerfest.Unrated.2006.1080p` or `remastered` of `Detective Conan
/// Remastered - 045`, or `signature collection`, or a language it's dubbed
/// or subtitled in, e.g. `spanish` of `Yo.Soy.Sola.Spanish.1080p`, or
/// `english dub`. Not before then, as a title has such words too, e.g.
/// "Uncut Gems" or "The Spanish Prisoner", nor `english` but before a dub,
/// as of "The English Patient". Not the title's first word.
pub(super) fn without_weak(title: &str) -> String {
    let mut words: Vec<&str> = title.split_whitespace().collect();
    while words.len() > 1 {
        let Some(word) = words
            .last()
            .map(|word| word.trim_matches(['(', ')']).to_lowercase())
        else {
            break;
        };
        if !WEAK.contains(&word.as_str()) {
            break;
        }
        words.pop();
        if matches!(word.as_str(), "dub" | "dubbed" | "subbed")
            && words.len() > 1
            && words
                .last()
                .is_some_and(|word| word.eq_ignore_ascii_case("english"))
        {
            words.pop();
        }
    }
    words.join(" ")
}

/// A word that's surely a release's, of [`NOISE_REGEX`] or
/// [`AFTER_TITLE_REGEX`].
pub(super) static RELEASE_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("{NOISE_REGEX}|{AFTER_TITLE_REGEX}")).unwrap());

/// A name cut at the first word that's surely its release's, if a title is
/// left before it: or at the bracket it's in, e.g. at `(` in `Show -
/// 20180208 (THK 1280x720 x264)`, as what's with it there is the release's
/// too. Not at a word the title begins with, e.g. `8-bit` of `8-bit Wumbo`.
pub(super) fn cut(name: &str) -> String {
    let Some(word) = RELEASE_WORD
        .find_iter(name)
        .find(|word| name[..word.start()].chars().any(char::is_alphanumeric))
    else {
        return name.to_string();
    };
    let before = &name[..word.start()];
    let open = |open: char, close: char| {
        before
            .rfind(open)
            .filter(|&at| !before[at..].contains(close))
    };
    let cut = open('(', ')')
        .into_iter()
        .chain(open('{', '}'))
        .min()
        .unwrap_or(word.start());
    if name[..cut].chars().any(char::is_alphanumeric) {
        name[..cut].to_string()
    } else {
        name.to_string()
    }
}

/// A word a release adds after an episode's number, to drop from the end of
/// a title before splitting it into a series' name and an episode's number:
/// a quality, a source, a codec or a language, as above, but also the
/// fansubs' `end` or `fin` of a last episode, `uncensored`, or a dash. See
/// `series_and_episode`.
pub(super) const AFTER_EPISODE_REGEX: &str = r"(?i)^(?:hd|sd|fhd|uhd|hd\d{3,4}p?|\d{3,4}[pi]|x26[45]|h26[45]|hevc|avc|aac|flac|v\d|bluray|blu-ray|bdrip|bd|dvdrip|dvd|web|webrip|web-dl|webdl|tv|tvrip|hdtv|sub|subs|ita|esp|eng|vostfr|raw|end|fin|final|uncensored|-|–|~)$";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_loses_the_words_a_release_adds_at_its_end() {
        assert_eq!(without_weak("Zibble Lark Remastered"), "Zibble Lark");
        assert_eq!(without_weak("wumbo max english dub"), "wumbo max");
        assert_eq!(without_weak("glimmer spanish"), "glimmer");
        // Words a title has too, not at its end, or its only word.
        assert_eq!(without_weak("the english fnord"), "the english fnord");
        assert_eq!(without_weak("uncut fnord"), "uncut fnord");
        assert_eq!(without_weak("remastered"), "remastered");
        // English only before a dub, as titles end in it too.
        assert_eq!(without_weak("the fnord english"), "the fnord english");
    }
}
