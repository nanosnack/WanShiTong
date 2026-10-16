use anyhow::Context;
use std::io::SeekFrom;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// A disc's sectors, in bytes.
const SECTOR: usize = 2048;
/// Where a disc's volume descriptors start: sector 16, after its system area.
const VOLUME_DESCRIPTORS: u64 = 16 * SECTOR as u64;
/// How many sectors to look through: ISO 9660's descriptors come first, if
/// any, then UDF's volume recognition sequence.
const SECTORS: usize = 16;

/// Whether the file is a disc image, e.g. an `.iso`: an ISO 9660 or UDF
/// filesystem, as its volume descriptors tell, whatever the file's called.
pub async fn is_disc_image(path: &Path) -> anyhow::Result<bool> {
    let mut file = File::open(path)
        .await
        .with_context(|| format!("opening file to tell a disc image: '{}'", path.display()))?;
    file.seek(SeekFrom::Start(VOLUME_DESCRIPTORS))
        .await
        .context("seeking to a disc image's volume descriptors")?;
    let mut sectors = Vec::with_capacity(SECTORS * SECTOR);
    file.take((SECTORS * SECTOR) as u64)
        .read_to_end(&mut sectors)
        .await
        .context("reading a disc image's volume descriptors")?;
    Ok(sectors.as_chunks::<SECTOR>().0.iter().any(|sector| {
        // Each descriptor's identifier follows its type byte.
        matches!(&sector[1..6], b"CD001" | b"BEA01" | b"NSR02" | b"NSR03")
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A file with these descriptors' identifiers, one per sector from
    /// sector 16.
    fn image(identifiers: &[&[u8; 5]]) -> tempfile::NamedTempFile {
        let mut bytes = vec![0; VOLUME_DESCRIPTORS as usize];
        for identifier in identifiers {
            let mut sector = vec![0; SECTOR];
            sector[1..6].copy_from_slice(*identifier);
            bytes.extend(sector);
        }
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&bytes).unwrap();
        file
    }

    #[tokio::test]
    async fn tells_disc_images_by_their_volume_descriptors() {
        // A Blu-ray's: UDF only.
        let blu_ray = image(&[b"BEA01", b"NSR03", b"TEA01"]);
        assert!(is_disc_image(blu_ray.path()).await.unwrap());
        // A DVD's: ISO 9660 and UDF.
        let dvd = image(&[b"CD001", b"CD001", b"BEA01", b"NSR02", b"TEA01"]);
        assert!(is_disc_image(dvd.path()).await.unwrap());
    }

    #[tokio::test]
    async fn other_files_arent_disc_images() {
        let other = image(&[b"ABCDE", b"FGHIJ"]);
        assert!(!is_disc_image(other.path()).await.unwrap());
        // Shorter than a disc's system area, e.g. a subtitle.
        let mut short = tempfile::NamedTempFile::new().unwrap();
        short
            .write_all(b"1\n00:00:01,000 --> 00:00:02,000\nHi\n")
            .unwrap();
        assert!(!is_disc_image(short.path()).await.unwrap());
    }
}
