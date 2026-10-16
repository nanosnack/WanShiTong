//! Discs: disc structures among an ingest's files, e.g. a ripped Blu-ray or
//! DVD, which are indivisible from ingest on. See *Disc* in
//! `spec/domain-model.md`.

use crate::{CoreError, CoreErrorContext};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DiscKind {
    /// A `BDMV` folder, with `index.bdmv`.
    BluRay,
    /// A `VIDEO_TS` folder, with `VIDEO_TS.IFO`.
    Dvd,
}

impl DiscKind {
    pub(crate) fn as_db_str(self) -> &'static str {
        match self {
            Self::BluRay => "bluray",
            Self::Dvd => "dvd",
        }
    }

    pub(crate) fn from_db_str(s: &str) -> Result<Self, CoreError> {
        match s {
            "bluray" => Ok(Self::BluRay),
            "dvd" => Ok(Self::Dvd),
            _ => Err(CoreErrorContext::UnknownDiscKind(s.to_string()).into()),
        }
    }
}

/// A disc, as a listing of files shows it: what kind it is, and how long it
/// plays, if its files' durations were measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiscSummary {
    pub kind: DiscKind,
    /// How many files it has, its index file among them.
    pub file_count: u64,
    pub duration: Option<Duration>,
}

impl DiscSummary {
    pub(crate) fn from_db(
        kind: Option<String>,
        member_count: Option<i64>,
        duration_ms: Option<i64>,
    ) -> Result<Option<Self>, CoreError> {
        let Some(kind) = kind else {
            return Ok(None);
        };
        Ok(Some(Self {
            kind: DiscKind::from_db_str(&kind)?,
            file_count: member_count.unwrap_or_default() as u64 + 1,
            duration: duration_ms.map(|ms| Duration::from_millis(ms as u64)),
        }))
    }
}

/// A disc found among a data source's files, by the files' IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedDisc<Id> {
    pub kind: DiscKind,
    /// The folder the disc structure is in, e.g. `Movie (2001)` for
    /// `Movie (2001)/BDMV/index.bdmv`: empty at the data source's root.
    pub root: String,
    /// `index.bdmv` or `VIDEO_TS.IFO`, which stands for the disc.
    pub index: Id,
    /// The disc's other files, e.g. its streams and playlists.
    pub members: Vec<Id>,
}

/// The discs among one data source's files, each its ID and its path, with
/// `/` between folders. Detection is strict: a partial structure, e.g. a
/// `BDMV` folder without `index.bdmv` or without streams, isn't a disc.
pub fn detect_discs<Id: Copy>(files: &[(Id, &str)]) -> Vec<DetectedDisc<Id>> {
    let mut discs = detect_blu_rays(files);
    discs.extend(detect_dvds(files));
    discs
}

/// The folder a path's file is in, and its name.
fn split(path: &str) -> (&str, &str) {
    match path.rsplit_once('/') {
        Some((folder, name)) => (folder, name),
        None => ("", path),
    }
}

/// A Blu-ray is the files under a `BDMV` folder, with `BDMV/index.bdmv` and at
/// least one stream in `BDMV/STREAM`, and those under the `CERTIFICATE`
/// folder beside it.
fn detect_blu_rays<Id: Copy>(files: &[(Id, &str)]) -> Vec<DetectedDisc<Id>> {
    #[derive(Default)]
    struct Group<Id> {
        index: Option<Id>,
        has_stream: bool,
        members: Vec<Id>,
    }
    // By the folder the `BDMV` folder is in.
    let mut groups = BTreeMap::<String, Group<Id>>::new();
    for &(id, path) in files {
        let segments: Vec<&str> = path.split('/').collect();
        // The nearest `BDMV` or `CERTIFICATE` folder the file is in.
        let Some(position) = segments[..segments.len() - 1].iter().rposition(|segment| {
            segment.eq_ignore_ascii_case("BDMV") || segment.eq_ignore_ascii_case("CERTIFICATE")
        }) else {
            continue;
        };
        let root = segments[..position].join("/");
        let group = groups.entry(root).or_insert_with(|| Group {
            index: None,
            has_stream: false,
            members: Vec::new(),
        });
        let within = &segments[position + 1..];
        let in_bdmv = segments[position].eq_ignore_ascii_case("BDMV");
        if in_bdmv && within.len() == 1 && within[0].eq_ignore_ascii_case("index.bdmv") {
            group.index = Some(id);
        } else {
            if in_bdmv && within.len() == 2 && within[0].eq_ignore_ascii_case("STREAM") {
                group.has_stream = true;
            }
            group.members.push(id);
        }
    }
    groups
        .into_iter()
        .filter_map(|(root, group)| {
            let index = group.index.filter(|_| group.has_stream)?;
            Some(DetectedDisc {
                kind: DiscKind::BluRay,
                root,
                index,
                members: group.members,
            })
        })
        .collect()
}

