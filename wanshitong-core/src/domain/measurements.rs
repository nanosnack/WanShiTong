use crate::domain::DiscKind;
use ffprobe_types as ffp;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct FileMeasurementRowId(pub i64);

#[derive(Clone, Debug)]
pub struct FileMeasurement {
    pub device: Option<u64>,
    pub inode: Option<u64>,
    pub size_bytes: u64,
    pub mtime: i64,
    pub ctime: Option<i64>,
    pub hardlink_count: Option<u64>,
    pub is_symlink: bool,
    pub media_measurement_id: Option<FileMediaMeasurementRowId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct FileMediaMeasurementRowId(pub i64);

/// The goal of this type is to abstract over our media probing tool output
/// (ffprobe) and make media measurements as ergonomic as possible for
/// heuristics to use without losing information.
///
/// We serialize this as a JSON BLOB before storing in SQL for mostly ergonomic
/// reasons:
/// - Accurately representing all measurement fields in SQL is verbose
/// - There is no need for fine-grained querying of these measurement fields
/// - It is simplest to fetch *all* measurements for heuristics, and memory
///   usage should be reasonable even for large media libraries
///
/// We leave fine-grained querying for where it makes a UX difference: the
/// *catalog*.
///
#[derive(Debug, Serialize, Deserialize)]
pub struct FileMediaMeasurements {
    pub container: Option<ContainerMeasurement>,
    pub video: Vec<VideoStreamMeasurement>,
    pub audio: Vec<AudioStreamMeasurement>,
    pub text: Vec<TextStreamMeasurement>,
    pub unclassified: Vec<ffp::Stream>,
    /// The kind of disc the file is an image of, e.g. a Blu-ray's `.iso`,
    /// whose container and streams are then its longest title's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disc_image: Option<DiscKind>,
}

impl FileMediaMeasurements {
    pub fn duration(&self) -> Option<Duration> {
        self.container.as_ref()?.duration()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerMeasurement {
    /// ffprobe’s primary container name
    pub format: String,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<i64>,
    pub stream_count: i32,

    pub tags: Option<HashMap<String, String>>,
}

impl ContainerMeasurement {
    pub fn duration(&self) -> Option<Duration> {
        self.duration_ms.map(|ms| Duration::from_millis(ms as u64))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoStreamMeasurement {
    pub stream_index: i32,

    pub codec: String,
    pub profile: Option<String>,
    pub width: u32,
    pub height: u32,
    pub pixel_format: Option<String>,

    pub color_primaries: Option<String>,
    pub color_transfer: Option<String>,
    pub color_space: Option<String>,
    pub color_range: Option<String>,

    pub frame_rate_num: Option<i32>,
    pub frame_rate_den: Option<i32>,
    pub bit_depth: Option<i32>,
    pub bitrate: Option<i64>,
    pub duration_ms: Option<i64>,

    pub tags: Option<HashMap<String, String>>,
    pub side_data_list: Vec<ffp::VideoSideData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AudioStreamMeasurement {
    pub stream_index: i32,

    pub codec: String,
    pub channels: u32,
    pub channel_layout: Option<String>,
    pub sample_rate: Option<i32>,
    pub bit_depth: Option<i32>,
    pub bitrate: Option<i64>,
    pub duration_ms: Option<i64>,
    pub language: Option<String>,

    pub tags: Option<HashMap<String, String>>,
    pub side_data_list: Vec<ffp::AudioSideData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextStreamMeasurement {
    pub stream_index: i32,

    pub codec: String,
    pub language: Option<String>,
    pub is_forced: Option<bool>,
    pub is_default: Option<bool>,

    pub tags: Option<HashMap<String, String>>,
}

impl FileMediaMeasurements {
    pub fn from_ffprobe(ffprobe: &ffp::FFProbeOutput) -> FileMediaMeasurements {
        let container = ContainerMeasurement::from_ffprobe(ffprobe);
        let mut video = Vec::new();
        let mut audio = Vec::new();
        let mut text = Vec::new();
        let mut unclassified = Vec::new();
        for stream in &ffprobe.streams {
            if let Some(codec_type) = &stream.codec_type {
                match codec_type.as_str() {
                    "video" => {
                        if let Some(m) = VideoStreamMeasurement::from_ffprobe(stream) {
                            video.push(m);
                        }
                    }
                    "audio" => {
                        let m = AudioStreamMeasurement::from_ffprobe(stream);
                        audio.push(m);
                    }
                    "subtitle" => {
                        let m = TextStreamMeasurement::from_ffprobe(stream);
                        text.push(m);
                    }
                    _ => {
                        unclassified.push(stream.clone());
                    }
                }
            } else {
                unclassified.push(stream.clone());
            }
        }
        Self {
            container,
            video,
            audio,
            text,
            unclassified,
            disc_image: None,
        }
    }
}

impl ContainerMeasurement {
    fn from_ffprobe(ffprobe: &ffp::FFProbeOutput) -> Option<Self> {
        let format = ffprobe
            .format
            .as_ref()?
            .format_name
            .as_ref()?
            .split(',')
            .next()?
            .to_string();

        let duration_ms = ffprobe
            .format
            .as_ref()
            .and_then(|f| parse_duration_ms(&f.duration));

        let bitrate = ffprobe.format.as_ref().and_then(|f| parse_i64(&f.bit_rate));

        let tags = ffprobe.format.as_ref().and_then(|f| f.tags.clone());

        Some(Self {
            format,
            duration_ms,
            bitrate,
            stream_count: ffprobe.streams.len() as i32,
            tags,
        })
    }
}

impl VideoStreamMeasurement {
    fn from_ffprobe(stream: &ffp::Stream) -> Option<VideoStreamMeasurement> {
        let (width, height) = match (stream.width, stream.height) {
            (Some(w), Some(h)) => (w, h),
            _ => return None, // skip malformed video streams
        };
        let (fr_num, fr_den) = parse_rational(&stream.avg_frame_rate).unwrap_or((0, 0));
        let m = Self {
            stream_index: stream.index,

            codec: stream
                .codec_name
                .clone()
                .unwrap_or_else(|| "unknown".into()),
            profile: stream.profile.clone(),
            width,
            height,
            pixel_format: stream.pix_fmt.clone(),

            color_primaries: stream.color_primaries.clone(),
            color_transfer: stream.color_transfer.clone(),
            color_space: stream.color_space.clone(),
            color_range: stream.color_range.clone(),

            frame_rate_num: if fr_den != 0 { Some(fr_num) } else { None },
            frame_rate_den: if fr_den != 0 { Some(fr_den) } else { None },
            bit_depth: parse_i32(&stream.bits_per_raw_sample),
            bitrate: parse_i64(&stream.bit_rate),
            duration_ms: parse_duration_ms(&stream.duration),

            tags: stream.tags.clone(),
            side_data_list: stream
                .side_data_list
                .iter()
                .cloned()
                .filter_map(|d| d.video())
                .collect(),
        };
        Some(m)
    }
}

impl AudioStreamMeasurement {
    fn from_ffprobe(stream: &ffp::Stream) -> AudioStreamMeasurement {
        Self {
            stream_index: stream.index,

            codec: stream
                .codec_name
                .clone()
                .unwrap_or_else(|| "unknown".into()),
            channels: stream.channels.unwrap_or(0),
            channel_layout: stream.channel_layout.clone(),
            sample_rate: parse_i32(&stream.sample_rate),
            bit_depth: parse_i32(&stream.bits_per_raw_sample),
            bitrate: parse_i64(&stream.bit_rate),
            duration_ms: parse_duration_ms(&stream.duration),
            language: stream
                .tags
                .as_ref()
                .and_then(|t| t.get("language"))
                .cloned(),

            tags: stream.tags.clone(),
            side_data_list: stream
                .side_data_list
                .iter()
                .cloned()
                .filter_map(|d| d.audio())
                .collect(),
        }
    }
}

impl TextStreamMeasurement {
    fn from_ffprobe(stream: &ffp::Stream) -> TextStreamMeasurement {
        let disp = stream.disposition.as_ref();

        TextStreamMeasurement {
            stream_index: stream.index,

            codec: stream
                .codec_name
                .clone()
                .unwrap_or_else(|| "unknown".into()),
            language: stream
                .tags
                .as_ref()
                .and_then(|t| t.get("language"))
                .cloned(),
            is_forced: disp.map(|d| d.forced != 0),
            is_default: disp.map(|d| d.default != 0),

            tags: stream.tags.clone(),
        }
    }
}

fn parse_i64(s: &Option<String>) -> Option<i64> {
    s.as_ref()?.parse().ok()
}

fn parse_i32(s: &Option<String>) -> Option<i32> {
    s.as_ref()?.parse().ok()
}

fn parse_duration_ms(s: &Option<String>) -> Option<i64> {
    let secs: f64 = s.as_ref()?.parse().ok()?;
    Some((secs * 1000.0).round() as i64)
}

fn parse_rational(s: &Option<String>) -> Option<(i32, i32)> {
    let s = s.as_ref()?;
    let (n, d) = s.split_once('/')?;
    Some((n.parse().ok()?, d.parse().ok()?))
}

/// Every version media measurements have been stored as.
#[derive(Deserialize)]
pub(crate) enum VersionedFileMediaMeasurements {
    V1(FileMediaMeasurements),
}

impl From<VersionedFileMediaMeasurements> for FileMediaMeasurements {
    fn from(VersionedFileMediaMeasurements::V1(media): VersionedFileMediaMeasurements) -> Self {
        media
    }
}

impl crate::json::Stored for FileMediaMeasurements {
    type Versions = VersionedFileMediaMeasurements;
    const VERSION: &'static str = "V1";
}
