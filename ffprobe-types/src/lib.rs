#![doc = include_str!("../README.md")]

use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Last tested against ffprobe version 8.0.1.
#[derive(Clone, Debug, Deserialize)]
pub struct FFProbeOutput {
    #[serde(default)]
    pub format: Option<Format>,

    #[serde(default)]
    pub streams: Vec<Stream>,

    #[serde(default)]
    pub chapters: Vec<Chapter>,

    #[serde(default)]
    pub programs: Vec<Program>,
}

impl FFProbeOutput {
    pub fn is_empty(&self) -> bool {
        self.format.is_none()
            && self.streams.is_empty()
            && self.chapters.is_empty()
            && self.programs.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Format {
    #[allow(unused)]
    pub filename: String,

    #[serde(default)]
    pub format_name: Option<String>,

    #[allow(unused)]
    #[serde(default)]
    pub format_long_name: Option<String>,

    #[serde(default)]
    pub duration: Option<String>,

    #[allow(unused)]
    #[serde(default)]
    pub size: Option<String>,

    #[serde(default)]
    pub bit_rate: Option<String>,

    #[allow(unused)]
    #[serde(default)]
    pub probe_score: Option<u32>,

    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stream {
    pub index: i32,

    pub codec_type: Option<String>, // "video" | "audio" | "subtitle" | others
    #[serde(default)]
    pub codec_name: Option<String>,
    #[allow(unused)]
    #[serde(default)]
    pub codec_long_name: Option<String>,

    #[serde(default)]
    pub profile: Option<String>,

    // --- video-specific (optional) ---
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub pix_fmt: Option<String>,
    #[serde(default)]
    pub color_space: Option<String>,
    #[serde(default)]
    pub color_transfer: Option<String>,
    #[serde(default)]
    pub color_primaries: Option<String>,
    #[serde(default)]
    pub color_range: Option<String>,
    #[serde(default)]
    pub bits_per_raw_sample: Option<String>,

    // --- audio-specific (optional) ---
    #[serde(default)]
    pub sample_rate: Option<String>, // ffprobe reports as string
    #[serde(default)]
    pub channels: Option<u32>,
    #[serde(default)]
    pub channel_layout: Option<String>,

    // --- timing ---
    #[serde(default)]
    pub duration: Option<String>, // seconds, string
    #[serde(default)]
    pub bit_rate: Option<String>,
    #[serde(default)]
    pub avg_frame_rate: Option<String>, // "30000/1001"
    #[allow(unused)]
    #[serde(default)]
    pub r_frame_rate: Option<String>,

    // --- disposition flags ---
    #[serde(default)]
    pub disposition: Option<Disposition>,

    // --- raw metadata ---
    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,

    // --- codec private / side data ---
    #[serde(default)]
    pub side_data_list: Vec<SideData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Disposition {
    pub default: i32,
    pub forced: i32,
    #[allow(unused)]
    pub hearing_impaired: i32,
    #[allow(unused)]
    pub visual_impaired: i32,
    #[allow(unused)]
    pub clean_effects: i32,
    #[allow(unused)]
    pub attached_pic: i32,
    #[allow(unused)]
    pub timed_thumbnails: i32,
}

// TODO: capture this in measurements
#[allow(unused)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chapter {
    pub id: i64,

    #[serde(default)]
    pub start_time: Option<String>,

    #[serde(default)]
    pub end_time: Option<String>,

    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,
}

// TODO: capture this in measurements
#[allow(unused)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Program {
    pub program_id: i32,

    #[serde(default)]
    pub program_num: Option<i32>,

    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,

    #[serde(default)]
    pub streams: Vec<Stream>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "side_data_type")]
pub enum SideData {
    // Video
    #[serde(rename = "Display Matrix")]
    DisplayMatrix(DisplayMatrix),
    #[serde(rename = "Mastering display metadata")]
    MasteringDisplayMetadata(MasteringDisplayMetadata),
    #[serde(rename = "Content light level metadata")]
    ContentLightLevel(ContentLightLevel),
    #[serde(rename = "Stereo 3D")]
    Stereo3D(Stereo3D),
    #[serde(rename = "Dolby Vision Metadata")]
    DolbyVision(DolbyVisionMetadata),
    #[serde(rename = "Closed Captions")]
    ClosedCaptions(ClosedCaptions),

