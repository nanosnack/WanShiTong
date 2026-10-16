//! NFO files: a release's, e.g. with its IMDb link in its ANSI art, or a
//! media server's or its manager's, e.g. Kodi's, Jellyfin's or
//! tinyMediaManager's XML, beside a video or at its series' root. Ingest
//! reads one's text as a fact (see [`FileFacts::nfo_text`]); here it's read
//! for the work it names.
//!
//! A media server's NFO is one of three: a `<movie>`, named after the video
//! or `movie.nfo` beside it; an `<episodedetails>`, named after the episode;
//! or a `<tvshow>`, `tvshow.nfo` at the series' root, above its seasons. A
//! release's NFO is text, named after the release, with a link to the work.
//! Either may be stale, from a file renamed since, so its claims are ranked
//! below an assertion, and its IDs are looked up, not believed: a knowledge
//! base finding nothing by one leaves the file to its other claims.
//!
//! The XML isn't parsed as XML, as it often isn't: a file of several
//! `<episodedetails>` has several roots, and one with a link after its root
//! has text outside it. Its elements are read where they are.

use super::{
    AddToClaims, HeuristicView, extras::in_extras_dir, names::file_stem,
    series_patterns::SeriesPatterns,
};
use regex::Regex;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use wanshitong_core::domain::{
    ClaimPredicate, DataSourceUuid, ExternalId, FileFacts, FileRole, Heuristic, PathId,
    RankedClaimPredicate, WorkKind,
};

/// Claims what the NFOs say of each video: its own NFO's, beside it, and
/// its series', at the root above it.
///
/// Its own is the NFO named as it is, or failing that the only one in its
/// directory, if it's the only video there, so that one release's NFO
/// isn't taken for each movie of a pack. Though an episode's, in a season
/// release of many, is the series' for each of them. A disc's own is named
/// as its folder is, beside it, or in the folder, as Kodi puts a disc's
/// `movie.nfo`.
///
/// A `<movie>`'s IDs, title and year are a movie's, unless the file's an
/// episode by its name or its siblings: then the NFO is of something else.
/// An `<episodedetails>`'s season and episodes are the file's, with its
/// title and its series' name, and make the file an episode. A release's
/// link is its movie's, or its series' if the file's an episode. A
/// `tvshow.nfo`'s IDs, name and year are claimed only for episodes, as a
/// series' directory holds its movies and OVAs too. Not for an extra, which
/// is of its main file's work.
///
/// After the numbered runs and the samples, whose claims it goes by.
pub(super) fn nfo_heuristic(
    view: HeuristicView,
    adder: &mut AddToClaims,
    [id_rank, episode_rank, work_rank]: [f32; 3],
    patterns: &SeriesPatterns,
    nfo_patterns: &NfoPatterns,
) {
    let nfos = Nfos::new(view, nfo_patterns);
    let videos_by_dir = videos_by_dir(view, adder);
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        if adder.exclusions.contains(&subject) {
            continue;
        }
        let path = &path_id.rel_path;
        if !facts.is_video() || patterns.is_extra(path) || in_extras_dir(path) {
            continue;
        }
        let claims = adder.claims.get(&subject).map_or(&[][..], Vec::as_slice);
        if is_sample(claims) {
            continue;
        }
        let named_episode = patterns.is_episode(path)
            || claims.iter().any(|claim| {
                matches!(
                    claim.pred,
                    ClaimPredicate::EpisodeRange(_) | ClaimPredicate::AbsoluteEpisodeRange(_)
                )
            });
        let own = own_nfo(&nfos, &videos_by_dir, path_id, facts, named_episode);
        let is_episode = named_episode || own.is_some_and(|nfo| nfo.kind == NfoKind::Episode);
        let data_source = path_id.data_source;
        let mut add = |rank, pred| {
            if let Some(mut adder) = adder.with(Heuristic::Nfo, rank).with_subject(subject) {
                adder.add_claim(pred);
            }
        };
        if let Some(nfo) = own {
            match nfo.kind {
                NfoKind::Episode => {
                    if let (Some(season), Some(episodes)) = (nfo.season, nfo.episodes) {
                        add(episode_rank, ClaimPredicate::Season(season));
                        add(episode_rank, ClaimPredicate::EpisodeRange(episodes));
                    }
                    if let Some(title) = &nfo.title {
                        add(work_rank, ClaimPredicate::EpisodeTitle(title.clone()));
                    }
                    if let Some(show) = &nfo.show_title {
                        add(work_rank, ClaimPredicate::TvSeriesName(show.clone()));
                    }
                    add(work_rank, ClaimPredicate::WorkKind(WorkKind::TvSeries));
                }
                NfoKind::Movie if !is_episode => {
                    for id in &nfo.ids {
                        add(id_rank, ClaimPredicate::MovieExternalId(id.clone()));
                    }
                    if let Some(title) = &nfo.title {
                        add(work_rank, ClaimPredicate::MovieTitle(title.clone()));
                    }
                    if let Some(year) = nfo.year {
                        add(work_rank, ClaimPredicate::MovieReleaseYear(year));
                    }
                    add(work_rank, ClaimPredicate::WorkKind(WorkKind::Movie));
                }
                NfoKind::Movie | NfoKind::TvShow => (),
                NfoKind::Release => {
                    for id in &nfo.ids {
                        let pred = if is_episode {
                            ClaimPredicate::TvSeriesExternalId(id.clone())
                        } else {
                            ClaimPredicate::MovieExternalId(id.clone())
                        };
                        add(id_rank, pred);
                    }
                }
            }
        }
        if !is_episode {
            continue;
        }
        let dir = video_dir(path_id, facts);
        if let Some(nfo) = nfos.series_nfo(data_source, &dir) {
            for id in &nfo.ids {
                add(id_rank, ClaimPredicate::TvSeriesExternalId(id.clone()));
            }
            if let Some(title) = &nfo.title {
                add(work_rank, ClaimPredicate::TvSeriesName(title.clone()));
            }
            if let Some(year) = nfo.year {
                add(work_rank, ClaimPredicate::TvSeriesYear(year));
            }
            add(work_rank, ClaimPredicate::WorkKind(WorkKind::TvSeries));
        }
    }
}