/// A DVD's file, by its name: `VIDEO_TS.IFO`, `.BUP` or `.VOB`, or a title
/// set's, e.g. `VTS_01_1.VOB`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DvdFile {
    Index,
    VideoTs,
    TitleSet { title_set: u8, part: u8, vob: bool },
}

fn dvd_file(name: &str) -> Option<DvdFile> {
    let name = name.to_ascii_uppercase();
    let (stem, extension) = name.rsplit_once('.')?;
    if !matches!(extension, "IFO" | "BUP" | "VOB") {
        return None;
    }
    if stem == "VIDEO_TS" {
        return Some(if extension == "IFO" {
            DvdFile::Index
        } else {
            DvdFile::VideoTs
        });
    }
    let rest = stem.strip_prefix("VTS_")?;
    let (title_set, part) = rest.split_once('_')?;
    if title_set.len() != 2 || part.len() != 1 {
        return None;
    }
    Some(DvdFile::TitleSet {
        title_set: title_set.parse().ok()?,
        part: part.parse().ok()?,
        vob: extension == "VOB",
    })
}

/// A DVD is the DVD files in one folder, with `VIDEO_TS.IFO` and at least one
/// title set's `.VOB`. The folder is usually `VIDEO_TS`, though it's sometimes
/// renamed, e.g. `VIDEO_TS (13)`, so its files are known by their names.
fn detect_dvds<Id: Copy>(files: &[(Id, &str)]) -> Vec<DetectedDisc<Id>> {
    #[derive(Default)]
    struct Group<Id> {
        index: Option<Id>,
        has_title: bool,
        members: Vec<Id>,
    }
    let mut groups = BTreeMap::<&str, Group<Id>>::new();
    for &(id, path) in files {
        let (folder, name) = split(path);
        let Some(file) = dvd_file(name) else {
            continue;
        };
        let group = groups.entry(folder).or_insert_with(|| Group {
            index: None,
            has_title: false,
            members: Vec::new(),
        });
        match file {
            DvdFile::Index => group.index = Some(id),
            DvdFile::TitleSet { vob: true, .. } => {
                group.has_title = true;
                group.members.push(id);
            }
            _ => group.members.push(id),
        }
    }
    groups
        .into_iter()
        .filter_map(|(folder, group)| {
            let index = group.index.filter(|_| group.has_title)?;
            // The disc's folder is the one the `VIDEO_TS` folder is in,
            // unless the DVD's files sit directly in it.
            let (parent, name) = split(folder);
            let root = if name.to_ascii_uppercase().starts_with("VIDEO_TS") {
                parent
            } else {
                folder
            };
            Some(DetectedDisc {
                kind: DiscKind::Dvd,
                root: root.to_string(),
                index,
                members: group.members,
            })
        })
        .collect()
}

/// How long a disc plays: its longest title. A Blu-ray's is its longest
/// stream. A DVD's is its longest title set, whose `.VOB` parts after the menu
/// (part 0) play one after another. `members` are the disc's other files, each
/// its path and duration, if measured.
pub fn disc_duration(kind: DiscKind, members: &[(&str, Option<Duration>)]) -> Option<Duration> {
    match kind {
        DiscKind::BluRay => members
            .iter()
            .filter(|(path, _)| {
                let (folder, _) = split(path);
                split(folder).1.eq_ignore_ascii_case("STREAM")
            })
            .filter_map(|(_, duration)| *duration)
            .max(),
        DiscKind::Dvd => {
            let mut title_sets = BTreeMap::<u8, Duration>::new();
            for (path, duration) in members {
                if let (
                    Some(DvdFile::TitleSet {
                        title_set,
                        part,
                        vob: true,
                    }),
                    Some(duration),
                ) = (dvd_file(split(path).1), duration)
                    && part != 0
                {
                    *title_sets.entry(title_set).or_default() += *duration;
                }
            }
            title_sets.into_values().max()
        }
    }
}

/// The folder that holds a disc's files, e.g. `Movie/BDMV`: the index file's,
/// or a disc image, which holds them all.
pub fn disc_folder(index_path: &str) -> &str {
    if is_disc_image(index_path) {
        index_path
    } else {
        split(index_path).0
    }
}

