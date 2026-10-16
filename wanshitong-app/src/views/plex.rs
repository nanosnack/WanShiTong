//! Naming movies and TV series by Plex's rules.
//!
//! See <https://support.plex.tv/articles/naming-and-organizing-your-movie-media-files/>.
//! A movie's folder is named like `Title (Year) {tmdb-N}`, and its manifestations are
//! named after the folder and their editions, e.g.
//! `Title (Year) {tmdb-N} {edition-Director's Cut}.mkv` (see
//! <https://support.plex.tv/articles/multiple-editions/>), and a manifestation's parts
//! after that and their numbers, e.g. `Title (Year) {tmdb-N} - pt1.mkv`. Plex
//! can't play a disc, be it a `BDMV` or `VIDEO_TS` folder or a disc image, so
//! discs are left out.
//!
//! See <https://support.plex.tv/articles/naming-and-organizing-your-tv-show-files/>.
//! A series' folder is named like `Name {tmdb-N}`.
//!
//! Extras go in an `Other` subfolder, as Plex knows nothing more of what they
//! are (see <https://support.plex.tv/articles/local-files-for-trailers-and-extras/>).

use super::layout::{Convention, IdTag};

pub(crate) struct Plex;

/// Plex shows no more of an edition's name than this many characters.
const MAX_EDITION_CHARS: usize = 32;

impl Convention for Plex {
    const NAME: &str = "Plex";
    const EXTRAS_FOLDER: &str = "Other";
    const PLAYS_DISCS: bool = false;

    fn id_tag(id: IdTag) -> String {
        match id {
            IdTag::Tmdb(id) => format!("{{tmdb-{id}}}"),
            IdTag::Tvdb(id) => format!("{{tvdb-{id}}}"),
            IdTag::Imdb(id) => format!("{{imdb-{id}}}"),
        }
    }

    fn edition_suffix(edition: &str) -> String {
        // A brace would end the tag early.
        let edition: String = edition
            .chars()
            .map(|c| match c {
                '{' => '(',
                '}' => ')',
                c => c,
            })
            .take(MAX_EDITION_CHARS)
            .collect();
        format!(" {{edition-{}}}", edition.trim_end())
    }

