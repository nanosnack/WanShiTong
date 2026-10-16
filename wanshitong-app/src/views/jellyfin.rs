//! Naming movies and TV series by Jellyfin's rules.
//!
//! See <https://jellyfin.org/docs/general/server/media/movies>. A movie's
//! folder is named like `Title (Year) [tmdbid-N]`, its manifestations are named after
//! the folder and their editions, e.g. `Title (Year) [tmdbid-N] - Director's
//! Cut.mkv`, a manifestation's parts after that and their numbers, e.g.
//! `Title (Year) [tmdbid-N] - part1.mkv`, and a disc's `BDMV` or `VIDEO_TS`
//! folder is linked as is.
//!
//! See <https://jellyfin.org/docs/general/server/media/shows>. A series'
//! folder is named like `Name [tmdbid-N]`.
//!
//! Extras go in an `extras` subfolder.

use super::layout::{Convention, IdTag};

pub(crate) struct Jellyfin;

impl Convention for Jellyfin {
    const NAME: &str = "Jellyfin";
    const EXTRAS_FOLDER: &str = "extras";
    const PLAYS_DISCS: bool = true;

    fn id_tag(id: IdTag) -> String {
        match id {
            IdTag::Tmdb(id) => format!("[tmdbid-{id}]"),
            IdTag::Tvdb(id) => format!("[tvdbid-{id}]"),
            IdTag::Imdb(id) => format!("[imdbid-{id}]"),
        }
    }

    fn edition_suffix(edition: &str) -> String {
        format!(" - {edition}")
    }

    fn part_suffix(part: u32) -> String {
        format!(" - part{part}")
    }
}