/// The directory a video is in: a disc's, the one its folder is in.
fn video_dir(path_id: &PathId, facts: &FileFacts) -> PathBuf {
    let dir = match &facts.disc {
        Some(disc) => Path::new(&disc.root).parent(),
        None => path_id.rel_path.parent(),
    };
    dir.map(Path::to_path_buf).unwrap_or_default()
}

/// A video's own NFO, as [`nfo_heuristic`] finds it, if it has one.
/// `named_episode` is whether the video's an episode by its name or its
/// siblings, which shares its release's NFO with the rest.
fn own_nfo<'a>(
    nfos: &'a Nfos,
    videos_by_dir: &HashMap<(DataSourceUuid, PathBuf), usize>,
    path_id: &PathId,
    facts: &FileFacts,
    named_episode: bool,
) -> Option<&'a Nfo> {
    let data_source = path_id.data_source;
    let dir = video_dir(path_id, facts);
    let alone = videos_by_dir.get(&(data_source, dir.clone())) == Some(&1);
    // A disc's names: its folder's, and the folder's without an image's
    // extension, e.g. `Movie` for `Movie.iso`.
    let names: Vec<&str> = match &facts.disc {
        Some(disc) => {
            let root = Path::new(&disc.root);
            [root.file_name(), root.file_stem()]
                .into_iter()
                .flatten()
                .filter_map(|name| name.to_str())
                .collect()
        }
        None => file_stem(&path_id.rel_path).into_iter().collect(),
    };
    let in_dir = nfos.in_dir(data_source, &dir);
    let named = in_dir
        .iter()
        .find(|(stem, _)| names.iter().any(|name| stem.eq_ignore_ascii_case(name)));
    if let Some((_, nfo)) = named {
        return Some(nfo);
    }
    // Kodi puts a disc's `movie.nfo` in its folder.
    if let Some(disc) = &facts.disc {
        let inside = nfos.in_dir(data_source, Path::new(&disc.root));
        if let [(_, nfo)] = inside
            && nfo.kind != NfoKind::TvShow
        {
            return Some(nfo);
        }
    }
    match in_dir {
        [(_, nfo)] if nfo.kind != NfoKind::TvShow && (alone || named_episode) => Some(nfo),
        _ => None,
    }
}

