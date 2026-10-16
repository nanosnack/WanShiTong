//! Interpreting files known by their paths alone, to score the heuristics
//! against a corpus of releases that was never ingested, e.g. a tracker's
//! listings of what each release holds.
//!
//! What an ingest measures is missing: nothing has a duration, so samples go
//! undetected, numbered runs aren't told to be episodes by their length, and a
//! movie directory of several videos has no longest one to pick as main.

use crate::interpret::{
    interpret_facts,
    knowledge_base::{KnowledgeBase, KnowledgeBaseApi},
};
use crate::knowledge_base_types::Tmdb;
use crate::title;
use crate::tmdb_client::TmdbClient;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use uuid::Uuid;
use wanshitong_core::domain::{
    ContainerMeasurement, DataSourceUuid, DiscFacts, ExternalId, FileFacts, FileMeasurement,
    FileMeasurementRowId, FileMediaMeasurements, PathId, VideoStreamMeasurement, detect_discs,
};

pub use wanshitong_core::commands::{
    NewCatalog, NewEpisode, NewInterpretationError, NewMovie, NewMovieFile, NewTvSeries,
};

/// A release's file: its path from the release's data source's root, with
/// `/` between folders, and its size.
pub struct CorpusFile {
    pub path: String,
    pub size_bytes: u64,
}

/// Video files' extensions that MIME types don't tell, e.g. a Blu-ray's
/// streams and a DVD's title sets, to be safe.
const VIDEO_EXTENSIONS: &[&str] = &[
    "avi", "divx", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "mts", "ogm", "rmvb",
    "ts", "vob", "webm", "wmv",
];

/// The facts of a release's files, as an ingest would find them if ffprobe
/// could read none: a video, told by its extension, has one video stream and
/// no duration; a disc's index file stands for the disc, and its other files
/// are left out, as from an ingest. Row IDs are the files' positions in
/// `files`, from 1.
pub fn facts_from_paths(files: &[CorpusFile]) -> Vec<(PathId, FileFacts)> {
    let indexed: Vec<(usize, &str)> = files
        .iter()
        .enumerate()
        .map(|(i, file)| (i, file.path.as_str()))
        .collect();
    let discs = detect_discs(&indexed);
    let mut disc_of_index = HashMap::new();
    let mut members = HashSet::new();
    for disc in discs {
        let size_bytes = disc
            .members
            .iter()
            .chain([&disc.index])
            .map(|&i| files[i].size_bytes)
            .sum();
        members.extend(disc.members.iter().copied());
        disc_of_index.insert(
            disc.index,
            DiscFacts {
                kind: disc.kind,
                root: disc.root,
                file_count: disc.members.len() as u64 + 1,
                size_bytes,
                duration: None,
            },
        );
    }

    files
        .iter()
        .enumerate()
        .filter(|(i, _)| !members.contains(i))
        .map(|(i, file)| {
            let disc = disc_of_index.remove(&i);
            let mime_type = mime_guess::from_path(&file.path)
                .first()
                .map(|mime| mime.essence_str().to_string());
            let extension = file
                .path
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase())
                .unwrap_or_default();
            let is_video = mime_type
                .as_deref()
                .is_some_and(|m| m.starts_with("video/"))
                || VIDEO_EXTENSIONS.contains(&extension.as_str());
            let media = (disc.is_none() && is_video).then(|| FileMediaMeasurements {
                container: Some(ContainerMeasurement {
                    format: extension.clone(),
                    duration_ms: None,
                    bitrate: None,
                    stream_count: 1,
                    tags: None,
                }),
                video: vec![VideoStreamMeasurement {
                    stream_index: 0,
                    codec: "unknown".into(),
                    profile: None,
                    width: 0,
                    height: 0,
                    pixel_format: None,
                    color_primaries: None,
                    color_transfer: None,
                    color_space: None,
                    color_range: None,
                    frame_rate_num: None,
                    frame_rate_den: None,
                    bit_depth: None,
                    bitrate: None,
                    duration_ms: None,
                    tags: None,
                    side_data_list: vec![],
                }],
                audio: vec![],
                text: vec![],
                unclassified: vec![],
                disc_image: None,
            });
            let path_id = PathId {
                data_source: DataSourceUuid(Uuid::nil()),
                rel_path: PathBuf::from(&file.path),
            };
            let facts = FileFacts {
                row_id: FileMeasurementRowId(i as i64 + 1),
                file: FileMeasurement {
                    device: None,
                    inode: None,
                    size_bytes: file.size_bytes,
                    mtime: 0,
                    ctime: None,
                    hardlink_count: None,
                    is_symlink: false,
                    media_measurement_id: None,
                },
                mime_type,
                media,
                disc,
            };
            (path_id, facts)
        })
        .collect()
}