    // Audio
    #[serde(rename = "Skip Samples")]
    SkipSamples(SkipSamples),
    #[serde(rename = "Matrix Encoding")]
    MatrixEncoding(MatrixEncoding),

    #[serde(other)]
    Unknown,
}

impl SideData {
    pub fn video(self) -> Option<VideoSideData> {
        match self {
            SideData::DisplayMatrix(d) => Some(VideoSideData::DisplayMatrix(d)),
            SideData::MasteringDisplayMetadata(d) => {
                Some(VideoSideData::MasteringDisplayMetadata(d))
            }
            SideData::ContentLightLevel(d) => Some(VideoSideData::ContentLightLevel(d)),
            SideData::Stereo3D(d) => Some(VideoSideData::Stereo3D(d)),
            SideData::DolbyVision(d) => Some(VideoSideData::DolbyVision(d)),
            SideData::ClosedCaptions(d) => Some(VideoSideData::ClosedCaptions(d)),

            _ => None,
        }
    }

    pub fn audio(self) -> Option<AudioSideData> {
        match self {
            SideData::SkipSamples(d) => Some(AudioSideData::SkipSamples(d)),
            SideData::MatrixEncoding(d) => Some(AudioSideData::MatrixEncoding(d)),

            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum VideoSideData {
    DisplayMatrix(DisplayMatrix),
    MasteringDisplayMetadata(MasteringDisplayMetadata),
    ContentLightLevel(ContentLightLevel),
    Stereo3D(Stereo3D),
    DolbyVision(DolbyVisionMetadata),
    ClosedCaptions(ClosedCaptions),

    Unknown {
        kind: String,
        payload: Option<Vec<u8>>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AudioSideData {
    SkipSamples(SkipSamples),
    MatrixEncoding(MatrixEncoding),

    Unknown {
        kind: String,
        payload: Option<Vec<u8>>,
    },
}

/// Rotation / display transform information.
///
/// Commonly derived from a display matrix.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisplayMatrix {
    /// Clockwise rotation in degrees.
    ///
    /// Common values: 0, 90, 180, 270.
    pub rotation_degrees: i32,

    /// Whether the image is horizontally flipped.
    pub flip_horizontal: bool,

    /// Whether the image is vertically flipped.
    pub flip_vertical: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chromaticity {
    pub x: f32,
    pub y: f32,
}

/// HDR mastering display metadata (SMPTE ST 2086).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MasteringDisplayMetadata {
    /// RGB primaries, in order: R, G, B.
    pub primaries: [Chromaticity; 3],

    /// White point chromaticity.
    pub white_point: Chromaticity,

    /// Maximum display luminance (cd/m²).
    pub max_luminance: f32,

    /// Minimum display luminance (cd/m²).
    pub min_luminance: f32,
}

/// HDR content light level metadata (CEA-861.3).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentLightLevel {
    /// Maximum content light level (MaxCLL), in cd/m².
    pub max_content_light_level: u16,

    /// Maximum frame-average light level (MaxFALL), in cd/m².
    pub max_frame_average_light_level: u16,
}

/// Stereo 3D layout information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stereo3D {
    pub layout: Stereo3DLayout,
    pub view_packing: Option<Stereo3DViewPacking>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Stereo3DLayout {
    Mono,
    SideBySide,
    TopBottom,
    LineByLine,
    ColumnInterleaved,
    Checkerboard,
    Anaglyph,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Stereo3DViewPacking {
    LeftFirst,
    RightFirst,
}

/// Dolby Vision metadata (container-level summary as exposed by ffprobe).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DolbyVisionMetadata {
    pub profile: u8,
    pub level: u8,

    /// Optional flags or additional information ffprobe may expose.
    pub rpu_present: bool,
}

/// Closed captions embedded in video (e.g. EIA-608/708).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClosedCaptions {
    /// Caption standard, if known.
    pub format: ClosedCaptionFormat,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClosedCaptionFormat {
    Eia608,
    Eia708,
    Unknown,
}

/// Number of samples to skip at start and/or end of the stream.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkipSamples {
    /// Number of samples to skip at the start.
    pub start: u32,

    /// Number of samples to skip at the end.
    pub end: u32,
}

/// Audio matrix encoding information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatrixEncoding {
    pub mode: AudioMatrixMode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AudioMatrixMode {
    None,
    Dolby,
    DolbyProLogic,
    DolbyProLogicII,
    DolbyProLogicIIx,
    DolbyProLogicIIz,
    Unknown,
}
