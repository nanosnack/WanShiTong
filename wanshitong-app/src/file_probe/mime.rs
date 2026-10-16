use anyhow::Context;
use mime_guess::MimeGuess;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// Result of MIME sniffing using multiple independent signals.
#[derive(Debug)]
pub struct MimeSniffResult {
    /// MIME inferred from file content using `infer`
    pub content_mime: Option<&'static str>,

    /// MIME guessed from file path / extension
    pub path_mime: MimeGuess,
}

/// Its message carries the whole cause chain, and it has no source, so that
/// whatever wraps it prints the chain once.
#[derive(Debug, thiserror::Error)]
#[error("MIME sniffing failed: {0:#}")]
pub struct MimeSniffError(anyhow::Error);

impl From<anyhow::Error> for MimeSniffError {
    fn from(error: anyhow::Error) -> Self {
        Self(error)
    }
}

/// Perform MIME sniffing using a bounded async read.
///
/// This function:
/// - Reads at most `SNIFF_LEN` bytes
/// - Uses `infer` on the prefix
/// - Uses `mime_guess` on the path
///
/// It does **not** interpret or reconcile the two results.
pub async fn sniff_mime(path: &Path) -> Result<MimeSniffResult, MimeSniffError> {
    let path_mime = mime_guess::from_path(path);

    /// Maximum number of bytes read for content sniffing.
    /// 8 KiB is more than enough for infer.
    const SNIFF_LEN: usize = 8 * 1024;

    let file = File::open(path)
        .await
        .with_context(|| format!("opening file for sniffing: '{}'", path.display()))?;
    let mut buf = vec![0u8; SNIFF_LEN];
    let _bytes_read = file
        .take(SNIFF_LEN as u64)
        .read_to_end(&mut buf)
        .await
        .context("reading file for sniffing")?;

    let content_mime = infer::get(&buf)
        .map(|kind| kind.mime_type())
        .filter(|mime| *mime != "application/octet-stream");

    Ok(MimeSniffResult {
        content_mime,
        path_mime,
    })
}