/// A knowledge base to interpret with.
pub struct Kb(Tmdb);

impl Kb {
    /// TMDB, with its responses cached in `cache_dir`, so that a corpus is
    /// searched for once, and at most `per_second` requests a second sent to
    /// it, so that a corpus searched in bulk doesn't strain it.
    pub fn tmdb(cache_dir: PathBuf, token: String, per_second: f64) -> Self {
        let interval = std::time::Duration::from_secs_f64(1.0 / per_second);
        Self(Tmdb::new(TmdbClient::with_rate_limit(
            cache_dir, token, interval,
        )))
    }

    /// A movie's title and release year, by its TMDB ID.
    pub async fn movie_title(&self, tmdb_id: u32) -> anyhow::Result<Option<(String, Option<u32>)>> {
        let movie = self
            .0
            .movie(&ExternalId::Tmdb(tmdb_id), &mut vec![])
            .await?;
        Ok(movie.map(|movie| (movie.title, movie.release_year)))
    }

    /// A movie's runtime in minutes, by its TMDB ID, if TMDB lists one.
    pub async fn movie_runtime(&self, tmdb_id: u32) -> anyhow::Result<Option<u32>> {
        let movie = self
            .0
            .movie(&ExternalId::Tmdb(tmdb_id), &mut vec![])
            .await?;
        Ok(movie.and_then(|movie| movie.runtime_minutes))
    }

    /// A series' name and the year it first aired, by its TMDB ID.
    pub async fn series_name(&self, tmdb_id: u32) -> anyhow::Result<Option<(String, Option<u32>)>> {
        let series = self
            .0
            .series(&ExternalId::Tmdb(tmdb_id), &mut vec![])
            .await?;
        Ok(series.map(|series| (series.name, series.first_air_year)))
    }
}

/// Gives a release's main video `duration`, as if measured: its largest
/// disc's, or else its largest video file's. The rest stay unmeasured.
pub fn measure_main_video(facts: &mut [(PathId, FileFacts)], duration: std::time::Duration) {
    let size = |facts: &FileFacts| facts.file.size_bytes;
    let main = facts
        .iter_mut()
        .filter(|(_, facts)| facts.disc.is_some())
        .max_by_key(|(_, facts)| facts.disc.as_ref().map(|disc| disc.size_bytes))
        .map(|(_, facts)| facts);
    if let Some(facts) = main {
        if let Some(disc) = &mut facts.disc {
            disc.duration = Some(duration);
        }
        return;
    }
    let main = facts
        .iter_mut()
        .filter(|(_, facts)| facts.media.is_some())
        .max_by_key(|(_, facts)| size(facts));
    if let Some((_, facts)) = main
        && let Some(container) = facts
            .media
            .as_mut()
            .and_then(|media| media.container.as_mut())
    {
        container.duration_ms = Some(duration.as_millis() as i64);
    }
}

#[cfg(feature = "corpus")]
pub use crate::interpret::LastResortProbe;

/// What each last resort finds alone for up to `max_videos` of `files`'
/// videos that only the last resorts could find.
#[cfg(feature = "corpus")]
pub async fn probe_last_resorts(
    files: &[(PathId, FileFacts)],
    kb: &Kb,
    max_videos: usize,
) -> anyhow::Result<Vec<LastResortProbe>> {
    Ok(crate::interpret::probe_last_resorts(files, &kb.0, max_videos).await?)
}

/// The catalog of `files`, every work in it whether identified or not, from
/// the heuristics alone or with the knowledge base. What a search couldn't
/// do goes in `errors`, as in an interpretation.
pub async fn interpret(
    files: &[(PathId, FileFacts)],
    kb: Option<&Kb>,
    errors: &mut Vec<NewInterpretationError>,
) -> anyhow::Result<NewCatalog> {
    let kb = kb.map(|kb| &kb.0 as &dyn KnowledgeBase);
    Ok(interpret_facts(files, &[], kb, errors).await?)
}

/// Whether two titles are the same words in the same order, whatever their
/// case and punctuation.
pub fn same_title(a: &str, b: &str) -> bool {
    title::same_words(a, b)
}
