use crate::CoreError;
use crate::domain::{
    DataSourceUuid, DiscKind, DiscSummary, EpisodeNumber, FileMeasurement, FileMeasurementRowId,
    FileMediaMeasurements, FileRole, MovieUuid, PathId, SeasonNumber, SubjectAuditLog,
    TvSeriesUuid,
};
use std::path::PathBuf;
use std::time::Duration;

/// The set of all measurements scoped to a single interpretation's ingest set.
pub struct Facts {
    pub files: Vec<(PathId, FileFacts)>,
}

pub struct FileFacts {
    pub row_id: FileMeasurementRowId,
    pub file: FileMeasurement,
    /// E.g. `video/x-matroska` or `image/webp`, if it could be told.
    pub mime_type: Option<String>,
    pub media: Option<FileMediaMeasurements>,
    /// The disc it stands for, if it's a disc's index file.
    pub disc: Option<DiscFacts>,
}

/// A disc, as its index file's facts carry it. See *Disc* in
/// `spec/domain-model.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscFacts {
    pub kind: DiscKind,
    /// The folder the disc structure is in, e.g. `Movie (2001)`: empty at the
    /// data source's root. A disc image's is the image, e.g. `Movie.iso`.
    pub root: String,
    /// How many files it has, its index file's among them.
    pub file_count: u64,
    /// All of its files', in bytes.
    pub size_bytes: u64,
    /// Its longest title's, if its files' durations were measured.
    pub duration: Option<Duration>,
}

impl DiscFacts {
    /// A disc from the columns a file's row joins in, or None if the file
    /// isn't a disc's index file.
    pub(crate) fn from_db(
        kind: Option<String>,
        root: Option<String>,
        member_count: Option<i64>,
        size: Option<i64>,
        duration_ms: Option<i64>,
    ) -> Result<Option<Self>, CoreError> {
        let Some(kind) = kind else {
            return Ok(None);
        };
        Ok(Some(Self {
            kind: DiscKind::from_db_str(&kind)?,
            root: root.unwrap_or_default(),
            file_count: member_count.unwrap_or_default() as u64 + 1,
            size_bytes: size.unwrap_or_default() as u64,
            duration: duration_ms.map(|ms| Duration::from_millis(ms as u64)),
        }))
    }
}

impl FileFacts {
    /// Whether it's a video, to catalog: a file with moving pictures, or a
    /// disc, which stands for its titles.
    pub fn is_video(&self) -> bool {
        self.disc.is_some() || self.has_video_streams()
    }

    /// Whether the file has moving pictures: not a still image, e.g. a scan,
    /// nor only cover art, e.g. a FLAC's embedded front cover.
    pub fn has_video_streams(&self) -> bool {
        let Some(media) = &self.media else {
            return false;
        };
        // ffprobe reports a still image as a video stream.
        if self
            .mime_type
            .as_deref()
            .is_some_and(|m| m.starts_with("image/"))
        {
            return false;
        }
        // Without a MIME type, ffprobe's image demuxers tell them, e.g.
        // `webp_pipe` or `image2`.
        if media
            .container
            .as_ref()
            .is_some_and(|c| c.format == "image2" || c.format.ends_with("_pipe"))
        {
            return false;
        }
        media
            .video
            .iter()
            .any(|v| !matches!(&*v.codec, "ansi" | "mjpeg" | "png"))
    }
}

/// Used for "shallow" file fact queries.
#[derive(Clone, Debug)]
pub struct FileSummary {
    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    pub rel_path: String,
    /// In bytes: a disc's, all of its files'.
    pub size: Option<i64>,
    /// The disc it stands for, if it's a disc's index file.
    pub disc: Option<DiscSummary>,
}

/// A file as one interpretation sees it: what was measured, how it was
/// identified, and what it is in the catalog.
pub struct FileDetails {
    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    pub rel_path: PathBuf,
    pub size: i64,
    /// UNIX timestamp.
    pub mtime: i64,
    pub mime_type: Option<String>,
    /// None when the file isn't media, or couldn't be probed.
    pub media: Option<FileMediaMeasurements>,
    /// None when no heuristic, assertion or knowledge base claimed anything
    /// about the file.
    pub audit_log: Option<SubjectAuditLog>,
    /// The movies the file is part of, by title.
    pub movies: Vec<FileMovie>,
    /// The episodes in the file, in order.
    pub episodes: Vec<FileEpisode>,
    /// The series the file is an extra of.
    pub series_extras: Vec<FileSeriesExtra>,
    /// The disc it stands for, if it's a disc's index file.
    pub disc: Option<DiscFacts>,
    /// The index file of the disc it's one of the other files of, which
    /// stands for it, if it is one.
    pub part_of_disc: Option<PathBuf>,
}

/// A file under a directory, as one interpretation placed it in its catalog.
pub struct DirectoryFile {
    pub rel_path: PathBuf,
    pub size: i64,
    /// The container's, when the file is media that has one.
    pub duration: Option<std::time::Duration>,
    /// Whether the interpretation recorded it as an unidentified video.
    pub unidentified_video: bool,
    /// The movies the file is part of, by title.
    pub movies: Vec<FileMovie>,
    /// The episodes in the file, in order.
    pub episodes: Vec<FileEpisode>,
    /// The series the file is an extra of.
    pub series_extras: Vec<FileSeriesExtra>,
}

pub struct FileMovie {
    pub uuid: MovieUuid,
    pub title: String,
    pub release_year: Option<i64>,
    /// None when nothing says what the file's role is.
    pub role: Option<FileRole>,
    /// For a main file, which of the movie's manifestations it's in, numbered from 0.
    /// See *Manifestation* in `spec/domain-model.md`.
    pub manifestation: Option<usize>,
    /// For a main file, which part of its manifestation it is, if its name numbers it
    /// as one, e.g. 1 for `CD1`.
    pub part: Option<u32>,
}

/// A series that a file is an extra of.
pub struct FileSeriesExtra {
    pub series_uuid: TvSeriesUuid,
    pub series_name: String,
    /// Whether it's the series' disc, rather than an extra.
    pub disc: bool,
}

pub struct FileEpisode {
    pub series_uuid: TvSeriesUuid,
    pub series_name: String,
    /// None when the season is unknown, and the episode is numbered through
    /// the whole series.
    pub season_number: Option<SeasonNumber>,
    pub episode_number: EpisodeNumber,
    pub title: Option<String>,
}