    fn part_suffix(part: u32) -> String {
        format!(" - pt{part}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::layout::{plan_movies, plan_shows};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use wanshitong_core::domain::{
        FileRole, ViewEpisodeFile, ViewMovie, ViewMovieFile, ViewTvSeries,
    };

    fn links(pairs: &[(&str, &str)]) -> BTreeMap<PathBuf, PathBuf> {
        pairs
            .iter()
            .map(|(link, target)| (link.into(), target.into()))
            .collect()
    }

    fn file(
        role: FileRole,
        manifestation: usize,
        edition: Option<&str>,
        path: &str,
    ) -> ViewMovieFile {
        ViewMovieFile {
            role: Some(role),
            manifestation: (role == FileRole::Main).then_some(manifestation),
            part: None,
            edition: edition.map(String::from),
            disc: false,
            path: path.into(),
        }
    }

    /// A manifestation that's a disc, by its index file.
    fn disc(manifestation: usize, path: &str) -> ViewMovieFile {
        ViewMovieFile {
            disc: true,
            ..file(FileRole::Main, manifestation, None, path)
        }
    }

    #[test]
    fn names_movies_by_id_tag_and_edition_and_puts_extras_in_other() {
        let alien = ViewMovie {
            title: "Alien³".into(),
            release_year: Some(1992),
            tmdb_id: Some(8077),
            imdb_id: None,
            files: vec![
                file(
                    FileRole::Main,
                    0,
                    Some("Assembly Cut"),
                    "/m/a/Alien3.AC.mkv",
                ),
                file(
                    FileRole::Main,
                    1,
                    Some("Assembly Cut"),
                    "/m/b/Alien3.AC.mkv",
                ),
                file(FileRole::Main, 2, None, "/m/c/Alien3.mp4"),
                file(FileRole::Extra, 0, None, "/m/c/Trailer.mkv"),
            ],
        };
        let heat = ViewMovie {
            title: "Heat".into(),
            release_year: Some(1995),
            tmdb_id: None,
            imdb_id: Some("tt0113277".into()),
            files: vec![file(
                FileRole::Main,
                0,
                Some("Director's {Definitive} Extended Cut"),
                "/m/heat.mkv",
            )],
        };
        let folder = "Alien³ (1992) {tmdb-8077}";
        let heat_folder = "Heat (1995) {imdb-tt0113277}";
        assert_eq!(
            plan_movies::<Plex>(&[alien, heat]),
            links(&[
                (
                    &format!("{folder}/{folder} {{edition-Assembly Cut}}.mkv"),
                    "/m/a/Alien3.AC.mkv"
                ),
                (
                    &format!("{folder}/{folder} {{edition-Assembly Cut}} - 2.mkv"),
                    "/m/b/Alien3.AC.mkv"
                ),
                (&format!("{folder}/{folder}.mp4"), "/m/c/Alien3.mp4"),
                (&format!("{folder}/Other/Trailer.mkv"), "/m/c/Trailer.mkv"),
                (
                    &format!(
                        "{heat_folder}/{heat_folder} {{edition-Director's (Definitive) Extended}}.mkv"
                    ),
                    "/m/heat.mkv"
                ),
            ]),
        );
    }

    #[test]
    fn leaves_out_discs_and_movies_only_on_discs() {
        let arrietty = ViewMovie {
            title: "Arrietty".into(),
            release_year: Some(2010),
            tmdb_id: None,
            imdb_id: None,
            files: vec![
                disc(0, "/m/Arrietty BD/BDMV/index.bdmv"),
                file(FileRole::Main, 1, None, "/m/Arrietty/arrietty.mkv"),
            ],
        };
        let wind = ViewMovie {
            title: "The Wind Rises".into(),
            release_year: Some(2013),
            tmdb_id: None,
            imdb_id: None,
            files: vec![disc(0, "/m/The.Wind.Rises.2013.Blu-ray.iso")],
        };
        let keep = ViewMovie {
            title: "Keep On Keepin' On".into(),
            release_year: Some(2014),
            tmdb_id: None,
            imdb_id: None,
            files: vec![
                disc(0, "/m/Keep/VIDEO_TS/VIDEO_TS.IFO"),
                file(FileRole::Extra, 0, None, "/m/Keep/trailer.mkv"),
            ],
        };
        assert_eq!(
            plan_movies::<Plex>(&[arrietty, wind, keep]),
            links(&[(
                "Arrietty (2010)/Arrietty (2010).mkv",
                "/m/Arrietty/arrietty.mkv"
            )]),
        );
    }

    #[test]
    fn names_series_by_id_tag_and_puts_extras_in_other() {
        let wire = ViewTvSeries {
            name: "The Wire".into(),
            tmdb_id: None,
            tvdb_id: Some(79126),
            imdb_id: None,
            episode_files: vec![
                ViewEpisodeFile {
                    season: Some(1),
                    episode: 1,
                    path: "/tv/wire/1x01.avi".into(),
                },
                ViewEpisodeFile {
                    season: Some(0),
                    episode: 3,
                    path: "/tv/wire/special.mkv".into(),
                },
            ],
            extras: vec!["/tv/wire/NCOP.mkv".into()],
        };
        let folder = "The Wire {tvdb-79126}";
        assert_eq!(
            plan_shows::<Plex>(&[wire]),
            links(&[
                (&format!("{folder}/Other/NCOP.mkv"), "/tv/wire/NCOP.mkv"),
                (
                    &format!("{folder}/Season 00/The Wire S00E03.mkv"),
                    "/tv/wire/special.mkv"
                ),
                (
                    &format!("{folder}/Season 01/The Wire S01E01.avi"),
                    "/tv/wire/1x01.avi"
                ),
            ]),
        );
    }
}
