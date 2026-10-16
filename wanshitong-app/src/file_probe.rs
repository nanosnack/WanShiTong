mod disc_image;
mod ffprobe;
mod mime;
mod nfo;

pub use ffprobe::*;
pub use mime::*;
pub use nfo::*;

use crate::file_probe::disc_image::is_disc_image;
use crate::file_probe::mime::sniff_mime;
use ffprobe_types::FFProbeOutput;
use std::path::Path;
use tokio::sync::Semaphore;
use wanshitong_core::domain::DiscKind;

// TODO: we are intentionally a bit lossy with the measurement, i.e. we collapse
// 3 different MIME signals into one guess here, which should really happen
// *after* measurement during interpretation. I'm just being lazy and hoping it
// doesn't really matter.
//
pub struct FileProbeResult {
    pub mime: Option<String>,
    pub ffprobe: Option<FFProbeResult>,
    /// The kind of disc the file is an image of, if ffprobe could read it as
    /// one: its output is then the disc's longest title's.
    pub disc_image: Option<DiscKind>,
}

/// How many files are probed at once, across every ingest. Each probe opens
/// the file and usually runs an ffprobe process, so this keeps the process
/// from hitting "too many open files".
///
/// Each ingest also allows only this many of its probes in flight at once.
/// The shared semaphore is first in, first out, so an ingest that queued
/// thousands of probes would otherwise be served to completion before a
/// later ingest's first file, however small that ingest is. With each
/// ingest's in-flight probes capped at the permit count, the queue cycles
/// through the running ingests in rounds of at most that many files, while
/// a lone ingest can still hold every permit. The two limits must stay equal:
/// a smaller per-ingest cap would leave permits idle unless several ingests
/// ran at once, and a larger one would bring back the starvation.
pub const MAX_CONCURRENT_PROBES: usize = 32;

pub async fn probe_file(
    ffprobe_tool: &FFProbeTool,
    path: &Path,
) -> anyhow::Result<FileProbeResult> {
    static SEMAPHORE: Semaphore = Semaphore::const_new(MAX_CONCURRENT_PROBES);
    let _permit = SEMAPHORE
        .acquire()
        .await
        .expect("we never close this semaphore");

    let mime = sniff_mime(path).await?;

    // An NFO is text, which ffprobe would read as ANSI art, and give a
    // duration. Its text is read apart, as a measurement of its own.
    if is_nfo_path(path) {
        return Ok(FileProbeResult {
            mime: preferred_mime(&mime, None),
            ffprobe: None,
            disc_image: None,
        });
    }

    // A disc image's MIME type is the image's, e.g. `application/x-iso9660-image`,
    // not its title's container's. Only Blu-rays' can be read so far, as our
    // ffprobe has libbluray but not libdvdnav.
    if is_disc_image(path).await? {
        let result = ffprobe_tool.probe_blu_ray_image(path).await?;
        if !result.output.is_empty() {
            return Ok(FileProbeResult {
                mime: preferred_mime(&mime, None),
                ffprobe: Some(result),
                disc_image: Some(DiscKind::BluRay),
            });
        }
    }

    let ffprobe = if should_use_ffprobe(&mime) {
        let result = ffprobe_tool.probe_file(path).await?;
        (!result.output.is_empty()).then_some(result)
    } else {
        None
    };
    let mime = preferred_mime(&mime, ffprobe.as_ref().map(|result| &result.output));
    Ok(FileProbeResult {
        mime,
        ffprobe,
        disc_image: None,
    })
}

fn preferred_mime(
    sniff_result: &MimeSniffResult,
    ffprobe_output: Option<&FFProbeOutput>,
) -> Option<String> {
    let MimeSniffResult {
        content_mime,
        path_mime,
    } = sniff_result;
    if let Some(mime) = ffprobe_output.and_then(mime_from_ffprobe) {
        Some(mime.into())
    } else if let Some(content_mime) = *content_mime {
        Some(content_mime.into())
    } else {
        path_mime.first().map(|first| first.essence_str().into())
    }
}

fn should_use_ffprobe(mime: &MimeSniffResult) -> bool {
    match mime.content_mime {
        Some(content_mime)
            if mime
                .path_mime
                .iter()
                .any(|guess| guess.essence_str() == content_mime) =>
        {
            // Path and content MIMEs match, so we can be confident they are correct.
            content_mime.starts_with("video")
                || content_mime.starts_with("audio")
                || content_mime.starts_with("image")
        }
        Some(_) => {
            // Path and content MIMEs don't match. We'll run ffprobe to be safe.
            true
        }
        None => {
            // No content MIME at all, we won't only trust the file extension.
            // Run ffprobe to be safe.
            true
        }
    }
}

/// Derives a best-effort MIME type from ffprobe output.
///
/// Returns None if no safe, conventional mapping exists.
/// This function does not attempt codec-based MIME inference.
///
fn mime_from_ffprobe(ff: &FFProbeOutput) -> Option<&'static str> {
    let format = ff
        .format
        .as_ref()?
        .format_name
        .as_ref()?
        .split(',')
        .next()?;

    let has_video = ff
        .streams
        .iter()
        .any(|s| s.codec_type.as_deref() == Some("video"));

    Some(match (format, has_video) {
        ("mp4" | "mov", true) => "video/mp4",
        ("mp4" | "mov", false) => "audio/mp4",
        ("matroska", true) => "video/x-matroska",
        ("matroska", false) => "audio/x-matroska",
        ("webm", _) => "video/webm",
        ("mpegts", _) => "video/mp2t",
        ("mp3", _) => "audio/mpeg",
        ("flac", _) => "audio/flac",
        _ => return None,
    })
}