/// Whether a file's claims so far make it a sample.
fn is_sample(claims: &[RankedClaimPredicate]) -> bool {
    claims
        .iter()
        .any(|claim| claim.pred == ClaimPredicate::FileRole(FileRole::Sample))
}

/// How many videos each directory has, samples left out, so that a
/// directory's only NFO is known to be its only video's.
fn videos_by_dir(
    view: HeuristicView,
    adder: &AddToClaims,
) -> HashMap<(DataSourceUuid, PathBuf), usize> {
    let mut counts = HashMap::new();
    for (subject, (path_id, facts)) in view.files.iter().enumerate() {
        if !facts.is_video() {
            continue;
        }
        if adder.claims.get(&subject).is_some_and(|c| is_sample(c)) {
            continue;
        }
        *counts
            .entry((path_id.data_source, video_dir(path_id, facts)))
            .or_default() += 1;
    }
    counts
}

/// The NFOs of an interpretation's files, by directory.
struct Nfos {
    /// Each directory's NFOs, by their stems.
    by_dir: HashMap<(DataSourceUuid, PathBuf), Vec<(String, Nfo)>>,
}

impl Nfos {
    fn new(view: HeuristicView, patterns: &NfoPatterns) -> Self {
        let mut by_dir = HashMap::<_, Vec<_>>::new();
        for (path_id, facts) in view.files {
            let Some(text) = &facts.nfo_text else {
                continue;
            };
            let path = &path_id.rel_path;
            let Some(stem) = file_stem(path) else {
                continue;
            };
            let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
            by_dir
                .entry((path_id.data_source, dir))
                .or_default()
                .push((stem.to_owned(), patterns.parse(text)));
        }
        Self { by_dir }
    }

    fn in_dir(&self, data_source: DataSourceUuid, dir: &Path) -> &[(String, Nfo)] {
        self.by_dir
            .get(&(data_source, dir.to_path_buf()))
            .map_or(&[], Vec::as_slice)
    }

    /// The nearest `tvshow.nfo` in `dir` or above it, up to the data
    /// source's root.
    fn series_nfo(&self, data_source: DataSourceUuid, dir: &Path) -> Option<&Nfo> {
        dir.ancestors().find_map(|dir| {
            self.in_dir(data_source, dir)
                .iter()
                .find(|(stem, _)| stem.eq_ignore_ascii_case("tvshow"))
                .map(|(_, nfo)| nfo)
        })
    }
}

/// What kind of NFO a file is, by its root element, if it has one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NfoKind {
    /// Kodi's `<movie>`.
    Movie,
    /// Kodi's `<tvshow>`.
    TvShow,
    /// Kodi's `<episodedetails>`, one or several.
    Episode,
    /// A release's text, or anything else.
    Release,
}

/// What an NFO says of its work.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Nfo {
    pub(super) kind: NfoKind,
    /// Its work's IDs: its elements', and its links', e.g. a release's
    /// `imdb.com/title/tt0113277`. Not an episode's own.
    pub(super) ids: Vec<ExternalId>,
    /// Its work's, or an episode's.
    pub(super) title: Option<String>,
    /// Its work's, from `<year>` or `<premiered>`.
    pub(super) year: Option<u32>,
    /// An episode's season.
    pub(super) season: Option<u32>,
    /// An episode's first and last episode numbers, as a file of several
    /// episodes has an `<episodedetails>` each.
    pub(super) episodes: Option<[u32; 2]>,
    /// An episode's series' name, its `<showtitle>`.
    pub(super) show_title: Option<String>,
}

/// The patterns an NFO is read by, compiled once a run.
pub(super) struct NfoPatterns {
    root: Regex,
    episode_block: Regex,
    uniqueid: Regex,
    uniqueid_type: Regex,
    elements: HashMap<&'static str, Regex>,
    imdb_link: Regex,
    tmdb_link: Regex,
    imdb_id: Regex,
}

