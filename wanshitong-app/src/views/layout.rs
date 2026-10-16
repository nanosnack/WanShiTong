//! Laying out movies and TV series as media servers expect, by a
//! [`Convention`] for naming them.
//!
//! Each movie gets a folder named like `Title (Year)` and an ID tag, each of
//! its manifestations is linked by its main file, named after the folder and its
//! edition (or, for a disc, its `BDMV` or `VIDEO_TS` folder is linked), and
//! extras go in a subfolder.
//!
//! Each series gets a folder named like `Name` and an ID tag, with a
//! `Season 01` folder of episodes named like `Name S01E01`, and extras go in a
//! subfolder.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, btree_map::Entry};
use std::path::{Path, PathBuf};
use tracing::warn;
use wanshitong_core::domain::{
    EpisodeNumber, FileRole, SeasonNumber, ViewMovie, ViewMovieFile, ViewTvSeries,
};

/// How a media server wants the folders and files of its libraries named.
pub(crate) trait Convention {
    /// The media server's name, for warnings.
    const NAME: &str;

    /// The subfolder of a movie's or series' folder for its extras.
    const EXTRAS_FOLDER: &str;

    /// Whether the media server plays a disc as the movie, be it a disc's
    /// `BDMV` or `VIDEO_TS` folder in the movie's folder or a disc image.
    const PLAYS_DISCS: bool;

    /// The tag that names a movie or series by its ID, to go after its name,
    /// e.g. `[tmdbid-8077]`.
    fn id_tag(id: IdTag) -> String;

    /// What goes after a movie's name to name a manifestation of an edition, e.g.
    /// ` - Director's Cut`. The edition is already sanitized.
    fn edition_suffix(edition: &str) -> String;

    /// What goes after a version's name to name one of its parts, e.g.
    /// ` - part1`, for a manifestation of several files.
    fn part_suffix(part: u32) -> String;
}

/// An ID to name a movie or series by. An IMDb ID is already sanitized.
pub(crate) enum IdTag<'a> {
    Tmdb(u32),
    Tvdb(u32),
    Imdb(&'a str),
}

