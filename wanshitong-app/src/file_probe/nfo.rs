//! NFO files: a release's, e.g. with its IMDb link, or a media server's or
//! its manager's, e.g. Kodi's XML, beside a video or at its series' root.
//! Ingest reads one's text as a measurement, for the heuristics to read as a
//! fact; nothing interprets it here.

use anyhow::Context;
use std::path::Path;

/// The most an NFO is read, in bytes. A release's is a few kilobytes of
/// text, and a media server's a few more of XML; anything larger is a file
/// misnamed, e.g. a video.
pub const MAX_NFO_BYTES: u64 = 1024 * 1024;

/// Whether a path is an NFO's, by its extension.
pub fn is_nfo_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("nfo"))
}

/// An NFO's text, if `path` is one of at most [`MAX_NFO_BYTES`], as `size`
/// measured it. A release's NFO is often encoded as its ANSI art is, e.g. in
/// code page 437, so what isn't UTF-8 is replaced: its IDs and links are
/// ASCII either way. NULs are dropped, as the database's text can't hold
/// them.
pub async fn read_nfo_text(path: &Path, size: u64) -> anyhow::Result<Option<String>> {
    if !is_nfo_path(path) || size > MAX_NFO_BYTES {
        return Ok(None);
    }
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("reading NFO: '{}'", path.display()))?;
    let text: String = String::from_utf8_lossy(&bytes)
        .chars()
        .filter(|&c| c != '\0')
        .collect();
    Ok(Some(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_nfo_is_told_by_its_extension() {
        assert!(is_nfo_path(Path::new("Heat.1995-GRP/heat.1995-grp.nfo")));
        assert!(is_nfo_path(Path::new("Movie.NFO")));
        assert!(!is_nfo_path(Path::new("Movie.nfo.txt")));
        assert!(!is_nfo_path(Path::new("nfo")));
    }

    #[tokio::test]
    async fn an_nfo_is_read_lossily_without_nuls() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("release.nfo");
        // Code page 437 box drawing, then a link, then a NUL.
        let bytes = b"\xc9\xcd\xbb\r\nhttps://www.imdb.com/title/tt0113277/\r\n\0";
        std::fs::write(&path, bytes).unwrap();
        let text = read_nfo_text(&path, bytes.len() as u64)
            .await
            .unwrap()
            .unwrap();
        assert!(text.contains("imdb.com/title/tt0113277"));
        assert!(!text.contains('\0'));
        assert!(text.starts_with(char::REPLACEMENT_CHARACTER));

        let video = dir.path().join("release.mkv");
        std::fs::write(&video, b"").unwrap();
        assert_eq!(read_nfo_text(&video, 0).await.unwrap(), None);
        assert_eq!(read_nfo_text(&path, MAX_NFO_BYTES + 1).await.unwrap(), None);
    }
}