/// The elements read: a work's or an episode's.
const ELEMENTS: [&str; 11] = [
    "title",
    "year",
    "premiered",
    "imdbid",
    "tmdbid",
    "tvdbid",
    "id",
    "season",
    "episode",
    "showtitle",
    "uniqueid",
];

impl NfoPatterns {
    pub(super) fn new() -> Self {
        let elements = ELEMENTS
            .into_iter()
            .map(|name| {
                // The element and its text, e.g. `<title>Heat</title>`, but
                // not another's that begins alike, e.g. `<titlesort>`.
                let re =
                    Regex::new(&format!(r"(?is)<{name}(?:\s[^>]*)?>(.*?)</{name}\s*>")).unwrap();
                (name, re)
            })
            .collect();
        Self {
            root: Regex::new(r"(?i)<(movie|tvshow|episodedetails)\b").unwrap(),
            episode_block: Regex::new(r"(?is)<episodedetails\b[^>]*>(.*?)</episodedetails\s*>")
                .unwrap(),
            uniqueid: Regex::new(r"(?is)<uniqueid(\s[^>]*)?>(.*?)</uniqueid\s*>").unwrap(),
            uniqueid_type: Regex::new(r#"(?i)\btype\s*=\s*["']([^"']*)["']"#).unwrap(),
            elements,
            imdb_link: Regex::new(r"(?i)imdb\.com/title/(tt\d+)").unwrap(),
            tmdb_link: Regex::new(r"(?i)themoviedb\.org/(?:movie|tv)/(\d+)").unwrap(),
            imdb_id: Regex::new(r"(?i)^tt\d+$").unwrap(),
        }
    }

    /// What `text` says, as [`Nfo`] holds it.
    pub(super) fn parse(&self, text: &str) -> Nfo {
        let text = text.trim_start_matches('\u{feff}');
        let (kind, body) = match self.root.captures(text) {
            Some(root) => {
                let kind = match root[1].to_ascii_lowercase().as_str() {
                    "movie" => NfoKind::Movie,
                    "tvshow" => NfoKind::TvShow,
                    _ => NfoKind::Episode,
                };
                (kind, &text[root.get(0).unwrap().start()..])
            }
            None => (NfoKind::Release, text),
        };
        let mut nfo = Nfo {
            kind,
            ids: Vec::new(),
            title: None,
            year: None,
            season: None,
            episodes: None,
            show_title: None,
        };
        match kind {
            NfoKind::Movie | NfoKind::TvShow => {
                nfo.title = self.element(body, "title");
                nfo.year = self.year(body);
                nfo.ids = self.ids(body, kind);
            }
            NfoKind::Episode => self.episodes(body, &mut nfo),
            NfoKind::Release => (),
        }
        // Links anywhere, e.g. a release's, or after a `<movie>` as Kodi
        // reads one.
        if kind != NfoKind::Episode {
            for link in self.imdb_link.captures_iter(text) {
                push_id(&mut nfo.ids, ExternalId::Imdb(link[1].to_lowercase()));
            }
            for link in self.tmdb_link.captures_iter(text) {
                if let Ok(id) = link[1].parse() {
                    push_id(&mut nfo.ids, ExternalId::Tmdb(id));
                }
            }
        }
        nfo
    }

    /// The text of the first element named `name` in `body`, decoded, if
    /// it has any.
    fn element(&self, body: &str, name: &str) -> Option<String> {
        let text = decode(&self.elements[name].captures(body)?[1]);
        (!text.is_empty()).then_some(text)
    }

    fn number(&self, body: &str, name: &str) -> Option<u32> {
        self.element(body, name)?.parse().ok()
    }

    /// `<year>`, or `<premiered>`'s, e.g. 1995 of `1995-12-15`.
    fn year(&self, body: &str) -> Option<u32> {
        self.number(body, "year").or_else(|| {
            let premiered = self.element(body, "premiered")?;
            premiered.get(..4)?.parse().ok()
        })
    }

    /// The IDs a `<movie>` or `<tvshow>` gives: its `<uniqueid>`s, by their
    /// types, its `<imdbid>`, `<tmdbid>` and `<tvdbid>`, and Kodi's old
    /// `<id>`, an IMDb ID or, for a series, TVDB's.
    fn ids(&self, body: &str, kind: NfoKind) -> Vec<ExternalId> {
        let mut ids = Vec::new();
        for unique in self.uniqueid.captures_iter(body) {
            let kind = unique
                .get(1)
                .and_then(|attrs| self.uniqueid_type.captures(attrs.as_str()))
                .map(|t| t[1].to_ascii_lowercase());
            let value = decode(&unique[2]);
            if let Some(id) = self.id(kind.as_deref(), &value) {
                push_id(&mut ids, id);
            }
        }
        for (name, kind) in [("imdbid", "imdb"), ("tmdbid", "tmdb"), ("tvdbid", "tvdb")] {
            if let Some(value) = self.element(body, name)
                && let Some(id) = self.id(Some(kind), &value)
            {
                push_id(&mut ids, id);
            }
        }
        if let Some(value) = self.element(body, "id") {
            let legacy = if self.imdb_id.is_match(&value) {
                Some("imdb")
            } else if kind == NfoKind::TvShow {
                Some("tvdb")
            } else {
                None
            };
            if let Some(id) = self.id(legacy, &value) {
                push_id(&mut ids, id);
            }
        }
        ids
    }

    /// An ID of a knowledge base, by the type an NFO names it by.
    fn id(&self, kind: Option<&str>, value: &str) -> Option<ExternalId> {
        let value = value.trim();
        match kind? {
            "imdb" if self.imdb_id.is_match(value) => Some(ExternalId::Imdb(value.to_lowercase())),
            "tmdb" => value.parse().ok().map(ExternalId::Tmdb),
            "tvdb" => value.parse().ok().map(ExternalId::Tvdb),
            _ => None,
        }
    }

    /// What a file's `<episodedetails>` say, each of one episode in it. Of
    /// several, the first's title and show, and the run of their episodes,
    /// if they're of one season.
    fn episodes(&self, body: &str, nfo: &mut Nfo) {
        let mut blocks = self.episode_block.captures_iter(body).peekable();
        let Some(first) = blocks.peek() else {
            return;
        };
        let first = first[1].to_owned();
        nfo.title = self.element(&first, "title");
        nfo.show_title = self.element(&first, "showtitle");
        let mut season = None;
        let mut episodes: Option<[u32; 2]> = None;
        for block in blocks {
            let (Some(s), Some(e)) = (
                self.number(&block[1], "season"),
                self.number(&block[1], "episode"),
            ) else {
                continue;
            };
            if *season.get_or_insert(s) != s {
                return;
            }
            episodes = Some(match episodes {
                Some([first, last]) => [first.min(e), last.max(e)],
                None => [e, e],
            });
        }
        nfo.season = season;
        nfo.episodes = episodes;
    }
}

fn push_id(ids: &mut Vec<ExternalId>, id: ExternalId) {
    if !ids.contains(&id) {
        ids.push(id);
    }
}

/// An element's text as written: without its CDATA markers, its entities
/// decoded, and trimmed.
fn decode(text: &str) -> String {
    let text = text
        .trim()
        .strip_prefix("<![CDATA[")
        .and_then(|t| t.strip_suffix("]]>"))
        .unwrap_or(text);
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';').filter(|&end| end <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix('#')
                .and_then(|n| match n.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => n.parse().ok(),
                })
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kodi_movie_nfo_gives_its_ids_title_and_year() {
        let patterns = NfoPatterns::new();
        let nfo = patterns.parse(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>
<movie>
    <title>Alien&#179;</title>
    <originaltitle>Alien³</originaltitle>
    <sorttitle>Alien 3</sorttitle>
    <year>1992</year>
    <premiered>1992-05-22</premiered>
    <uniqueid type="imdb">tt0103644</uniqueid>
    <uniqueid type="tmdb" default="true">8077</uniqueid>
    <actor><name>Sigourney Weaver</name></actor>
</movie>
https://www.themoviedb.org/movie/8077-alien
"#,
        );
        assert_eq!(nfo.kind, NfoKind::Movie);
        assert_eq!(nfo.title.as_deref(), Some("Alien³"));
        assert_eq!(nfo.year, Some(1992));
        assert_eq!(
            nfo.ids,
            [ExternalId::Imdb("tt0103644".into()), ExternalId::Tmdb(8077)]
        );
    }