/// Plans the symlinks for a movies library: link paths, relative to the
/// library directory, mapped to the absolute paths they point at.
///
/// Each manifestation of a movie is linked as one of the media server's versions of
/// it, named after the folder and its expression's edition, e.g.
/// `Brazil (1985) - Director's Cut.mkv`, or the folder alone for the
/// unspecified expression. Manifestations of one expression are told apart by a
/// number after it, in the order of their paths. A manifestation of several parts,
/// e.g. `CD1` and `CD2`, is linked as the media server's parts of one
/// version, e.g. `Brazil (1985) - part1.mkv`. See *View* in
/// `spec/domain-model.md`.
///
/// Link names only depend on the movies and their files, never on catalog
/// row IDs or UUIDs, so a movie keeps its path from one interpretation to the
/// next and the media server keeps its watch state.
pub(crate) fn plan_movies<C: Convention>(movies: &[ViewMovie]) -> BTreeMap<PathBuf, PathBuf> {
    // Visit movies in the order of their first manifestation's path, so that name
    // collisions are always resolved the same way.
    let mut movies: Vec<(&ViewMovie, Vec<Vec<&ViewMovieFile>>)> = movies
        .iter()
        .filter_map(|movie| {
            let mut manifestations = manifestations(movie);
            if !C::PLAYS_DISCS {
                manifestations.retain(|manifestation| {
                    if manifestation[0].disc {
                        warn!(
                            "{} can't play a disc; leaving out {}",
                            C::NAME,
                            manifestation[0].path.display()
                        );
                    }
                    !manifestation[0].disc
                });
            }
            if manifestations.is_empty() {
                warn!(
                    "Movie \"{}\" has no main file to link; leaving it out",
                    movie.title
                );
            }
            Some((movie, manifestations)).filter(|(_, manifestations)| !manifestations.is_empty())
        })
        .collect();
    movies.sort_by(|a, b| a.1[0][0].path.cmp(&b.1[0][0].path));

    let mut links = BTreeMap::new();
    let mut folders = HashSet::new();
    for (movie, manifestations) in movies {
        let base = base_name::<C>(movie);
        let folder = unique_name(&mut folders, &base, "");
        if folder != base {
            warn!(
                "Movie \"{}\" collides with another movie's name; linking it as \"{folder}\"",
                movie.title
            );
        }
        let folder_path = Path::new(&folder);

        let mut versions = HashSet::new();
        for manifestation in manifestations {
            let first = manifestation[0];
            if let Some((disc_folder, target)) = disc_folder(&first.path) {
                // A media server plays one disc folder in a movie's folder.
                match links.entry(folder_path.join(disc_folder)) {
                    Entry::Vacant(link) => {
                        link.insert(target.to_owned());
                    }
                    Entry::Occupied(_) => warn!(
                        "Movie \"{}\" has another manifestation on a disc; leaving {} out",
                        movie.title,
                        target.display(),
                    ),
                }
                continue;
            }
            let version = match &first.edition {
                Some(edition) => folder.clone() + &C::edition_suffix(&sanitize(edition)),
                None => folder.clone(),
            };
            // Names differing only by extension would be one version too.
            let name = unique_name(&mut versions, &version, "");
            if let [only] = manifestation[..] {
                links.insert(
                    folder_path.join(file_name(&name, extension(&only.path))),
                    only.path.clone(),
                );
                continue;
            }
            // The parts of one manifestation, each by its number. Two numbered alike
            // would be one part to the media server, so the second is left
            // out.
            let mut parts = HashSet::new();
            for (i, main) in manifestation.iter().enumerate() {
                let part = main.part.unwrap_or(i as u32 + 1);
                if !parts.insert(part) {
                    warn!(
                        "Movie \"{}\" has two files numbered as part {part} of one manifestation; leaving {} out",
                        movie.title,
                        main.path.display(),
                    );
                    continue;
                }
                links.insert(
                    folder_path.join(file_name(
                        &format!("{name}{}", C::part_suffix(part)),
                        extension(&main.path),
                    )),
                    main.path.clone(),
                );
            }
        }

        let extras = movie
            .files
            .iter()
            .filter(|file| file.role == Some(FileRole::Extra))
            .map(|file| file.path.as_path());
        link_extras::<C>(&mut links, folder_path, extras);
    }
    links
}

/// A movie's manifestations, each as its main files in the order of their parts, then
/// their paths, and the manifestations in the order of their first files' paths. A
/// main file in no manifestation, as catalogs from before manifestations were recorded have,
/// is a manifestation of its own.
fn manifestations(movie: &ViewMovie) -> Vec<Vec<&ViewMovieFile>> {
    let mut manifestations = BTreeMap::<usize, Vec<&ViewMovieFile>>::new();
    let mains = movie
        .files
        .iter()
        .filter(|file| file.role == Some(FileRole::Main));
    for (i, main) in mains.enumerate() {
        let manifestation = main.manifestation.unwrap_or(usize::MAX - i);
        manifestations.entry(manifestation).or_default().push(main);
    }
    let mut manifestations: Vec<Vec<&ViewMovieFile>> = manifestations.into_values().collect();
    for manifestation in &mut manifestations {
        manifestation.sort_by(|a, b| a.part.cmp(&b.part).then_with(|| a.path.cmp(&b.path)));
    }
    manifestations.sort_by(|a, b| a[0].path.cmp(&b[0].path));
    manifestations
}

