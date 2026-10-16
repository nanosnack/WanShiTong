//! Text for people, and agents, to read.

use wanshitong_api::v1::{
    Disc, DiscKind, ExternalId, FileRole, Interpretation, InterpretationState,
};

pub fn interpretation_line(interpretation: &Interpretation) -> String {
    let canonical = if interpretation.canonical {
        " (canonical)"
    } else {
        ""
    };
    // A catalog that's missing or incomplete, e.g. to compare, says why.
    let state = match interpretation.state {
        InterpretationState::Succeeded => "",
        InterpretationState::Running => " [still running]",
        InterpretationState::Interrupted => " [interrupted]",
        InterpretationState::Failed => " [failed]",
    };
    format!(
        "Interpretation {}{canonical}{state}: {}",
        interpretation.uuid, interpretation.name
    )
}

/// E.g. "1 season" or "2 seasons", or "2 directories".
pub fn count(n: i64, noun: &str) -> String {
    match (n, noun.strip_suffix('y')) {
        (1, _) => format!("1 {noun}"),
        (n, Some(stem)) => format!("{n} {stem}ies"),
        (n, None) => format!("{n} {noun}s"),
    }
}

/// E.g. `tmdb:71883`.
pub fn external_id(id: &ExternalId) -> String {
    match id {
        ExternalId::Imdb(id) => format!("imdb:{id}"),
        ExternalId::Tmdb(id) => format!("tmdb:{id}"),
        ExternalId::Tvdb(id) => format!("tvdb:{id}"),
    }
}

/// E.g. `1:58:03` or `0:42`.
pub fn duration(seconds: f64) -> String {
    let seconds = seconds.round() as u64;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// E.g. `4.2 GB`.
pub fn size(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Prints a response as JSON, on one line, for `--json`.
pub fn print_json(value: &impl serde::Serialize) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

/// E.g. `Blu-ray disc of 47 files, 58:03`.
pub fn disc_text(disc: &Disc) -> String {
    let kind = match disc.kind {
        DiscKind::BluRay => "Blu-ray disc",
        DiscKind::Dvd => "DVD",
    };
    let files = count(disc.file_count as i64, "file");
    match disc.duration_seconds {
        Some(seconds) => format!("{kind} of {files}, {}", duration(seconds)),
        None => format!("{kind} of {files}"),
    }
}

/// What a file is of its movie, e.g. `main file`, or `file` when nothing
/// says its role.
pub fn movie_part(role: Option<FileRole>) -> &'static str {
    match role {
        Some(FileRole::Main) => "main file",
        Some(FileRole::Extra) => "extra",
        Some(FileRole::Sample) => "sample",
        Some(FileRole::Disc) => "disc file",
        None => "file",
    }
}

/// What a file is of its movie, with which manifestation a main file is, counted from
/// 1, and which part of that manifestation if its name numbers it as one, e.g. `main
/// file (manifestation 2, part 1)`.
pub fn movie_part_of_manifestation(
    role: Option<FileRole>,
    manifestation: Option<usize>,
    part: Option<u32>,
) -> String {
    let role = movie_part(role);
    match (manifestation, part) {
        (Some(manifestation), Some(part)) => {
            format!("{role} (manifestation {}, part {part})", manifestation + 1)
        }
        (Some(manifestation), None) => format!("{role} (manifestation {})", manifestation + 1),
        (None, _) => role.to_owned(),
    }
}