    #[test]
    fn older_and_other_managers_id_elements_are_read() {
        let patterns = NfoPatterns::new();
        let nfo = patterns.parse(
            "<movie><title><![CDATA[Heat &amp; Co.]]></title><premiered>1995-12-15</premiered><id>tt0113277</id><tmdbid>949</tmdbid></movie>",
        );
        assert_eq!(nfo.title.as_deref(), Some("Heat & Co."));
        assert_eq!(nfo.year, Some(1995));
        assert_eq!(
            nfo.ids,
            [ExternalId::Tmdb(949), ExternalId::Imdb("tt0113277".into())]
        );

        let show = patterns.parse(
            "<tvshow>\n<title>Berserk</title>\n<year>1997</year>\n<id>73752</id>\n<tvdbid>73752</tvdbid>\n</tvshow>",
        );
        assert_eq!(show.kind, NfoKind::TvShow);
        assert_eq!(show.title.as_deref(), Some("Berserk"));
        assert_eq!(show.year, Some(1997));
        assert_eq!(show.ids, [ExternalId::Tvdb(73752)]);
    }

    #[test]
    fn episode_details_give_the_episodes_and_their_series() {
        let patterns = NfoPatterns::new();
        let one = patterns.parse(
            "<episodedetails><title>Ozymandias</title><showtitle>Breaking Bad</showtitle><season>5</season><episode>14</episode><uniqueid type=\"tvdb\">4645978</uniqueid></episodedetails>",
        );
        assert_eq!(one.kind, NfoKind::Episode);
        assert_eq!(one.title.as_deref(), Some("Ozymandias"));
        assert_eq!(one.show_title.as_deref(), Some("Breaking Bad"));
        assert_eq!((one.season, one.episodes), (Some(5), Some([14, 14])));
        // An episode's own ID isn't its series'.
        assert!(one.ids.is_empty());

        // A file of two episodes: two roots.
        let two = patterns.parse(
            "<episodedetails><title>Pilot</title><season>1</season><episode>1</episode></episodedetails>\n<episodedetails><title>Cat's in the Bag...</title><season>1</season><episode>2</episode></episodedetails>",
        );
        assert_eq!((two.season, two.episodes), (Some(1), Some([1, 2])));
        assert_eq!(two.title.as_deref(), Some("Pilot"));

        // Of two seasons, nothing is numbered.
        let mixed = patterns.parse(
            "<episodedetails><season>1</season><episode>9</episode></episodedetails><episodedetails><season>2</season><episode>1</episode></episodedetails>",
        );
        assert_eq!((mixed.season, mixed.episodes), (None, None));
    }

    #[test]
    fn a_releases_nfo_gives_its_links() {
        let patterns = NfoPatterns::new();
        let nfo = patterns.parse(
            "\u{feff}        ▄▄▄▄▄▄▄▄▄▄▄▄▄▄\n   GRP presents Heat 1995 1080p\n\n   IMDb: http://www.imdb.com/title/tt0113277/\n   Rating: 8.3/10\n   TMDb: https://www.themoviedb.org/movie/949\n   https://www.imdb.com/title/tt0113277/\n",
        );
        assert_eq!(nfo.kind, NfoKind::Release);
        assert_eq!(
            nfo.ids,
            [ExternalId::Imdb("tt0113277".into()), ExternalId::Tmdb(949)]
        );
        assert_eq!(nfo.title, None);
    }

    #[test]
    fn entities_are_decoded_and_the_rest_left_alone() {
        assert_eq!(
            decode(" R&amp;D &lt;3 &#x41;&#66; &unknown; & co "),
            "R&D <3 AB &unknown; & co"
        );
    }
}