/// Plans the symlinks for a TV shows library, as [`plan_movies`] does for
/// movies.
///
/// Each series gets a folder named like `Name [tmdbid-N]`, holding a
/// `Season NN` folder per season, with its episodes named like
/// `Name S01E01.mkv` (or `Name S01E01-E02.mkv` for a file of several), and
/// its extras in their own folder. Every file of an episode is linked, as
/// the media server's versions of it, told apart by a number, e.g.
/// `Name S01E01 - 2.mkv` (see [`episode_links`]). Episode titles are left
/// out of the names, so correcting one doesn't cost the media server its
/// watch state.
pub(crate) fn plan_shows<C: Convention>(series: &[ViewTvSeries]) -> BTreeMap<PathBuf, PathBuf> {
    // Visit series in the order of their first episode file's path, so that
    // name collisions are always resolved the same way.
    let mut series: Vec<(&ViewTvSeries, &Path)> = series
        .iter()
        .filter_map(|series| {
            let first = series.episode_files.iter().map(|f| f.path.as_path()).min();
            if first.is_none() {
                warn!(
                    "Series \"{}\" has no episode files; leaving it out",
                    series.name
                );
            }
            Some((series, first?))
        })
        .collect();
    series.sort_by_key(|(_, first)| *first);

    let mut links = BTreeMap::new();
    let mut folders = HashSet::new();
    for (series, _) in series {
        let base = series_base_name::<C>(series);
        let folder = unique_name(&mut folders, &base, "");
        if folder != base {
            warn!(
                "Series \"{}\" collides with another series' name; linking it as \"{folder}\"",
                series.name
            );
        }
        let folder_path = Path::new(&folder);
        let name = sanitize(&series.name);

        let (episodes, unseasoned) = episode_links(series);
        // Versions of one episode are told apart by a number, in the order
        // of their paths, as the links are.
        let mut names = HashSet::new();
        for episode in episodes {
            let code = if episode.first == episode.last {
                format!("S{:02}E{:02}", episode.season, episode.first)
            } else {
                format!(
                    "S{:02}E{:02}-E{:02}",
                    episode.season, episode.first, episode.last
                )
            };
            let ext = extension(episode.path);
            let link = format!("Season {:02}/{name} {code}", episode.season);
            let link = unique_name(&mut names, &link, ext);
            links.insert(
                folder_path.join(file_name(&link, ext)),
                episode.path.to_owned(),
            );
        }

        link_extras::<C>(
            &mut links,
            folder_path,
            series.extras.iter().map(PathBuf::as_path).chain(unseasoned),
        );
    }
    links
}

/// A file to link as a version of the episodes `first` to `last` of a season:
/// the longest range of the files beginning on `first`, which may be more
/// than the file holds (see [`episode_links`]).
struct EpisodeLink<'a> {
    season: SeasonNumber,
    first: EpisodeNumber,
    last: EpisodeNumber,
    path: &'a Path,
}

/// The files to link as a series' episodes, in the order of their seasons,
/// first episodes, ranges (longest first) and paths, and the files of an
/// unknown season to link as extras instead.
///
/// Every file of an episode is linked, as the media server's versions of it
/// (see *View* in `spec/domain-model.md`). A media server takes the files
/// that begin on one episode for versions of one entry, whichever episode
/// each ends on, and shows the entry as its first version by name, so the
/// files beginning on an episode are all named by the longest range among
/// them, the longest first: `E01-E02` beside `E01` are `S01E01-E02` and
/// `S01E01-E02 - 2`, and the whole range plays by default. A file within
/// another's range that begins later, e.g. `E450` beside `E447-E450`, is an
/// entry of its own.
///
/// Media servers need a season for every episode, so episodes of an unknown
/// season, numbered through the whole series, go in season 1. If the series
/// has a season 1 already, they aren't mixed into it: their files are
/// returned apart, in order of their paths, to link as extras, so that no
/// file of the series is left out.
fn episode_links(series: &ViewTvSeries) -> (Vec<EpisodeLink<'_>>, Vec<&Path>) {
    let has_season_one = series.episode_files.iter().any(|f| f.season == Some(1));
    // Each file's season and episodes.
    let mut files = HashMap::<&Path, (SeasonNumber, Vec<EpisodeNumber>)>::new();
    let mut unseasoned = BTreeSet::<&Path>::new();
    for file in &series.episode_files {
        let season = match file.season {
            Some(season) => season,
            None if !has_season_one => 1,
            None => {
                unseasoned.insert(file.path.as_path());
                continue;
            }
        };
        files
            .entry(file.path.as_path())
            .or_insert((season, Vec::new()))
            .1
            .push(file.episode);
    }
    if !unseasoned.is_empty() {
        warn!(
            "Series \"{}\" has a season 1 as well as episodes of an unknown season; \
             linking its {} files of the unknown season as extras",
            series.name,
            unseasoned.len(),
        );
    }

    // The files that begin on each episode, with their last episodes.
    let mut by_first =
        BTreeMap::<(SeasonNumber, EpisodeNumber), Vec<(EpisodeNumber, &Path)>>::new();
    for (path, (season, episodes)) in &files {
        let first = *episodes.iter().min().expect("files have episodes");
        let last = *episodes.iter().max().expect("files have episodes");
        by_first
            .entry((*season, first))
            .or_default()
            .push((last, path));
    }
    let mut links = Vec::new();
    for ((season, first), mut ranges) in by_first {
        // Longest first, then by path.
        ranges.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        let last = ranges[0].0;
        for (_, path) in ranges {
            links.push(EpisodeLink {
                season,
                first,
                last,
                path,
            });
        }
    }
    (links, unseasoned.into_iter().collect())
}

