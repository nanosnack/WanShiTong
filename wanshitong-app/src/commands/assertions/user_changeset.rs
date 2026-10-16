//! A changeset for one correction by the user, and how it and its
//! interpretation are named.

use crate::commands::{USER, create_changeset};
use crate::{AppContext, AppError};
use std::path::Path;
use wanshitong_core::commands::NewChangeset;
use wanshitong_core::domain::{ChangesetUuid, ClaimPredicate, ExternalId};

/// A changeset for one correction by the user in the web UI.
pub(super) async fn user_changeset(
    ctx: &AppContext,
    description: &str,
) -> Result<ChangesetUuid, AppError> {
    create_changeset(
        ctx,
        NewChangeset {
            author: USER.into(),
            description: description.into(),
        },
    )
    .await
}

/// A few words about a claim, for naming interpretations.
pub(super) fn describe(claim: &ClaimPredicate) -> String {
    match claim {
        ClaimPredicate::MovieExternalId(id) | ClaimPredicate::TvSeriesExternalId(id) => match id {
            ExternalId::Imdb(id) => format!("IMDB {id}"),
            ExternalId::Tmdb(id) => format!("TMDB {id}"),
            ExternalId::Tvdb(id) => format!("TVDB {id}"),
        },
        _ => "a claim".to_string(),
    }
}

/// The file's name, made fit for an interpretation name: shortened, and
/// without control characters.
///
/// Interpretation names must be under 128 bytes. Many scripts take several
/// bytes a character, so this is limited in bytes, leaving room for the rest
/// of the name.
pub(super) fn file_name(rel_path: &Path) -> String {
    const MAX_BYTES: usize = 60;
    let name = rel_path
        .file_name()
        .unwrap_or(rel_path.as_os_str())
        .to_string_lossy()
        .replace(char::is_control, " ");
    if name.len() <= MAX_BYTES {
        return name;
    }
    let mut end = MAX_BYTES - '…'.len_utf8();
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &name[..end])
}

#[cfg(test)]
mod tests {
    use super::file_name;
    use std::path::Path;

    #[test]
    fn shortens_file_names_by_bytes() {
        let name = file_name(Path::new(
            "AnimeMovies/(アニメ)(映画)東映まんがまつり グリム童話 金の鳥(1987) (東映チャンネルHD 1440x1080 x265).mp4",
        ));
        assert!(name.len() <= 60, "{} bytes", name.len());
        assert_eq!(name, "(アニメ)(映画)東映まんがまつり グリム童…");

        assert_eq!(file_name(Path::new("Heat (1995).mkv")), "Heat (1995).mkv");
        assert_eq!(file_name(Path::new("new\nline.mkv")), "new line.mkv");
    }
}