/// Whether a disc's index file is an image of the whole disc, e.g. an `.iso`,
/// rather than a disc folder's `index.bdmv` or `VIDEO_TS.IFO`.
pub fn is_disc_image(index_path: &str) -> bool {
    let name = split(index_path).1;
    !name.eq_ignore_ascii_case("index.bdmv") && !name.eq_ignore_ascii_case("VIDEO_TS.IFO")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(paths: &[&str]) -> Vec<(DiscKind, String, &'static str, Vec<&'static str>)> {
        let files: Vec<(usize, &str)> = paths.iter().copied().enumerate().collect();
        // Leaked so that the results can name the files by path.
        let names: &'static [String] =
            Vec::leak(paths.iter().map(|path| path.to_string()).collect());
        detect_discs(&files)
            .into_iter()
            .map(|disc| {
                (
                    disc.kind,
                    disc.root,
                    names[disc.index].as_str(),
                    disc.members.iter().map(|&i| names[i].as_str()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn a_blu_ray_is_its_bdmv_folder_and_certificate() {
        let discs = detect(&[
            "D2/BDMV/index.bdmv",
            "D2/BDMV/MovieObject.bdmv",
            "D2/BDMV/PLAYLIST/00000.mpls",
            "D2/BDMV/STREAM/00098.m2ts",
            "D2/BDMV/BACKUP/index.bdmv",
            "D2/CERTIFICATE/id.bdmv",
            "D2/disc.inf",
            "Other/movie.mkv",
        ]);
        assert_eq!(
            discs,
            [(
                DiscKind::BluRay,
                "D2".to_string(),
                "D2/BDMV/index.bdmv",
                vec![
                    "D2/BDMV/MovieObject.bdmv",
                    "D2/BDMV/PLAYLIST/00000.mpls",
                    "D2/BDMV/STREAM/00098.m2ts",
                    "D2/BDMV/BACKUP/index.bdmv",
                    "D2/CERTIFICATE/id.bdmv",
                ]
            )]
        );
    }

    #[test]
    fn a_partial_blu_ray_is_not_a_disc() {
        // No index, or no streams.
        assert_eq!(detect(&["A/BDMV/STREAM/00001.m2ts"]), []);
        assert_eq!(
            detect(&["B/BDMV/index.bdmv", "B/BDMV/PLAYLIST/00000.mpls"]),
            []
        );
    }

    #[test]
    fn a_blu_ray_at_the_root_has_an_empty_folder() {
        let discs = detect(&["BDMV/index.bdmv", "BDMV/STREAM/00001.m2ts"]);
        assert_eq!(discs[0].1, "");
    }

    #[test]
    fn a_dvd_is_its_dvd_files_in_a_folder_however_named() {
        let discs = detect(&[
            "Film/VIDEO_TS (13)/VIDEO_TS.IFO",
            "Film/VIDEO_TS (13)/VIDEO_TS.BUP",
            "Film/VIDEO_TS (13)/VTS_01_0.IFO",
            "Film/VIDEO_TS (13)/VTS_01_1.VOB",
            "Film/VIDEO_TS (13)/notes.txt",
            "Loose/video_ts.ifo",
            "Loose/vts_01_1.vob",
        ]);
        assert_eq!(
            discs,
            [
                (
                    DiscKind::Dvd,
                    "Film".to_string(),
                    "Film/VIDEO_TS (13)/VIDEO_TS.IFO",
                    vec![
                        "Film/VIDEO_TS (13)/VIDEO_TS.BUP",
                        "Film/VIDEO_TS (13)/VTS_01_0.IFO",
                        "Film/VIDEO_TS (13)/VTS_01_1.VOB",
                    ]
                ),
                (
                    DiscKind::Dvd,
                    "Loose".to_string(),
                    "Loose/video_ts.ifo",
                    vec!["Loose/vts_01_1.vob"]
                ),
            ]
        );
    }

    #[test]
    fn a_dvd_without_a_title_set_is_not_a_disc() {
        assert_eq!(
            detect(&["X/VIDEO_TS/VIDEO_TS.IFO", "X/VIDEO_TS/VIDEO_TS.VOB"]),
            []
        );
    }

    #[test]
    fn a_disc_plays_for_its_longest_title() {
        let minutes = |m: u64| Some(Duration::from_secs(m * 60));
        let blu_ray = [
            ("D/BDMV/STREAM/00001.m2ts", minutes(58)),
            ("D/BDMV/STREAM/00002.m2ts", minutes(1)),
            ("D/BDMV/PLAYLIST/00000.mpls", minutes(120)),
        ];
        assert_eq!(disc_duration(DiscKind::BluRay, &blu_ray), minutes(58));
        let dvd = [
            ("V/VTS_01_0.VOB", minutes(5)),
            ("V/VTS_01_1.VOB", minutes(30)),
            ("V/VTS_01_2.VOB", minutes(30)),
            ("V/VTS_02_1.VOB", minutes(45)),
        ];
        assert_eq!(disc_duration(DiscKind::Dvd, &dvd), minutes(60));
    }
}