/// Links extras in a folder's extras folder, named after their own files, in
/// order of their paths.
fn link_extras<'a, C: Convention>(
    links: &mut BTreeMap<PathBuf, PathBuf>,
    folder_path: &Path,
    extras: impl Iterator<Item = &'a Path>,
) {
    let mut extras: Vec<&Path> = extras.collect();
    extras.sort();
    let mut names = HashSet::new();
    for extra in extras {
        let stem = sanitize(&extra.file_stem().unwrap_or_default().to_string_lossy());
        let ext = extension(extra);
        let name = unique_name(&mut names, &stem, ext);
        links.insert(
            folder_path
                .join(C::EXTRAS_FOLDER)
                .join(file_name(&name, ext)),
            extra.to_owned(),
        );
    }
}

/// For a disc's main file, the name media servers expect for the disc's
/// folder, and that folder. Those that play discs play a `BDMV` or `VIDEO_TS`
/// folder in a movie's folder as the movie, but not the file that represents
/// the disc.
///
/// A DVD's folder is linked as `VIDEO_TS` whatever it's called, as rips
/// sometimes rename it (e.g. `VIDEO_TS (13)`) or put the DVD files directly
/// in the movie's folder.
fn disc_folder(main: &Path) -> Option<(&'static str, &Path)> {
    let file_name = main.file_name()?;
    let parent = main.parent()?;
    if file_name.eq_ignore_ascii_case("index.bdmv")
        && parent
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("BDMV"))
    {
        Some(("BDMV", parent))
    } else if file_name.eq_ignore_ascii_case("VIDEO_TS.IFO") {
        Some(("VIDEO_TS", parent))
    } else {
        None
    }
}

/// `Title (Year) [tmdbid-N]`, leaving out whatever is unknown.
fn base_name<C: Convention>(movie: &ViewMovie) -> String {
    let mut name = sanitize(&movie.title);
    if let Some(year) = movie.release_year {
        name.push_str(&format!(" ({year})"));
    }
    let imdb_id = movie.imdb_id.as_deref().map(sanitize);
    let id = match (movie.tmdb_id, &imdb_id) {
        (Some(tmdb_id), _) => Some(IdTag::Tmdb(tmdb_id)),
        (None, Some(imdb_id)) => Some(IdTag::Imdb(imdb_id)),
        (None, None) => None,
    };
    if let Some(id) = id {
        name.push(' ');
        name.push_str(&C::id_tag(id));
    }
    name
}

/// `Name [tmdbid-N]`, or with a TVDB or IMDb ID, or none.
fn series_base_name<C: Convention>(series: &ViewTvSeries) -> String {
    let mut name = sanitize(&series.name);
    let imdb_id = series.imdb_id.as_deref().map(sanitize);
    let id = match (series.tmdb_id, series.tvdb_id, &imdb_id) {
        (Some(tmdb_id), _, _) => Some(IdTag::Tmdb(tmdb_id)),
        (None, Some(tvdb_id), _) => Some(IdTag::Tvdb(tvdb_id)),
        (None, None, Some(imdb_id)) => Some(IdTag::Imdb(imdb_id)),
        (None, None, None) => None,
    };
    if let Some(id) = id {
        name.push(' ');
        name.push_str(&C::id_tag(id));
    }
    name
}

