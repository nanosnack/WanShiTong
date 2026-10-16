use serde::Deserialize;

/// A TMDB ID, given as is or as its page's URL, e.g. `71883` or
/// `https://www.themoviedb.org/movie/71883-redline` for `kind` `movie`, or
/// `https://www.themoviedb.org/tv/46298-hunter-x-hunter` for `kind` `tv`.
/// Another kind's URL is none, as TMDB numbers each kind apart.
pub fn parse_tmdb_id(text: &str, kind: &str) -> Option<u32> {
    let text = text.trim();
    let id = match text.split_once(&format!("/{kind}/")) {
        Some((_, rest)) => rest.split(|c: char| !c.is_ascii_digit()).next()?,
        None => text,
    };
    id.parse().ok()
}

/// The kinds of work TMDB numbers apart, as its URLs name them, and as a
/// form or query names which to search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TmdbKind {
    #[default]
    Movie,
    Tv,
}

/// A TMDB ID and its kind, given as its page's URL, whose kind it says, or
/// as is, taken to be of `kind`.
pub fn parse_tmdb_ref(text: &str, kind: TmdbKind) -> Option<(TmdbKind, u32)> {
    if let Some(id) = parse_tmdb_id(text, "movie").filter(|_| text.contains("/movie/")) {
        return Some((TmdbKind::Movie, id));
    }
    if let Some(id) = parse_tmdb_id(text, "tv").filter(|_| text.contains("/tv/")) {
        return Some((TmdbKind::Tv, id));
    }
    let as_is = match kind {
        TmdbKind::Movie => "movie",
        TmdbKind::Tv => "tv",
    };
    Some((kind, parse_tmdb_id(text, as_is)?))
}

#[cfg(test)]
mod tests {
    use super::parse_tmdb_id;
    use super::{TmdbKind, parse_tmdb_ref};

    #[test]
    fn a_urls_kind_overrides_the_one_taken_as_is() {
        let movie = TmdbKind::Movie;
        assert_eq!(
            parse_tmdb_ref("https://www.themoviedb.org/tv/63404-taskmaster", movie),
            Some((TmdbKind::Tv, 63404))
        );
        assert_eq!(parse_tmdb_ref(" 949 ", movie), Some((movie, 949)));
        assert_eq!(
            parse_tmdb_ref("949", TmdbKind::Tv),
            Some((TmdbKind::Tv, 949))
        );
        assert_eq!(parse_tmdb_ref("heat", movie), None);
    }

    #[test]
    fn parses_tmdb_ids_and_urls() {
        assert_eq!(parse_tmdb_id(" 71883 ", "movie"), Some(71883));
        assert_eq!(
            parse_tmdb_id("https://www.themoviedb.org/movie/71883-redline", "movie"),
            Some(71883)
        );
        assert_eq!(
            parse_tmdb_id(
                "https://www.themoviedb.org/movie/466000?language=ja",
                "movie"
            ),
            Some(466000)
        );
        assert_eq!(
            parse_tmdb_id("https://www.themoviedb.org/tv/128062", "movie"),
            None
        );
        assert_eq!(
            parse_tmdb_id("https://www.themoviedb.org/tv/46298-hunter-x-hunter", "tv"),
            Some(46298)
        );
        assert_eq!(
            parse_tmdb_id("https://www.themoviedb.org/movie/71883-redline", "tv"),
            None
        );
        assert_eq!(parse_tmdb_id("redline", "movie"), None);
        assert_eq!(parse_tmdb_id("", "movie"), None);
    }
}