/// Returns `name`, or `name - 2`, `name - 3`, ... if it's already taken, and
/// marks it taken. Names differing only by case collide, as they would on a
/// case-insensitive share.
fn unique_name(taken: &mut HashSet<String>, name: &str, ext: &str) -> String {
    let mut candidate = name.to_string();
    let mut n = 1;
    while !taken.insert(file_name(&candidate, ext).to_lowercase()) {
        n += 1;
        candidate = format!("{name} - {n}");
    }
    candidate
}

fn file_name(stem: &str, ext: &str) -> String {
    if ext.is_empty() {
        stem.to_string()
    } else {
        format!("{stem}.{ext}")
    }
}

fn extension(path: &Path) -> &str {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
}

/// Makes a string safe as a file name here and on the file systems media
/// servers are commonly served from (e.g. SMB shares).
fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            // "Mission: Impossible" -> "Mission - Impossible"
            ':' => out.push_str(" - "),
            // "Face/Off" -> "Face-Off"
            '/' | '\\' => out.push('-'),
            '"' => out.push('\''),
            '*' | '?' | '<' | '>' | '|' => out.push(' '),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    // Leading dots hide files; trailing dots and spaces are dropped on Windows.
    let trimmed = collapsed
        .trim_start_matches('.')
        .trim_end_matches(['.', ' ']);
    if trimmed.is_empty() {
        "Untitled".into()
    } else {
        trimmed.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::jellyfin::Jellyfin;
    use wanshitong_core::domain::ViewEpisodeFile;

    fn movie(
        title: &str,
        year: Option<u32>,
        tmdb_id: Option<u32>,
        files: &[(FileRole, &str)],
    ) -> ViewMovie {
        ViewMovie {
            title: title.into(),
            release_year: year,
            tmdb_id,
            imdb_id: None,
            files: files
                .iter()
                .map(|(role, path)| ViewMovieFile {
                    role: Some(*role),
                    manifestation: None,
                    part: None,
                    edition: None,
                    disc: disc_folder(Path::new(path)).is_some(),
                    path: path.into(),
                })
                .collect(),
        }
    }

    fn links(pairs: &[(&str, &str)]) -> BTreeMap<PathBuf, PathBuf> {
        pairs
            .iter()
            .map(|(link, target)| (link.into(), target.into()))
            .collect()
    }

    #[test]
    fn names_folder_and_main_file_after_title_year_and_id() {
        let movies = [movie(
            "Blade",
            Some(1998),
            Some(36647),
            &[(FileRole::Main, "/media/blade.MKV")],
        )];
        assert_eq!(
            plan_movies::<Jellyfin>(&movies),
            links(&[(
                "Blade (1998) [tmdbid-36647]/Blade (1998) [tmdbid-36647].MKV",
                "/media/blade.MKV"
            )]),
        );
    }

    #[test]
    fn falls_back_to_imdb_id_then_to_no_id_and_no_year() {
        let mut imdb_only = movie("Heat", Some(1995), None, &[(FileRole::Main, "/m/heat.mkv")]);
        imdb_only.imdb_id = Some("tt0113277".into());
        let bare = movie("Unknown", None, None, &[(FileRole::Main, "/m/unknown.avi")]);
        assert_eq!(
            plan_movies::<Jellyfin>(&[imdb_only, bare]),
            links(&[
                (
                    "Heat (1995) [imdbid-tt0113277]/Heat (1995) [imdbid-tt0113277].mkv",
                    "/m/heat.mkv"
                ),
                ("Unknown/Unknown.avi", "/m/unknown.avi"),
            ]),
        );
    }

    #[test]
    fn puts_extras_in_an_extras_folder() {
        let movies = [movie(
            "Heat",
            Some(1995),
            None,
            &[
                (FileRole::Extra, "/m/Heat/Extras/Making Of.mkv"),
                (FileRole::Main, "/m/Heat/heat.mkv"),
                (FileRole::Extra, "/m/Heat/Other/Making Of.mkv"),
            ],
        )];
        assert_eq!(
            plan_movies::<Jellyfin>(&movies),
            links(&[
                ("Heat (1995)/Heat (1995).mkv", "/m/Heat/heat.mkv"),
                (
                    "Heat (1995)/extras/Making Of.mkv",
                    "/m/Heat/Extras/Making Of.mkv"
                ),
                (
                    "Heat (1995)/extras/Making Of - 2.mkv",
                    "/m/Heat/Other/Making Of.mkv"
                ),
            ]),
        );
    }

    #[test]
    fn links_disc_folders_in_place_of_the_main_file() {
        let movies = [
            movie(
                "Arrietty",
                Some(2010),
                None,
                &[(FileRole::Main, "/m/Arrietty.2010/BDMV/index.bdmv")],
            ),
            movie(
                "Keep On Keepin' On",
                Some(2014),
                None,
                &[
                    (FileRole::Main, "/m/Keep (13)/VIDEO_TS (13)/VIDEO_TS.IFO"),
                    (FileRole::Extra, "/m/Keep (13)/trailer.mkv"),
                ],
            ),
            movie(
                "Heat",
                Some(1995),
                None,
                &[(FileRole::Main, "/m/Heat/VIDEO_TS.IFO")],
            ),
        ];
        assert_eq!(
            plan_movies::<Jellyfin>(&movies),
            links(&[
                ("Arrietty (2010)/BDMV", "/m/Arrietty.2010/BDMV"),
                ("Heat (1995)/VIDEO_TS", "/m/Heat"),
                (
                    "Keep On Keepin' On (2014)/VIDEO_TS",
                    "/m/Keep (13)/VIDEO_TS (13)"
                ),
                (
                    "Keep On Keepin' On (2014)/extras/trailer.mkv",
                    "/m/Keep (13)/trailer.mkv"
                ),
            ]),
        );
    }

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize("Mission: Impossible"), "Mission - Impossible");
        assert_eq!(sanitize("Face/Off"), "Face-Off");
        assert_eq!(sanitize("What?  Now*"), "What Now");
        assert_eq!(sanitize("Say \"Hi\"\t..."), "Say 'Hi'");
        assert_eq!(sanitize("...And Justice"), "And Justice");
        assert_eq!(sanitize("???"), "Untitled");
    }

    #[test]
    fn resolves_collisions_by_main_file_path_regardless_of_order() {
        let a = movie("Heat", Some(1995), None, &[(FileRole::Main, "/m/a.mkv")]);
        let b = movie("heat", Some(1995), None, &[(FileRole::Main, "/m/b.mkv")]);
        let expected = links(&[
            ("Heat (1995)/Heat (1995).mkv", "/m/a.mkv"),
            ("heat (1995) - 2/heat (1995) - 2.mkv", "/m/b.mkv"),
        ]);
        assert_eq!(plan_movies::<Jellyfin>(&[a.clone(), b.clone()]), expected);
        assert_eq!(plan_movies::<Jellyfin>(&[b, a]), expected);
    }

    fn series(
        name: &str,
        tmdb_id: Option<u32>,
        episode_files: &[(Option<u32>, u32, &str)],
        extras: &[&str],
    ) -> ViewTvSeries {
        ViewTvSeries {
            name: name.into(),
            tmdb_id,
            tvdb_id: None,
            imdb_id: None,
            episode_files: episode_files
                .iter()
                .map(|(season, episode, path)| ViewEpisodeFile {
                    season: *season,
                    episode: *episode,
                    path: path.into(),
                })
                .collect(),
            extras: extras.iter().map(PathBuf::from).collect(),
        }
    }

    #[test]
    fn names_series_season_and_episode_files() {
        let mut wire = series(
            "The Wire",
            None,
            &[
                (Some(1), 1, "/tv/wire/1x01.avi"),
                (Some(0), 3, "/tv/wire/special.mkv"),
            ],
            &[],
        );
        wire.tvdb_id = Some(79126);
        let geass = series(
            "Code Geass: Lelouch",
            Some(31724),
            &[
                (Some(1), 1, "/tv/cg/S01E01.mkv"),
                (Some(2), 1, "/tv/cg/S02E01-02.mkv"),
                (Some(2), 2, "/tv/cg/S02E01-02.mkv"),
            ],
            &["/tv/cg/NCOP.mkv", "/tv/cg/extra/NCOP.mkv"],
        );
        let folder = "Code Geass - Lelouch [tmdbid-31724]";
        assert_eq!(
            plan_shows::<Jellyfin>(&[wire, geass]),
            links(&[
                (
                    &format!("{folder}/Season 01/Code Geass - Lelouch S01E01.mkv"),
                    "/tv/cg/S01E01.mkv"
                ),
                (
                    &format!("{folder}/Season 02/Code Geass - Lelouch S02E01-E02.mkv"),
                    "/tv/cg/S02E01-02.mkv"
                ),
                (&format!("{folder}/extras/NCOP.mkv"), "/tv/cg/NCOP.mkv"),
                (
                    &format!("{folder}/extras/NCOP - 2.mkv"),
                    "/tv/cg/extra/NCOP.mkv"
                ),
                (
                    "The Wire [tvdbid-79126]/Season 00/The Wire S00E03.mkv",
                    "/tv/wire/special.mkv"
                ),
                (
                    "The Wire [tvdbid-79126]/Season 01/The Wire S01E01.avi",
                    "/tv/wire/1x01.avi"
                ),
            ]),
        );
    }

    #[test]
    fn links_every_file_of_an_episode_as_a_version() {
        let show = series(
            "Show",
            None,
            &[
                // Begins on the same episode as the double, so a version of
                // the same entry: named by the double's range, after it,
                // though it's earlier by path.
                (Some(1), 1, "/tv/a/E01.mkv"),
                (Some(1), 1, "/tv/b/E01-02.mkv"),
                // Within the double's range, but begins later: another entry.
                (Some(1), 2, "/tv/a/E02.mkv"),
                (Some(1), 2, "/tv/b/E01-02.mkv"),
                // Two files of one episode: versions, numbered by path.
                (Some(1), 3, "/tv/b/E03.mkv"),
                (Some(1), 3, "/tv/a/E03.mkv"),
            ],
            &[],
        );
        assert_eq!(
            plan_shows::<Jellyfin>(&[show]),
            links(&[
                ("Show/Season 01/Show S01E01-E02.mkv", "/tv/b/E01-02.mkv"),
                ("Show/Season 01/Show S01E01-E02 - 2.mkv", "/tv/a/E01.mkv"),
                ("Show/Season 01/Show S01E02.mkv", "/tv/a/E02.mkv"),
                ("Show/Season 01/Show S01E03.mkv", "/tv/a/E03.mkv"),
                ("Show/Season 01/Show S01E03 - 2.mkv", "/tv/b/E03.mkv"),
            ]),
        );
    }

    #[test]
    fn puts_episodes_of_an_unknown_season_in_season_1_or_else_in_extras() {
        let one_piece = series(
            "One Piece",
            None,
            &[(None, 92, "/tv/op/092.mkv"), (Some(0), 1, "/tv/op/sp1.mkv")],
            &[],
        );
        let mixed = series(
            "Mixed",
            None,
            &[
                (None, 92, "/tv/mixed/092.mkv"),
                (Some(1), 1, "/tv/mixed/S01E01.mkv"),
            ],
            &[],
        );
        assert_eq!(
            plan_shows::<Jellyfin>(&[one_piece, mixed]),
            links(&[
                ("Mixed/Season 01/Mixed S01E01.mkv", "/tv/mixed/S01E01.mkv"),
                // Not mixed into a season 1 it has, but not left out either.
                ("Mixed/extras/092.mkv", "/tv/mixed/092.mkv"),
                ("One Piece/Season 00/One Piece S00E01.mkv", "/tv/op/sp1.mkv"),
                ("One Piece/Season 01/One Piece S01E92.mkv", "/tv/op/092.mkv"),
            ]),
        );
    }

    #[test]
    fn resolves_series_collisions_by_first_episode_path_and_leaves_out_extras_only_series() {
        let a = series("Show", None, &[(Some(1), 1, "/tv/a.mkv")], &[]);
        let b = series("show", None, &[(Some(1), 1, "/tv/b.mkv")], &[]);
        let extras_only = series("Show", None, &[], &["/tv/NCOP.mkv"]);
        let expected = links(&[
            ("Show/Season 01/Show S01E01.mkv", "/tv/a.mkv"),
            ("show - 2/Season 01/show S01E01.mkv", "/tv/b.mkv"),
        ]);
        assert_eq!(
            plan_shows::<Jellyfin>(&[extras_only.clone(), a.clone(), b.clone()]),
            expected
        );
        assert_eq!(plan_shows::<Jellyfin>(&[b, a, extras_only]), expected);
    }

    #[test]
    fn links_each_copy_as_a_version_named_by_its_edition() {
        let main = |manifestation, edition: Option<&str>, path: &str| ViewMovieFile {
            role: Some(FileRole::Main),
            manifestation: Some(manifestation),
            part: None,
            edition: edition.map(String::from),
            disc: disc_folder(Path::new(path)).is_some(),
            path: path.into(),
        };
        let alien = ViewMovie {
            title: "Alien³".into(),
            release_year: Some(1992),
            tmdb_id: Some(8077),
            imdb_id: None,
            files: vec![
                main(0, Some("Assembly Cut"), "/m/Alien3.Assembly.Cut.2160p.mkv"),
                main(1, Some("Assembly Cut"), "/m/Alien3.Assembly.Cut.1080p.mkv"),
                main(2, None, "/m/Alien 3/Alien3.mp4"),
                // A manifestation in parts is linked as the parts of one version, in
                // the order of their numbers.
                ViewMovieFile {
                    part: Some(2),
                    ..main(3, None, "/n/Alien 3/Alien3.cd2.avi")
                },
                ViewMovieFile {
                    part: Some(1),
                    ..main(3, None, "/n/Alien 3/Alien3.cd1.avi")
                },
                main(4, None, "/n/Alien 3 BD/BDMV/index.bdmv"),
                // Jellyfin plays one disc folder.
                main(5, None, "/o/Alien 3 BD/BDMV/index.bdmv"),
            ],
        };
        let folder = "Alien³ (1992) [tmdbid-8077]";
        assert_eq!(
            plan_movies::<Jellyfin>(&[alien]),
            links(&[
                (
                    &format!("{folder}/{folder} - Assembly Cut.mkv"),
                    "/m/Alien3.Assembly.Cut.1080p.mkv"
                ),
                (
                    &format!("{folder}/{folder} - Assembly Cut - 2.mkv"),
                    "/m/Alien3.Assembly.Cut.2160p.mkv"
                ),
                (&format!("{folder}/{folder}.mp4"), "/m/Alien 3/Alien3.mp4"),
                (
                    &format!("{folder}/{folder} - 2 - part1.avi"),
                    "/n/Alien 3/Alien3.cd1.avi"
                ),
                (
                    &format!("{folder}/{folder} - 2 - part2.avi"),
                    "/n/Alien 3/Alien3.cd2.avi"
                ),
                (&format!("{folder}/BDMV"), "/n/Alien 3 BD/BDMV"),
            ]),
        );
    }

    #[test]
    fn leaves_out_movies_without_a_main_file() {
        let movies = [movie(
            "Heat",
            None,
            None,
            &[(FileRole::Extra, "/m/trailer.mkv")],
        )];
        assert_eq!(plan_movies::<Jellyfin>(&movies), BTreeMap::new());
    }
}
