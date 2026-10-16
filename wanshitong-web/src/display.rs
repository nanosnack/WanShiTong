use jiff::{Timestamp, tz::TimeZone};
use std::{borrow::Cow, time::Duration};
use topcoat::{
    Result,
    view::{View, component, view},
};
use wanshitong_app::domain::{
    ClaimPredicate, ClaimSource, DataSourceConfig, DiscKind, ExternalId, FileRole,
    FileSystemSource, Heuristic, KbQueryEvent, WorkKind,
};

pub fn human_duration(d: Duration) -> String {
    let secs = d.as_secs();

    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    let seconds = secs % 60;

    if days > 0 {
        if seconds > 0 {
            format!("{days}d {hours}h {minutes}m {seconds}s")
        } else if minutes > 0 {
            format!("{days}d {hours}h {minutes}m")
        } else if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours > 0 {
        if seconds > 0 {
            format!("{hours}h {minutes}m {seconds}s")
        } else if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    } else if minutes > 0 {
        if seconds > 0 {
            format!("{minutes}m {seconds}s")
        } else {
            format!("{minutes}m")
        }
    } else {
        format!("{seconds}s")
    }
}

pub fn ts(ts: &Timestamp) -> jiff::fmt::strtime::Display<'static> {
    ts.to_zoned(TimeZone::system())
        .strftime("%Y-%m-%d %H:%M:%S")
}

/// Where a data source reads from.
#[component]
pub async fn source(config: &DataSourceConfig) -> Result<impl View> {
    let DataSourceConfig::FileSystem(FileSystemSource {
        root_path,
        follow_symlinks,
    }) = config;
    Ok(view! {
        <span translate="no">(root_path.display().to_string())</span>
        if *follow_symlinks {
            " (+symlinks)"
        }
    })
}

pub struct FileSize(pub Option<i64>);

impl std::fmt::Display for FileSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(size) = self.0 {
            write!(f, "{}", bytesize::ByteSize::b(size as u64))
        } else {
            write!(f, "unknown")
        }
    }
}

/// A knowledge base query: what kind it was, then what it asked for, which
/// translators should leave alone.
#[component]
pub async fn kb_query(query: &KbQueryEvent) -> Result<impl View> {
    let (label, value) = match query {
        KbQueryEvent::Find { id, .. } => ("Find", external_id(id)),
        KbQueryEvent::MovieSearch { query, .. } => ("Movie Search", format!("\"{query}\"")),
        KbQueryEvent::TvSeriesSearch { query, .. } => ("TV Series Search", format!("\"{query}\"")),
        KbQueryEvent::MovieDetails { id, .. } => ("Movie Details", external_id(id)),
        KbQueryEvent::TvSeriesDetails { id, .. } => ("TV Series Details", external_id(id)),
        KbQueryEvent::TvSeasonDetails { id, .. } => ("TV Season Details", external_id(id)),
        KbQueryEvent::AlternativeTitles { id } => ("Alternative Titles", external_id(id)),
    };
    Ok(view! {
        (label)
        ": "
        <span translate="no">(value)</span>
        if let KbQueryEvent::TvSeasonDetails { season, .. } = query {
            (format!(" season {season}"))
        }
    })
}

pub fn external_id(id: &ExternalId) -> String {
    match id {
        ExternalId::Imdb(imdb_id) => format!("IMDB {}", imdb_id),
        ExternalId::Tmdb(tmdb_id) => format!("TMDB {}", tmdb_id),
        ExternalId::Tvdb(tvdb_id) => format!("TVDB {}", tvdb_id),
    }
}

/// A claim: what it is about, then its value, which translators should leave
/// alone.
#[component]
pub async fn claim_pred(pred: &ClaimPredicate) -> Result<impl View> {
    let quoted = |s: &dyn std::fmt::Display| Some(format!("\"{s}\""));
    let range = |r: &[_; 2]| Some(format!("{}-{}", r[0], r[1]));
    let (label, value): (Cow<str>, Option<String>) = match pred {
        ClaimPredicate::Duration(duration) => ("Duration".into(), Some(human_duration(*duration))),
        // A role is our word, not data, so it is translated with its label.
        ClaimPredicate::FileRole(role) => (format!("File Role: {}", file_role(role)).into(), None),
        ClaimPredicate::MovieExternalId(id) => ("Movie ID".into(), Some(external_id(id))),
        ClaimPredicate::MovieTitle(title) => ("Movie Title".into(), quoted(title)),
        ClaimPredicate::MovieReleaseYear(year) => ("Release Year".into(), Some(year.to_string())),
        ClaimPredicate::TvSeriesExternalId(id) => ("TV Series ID".into(), Some(external_id(id))),
        ClaimPredicate::TvSeriesName(name) => ("TV Series".into(), quoted(name)),
        ClaimPredicate::TvSeriesYear(year) => ("First Aired".into(), Some(year.to_string())),
        ClaimPredicate::Season(num) => ("Season".into(), Some(num.to_string())),
        ClaimPredicate::EpisodeRange(r) => ("Episodes".into(), range(r)),
        ClaimPredicate::Special => ("Special of its series".into(), None),
        ClaimPredicate::Part(part) => ("Part".into(), Some(part.to_string())),
        ClaimPredicate::SpecialOf(series) => ("Special of".into(), quoted(series)),
        ClaimPredicate::AbsoluteEpisodeRange(r) => ("Episodes through the series".into(), range(r)),
        ClaimPredicate::ExtraOf(main_path) => ("Extra of".into(), Some(main_path.to_string())),
        ClaimPredicate::DiscOf(index_path) => {
            ("Part of the disc".into(), Some(index_path.to_string()))
        }
        ClaimPredicate::Excluded => ("Excluded from the catalog".into(), None),
        ClaimPredicate::Edition(edition) => ("Edition".into(), quoted(edition)),
        ClaimPredicate::EpisodeTitle(title) => ("Episode Title".into(), quoted(title)),
        ClaimPredicate::WorkKind(kind) => (
            "Kind of Work".into(),
            Some(match kind {
                WorkKind::Movie => "movie".to_string(),
                WorkKind::TvSeries => "TV series".to_string(),
            }),
        ),
    };
    Ok(view! {
        (label.as_ref())
        if let Some(value) = value {
            ": "
            <span translate="no">(value)</span>
        }
    })
}

pub fn claim_source(claim_source: &ClaimSource) -> String {
    match claim_source {
        ClaimSource::Heuristic(heuristic) => heuristic_name(heuristic).into(),
        ClaimSource::Assertion => "Assertion".into(),
        ClaimSource::KnowledgeBase { result } => format!("Knowledge base result {}", result + 1),
    }
}

pub fn heuristic_name(heuristic: &Heuristic) -> &'static str {
    match heuristic {
        Heuristic::Bdmv => "Blu-ray directory",
        Heuristic::Dvd => "DVD directory",
        Heuristic::MovieDirectory => "Movie directory",
        Heuristic::Sample => "Sample name",
        Heuristic::ContainerDuration => "Container duration",
        Heuristic::ReleaseName => "Release name",
        Heuristic::FileNameSeriesName => "Series name in file name",
        Heuristic::FileNameSeasonEpisode => "Season/episode in file name",
        Heuristic::ContainerTitleTag => "Container title tag",
        Heuristic::FileNameTitle => "Title from file name",
        Heuristic::ExtrasNameOrDirectory => "Extras name or directory",
        Heuristic::SeriesOfNearbyEpisodes => "Series of nearby episodes",
        Heuristic::TitleWithoutNumbering => "Title without movie or OVA number",
        Heuristic::FileNameEpisode => "Episode in file name",
        Heuristic::SeasonDirectory => "Season directory",
        Heuristic::NumberedRun => "Numbered like its sibling files",
        Heuristic::SeriesSpecial => "OVA in file name",
        Heuristic::FileNamePart => "Part in file name",
        Heuristic::SeasonRelease => "In a season release of another series",
        Heuristic::TitleBeforePart => "Title before its part",
        Heuristic::SeasonEpisodeNumber => "Season/episode number in its season directory",
        Heuristic::EpisodeInParentheses => "Episode number through the series in parentheses",
        Heuristic::SeriesNameBeforeSeasonEpisodeNumber => {
            "Series name before its season/episode number"
        }
        Heuristic::SpecialOfNearbySeries => "Special of the series of nearby episodes",
        Heuristic::ExtrasOfNearbyMovie => "Extra of the movie in a directory above",
        Heuristic::LoneNumberedEpisode => "Episode number in a lone video's name",
        Heuristic::TitleBeforeSubtitle => "Title before its subtitle",
        Heuristic::TitleSplitAtCapitals => "Title split at its capitals",
        Heuristic::TitleWithShortVowels => "Title with long vowels written short",
        Heuristic::TitleWithoutGroupTag => "Title without a group's tag",
        Heuristic::TitleWithoutAnd => "Title without \"and\"",
        Heuristic::TitleOfOneWord => "Title of one word without release words",
        Heuristic::SeriesAlsoKnownAs => "Series also known as another",
        Heuristic::KindOfWork => "Kind of work by its name and release",
        Heuristic::KindOfSiblings => "Series by the videos near it",
        Heuristic::KindOfDuration => "Movie by its duration",
        Heuristic::SeriesNameWithoutReleaseWords => "Series name without release words",
        Heuristic::TitleWithoutReleaseNoise => "Title without release tags",
        Heuristic::PartInTag => "Part number in a tag",
        Heuristic::FileNameEdition => "Edition in file name",
        Heuristic::TitleWithoutEdition => "Title without its edition",
        Heuristic::FileNameEpisodeTitle => "Episode title in file name",
        Heuristic::NameExternalId => "ID in a name",
        Heuristic::Nfo => "NFO file",
    }
}

pub fn file_role(role: &FileRole) -> &'static str {
    match role {
        FileRole::Main => "Main",
        FileRole::Extra => "Extra",
        FileRole::Sample => "Sample",
        FileRole::Disc => "Disc",
    }
}

/// What a file is of its movie, in a sentence, e.g. `main file of Heat`, or
/// `file` when nothing says its role.
pub fn movie_part(role: Option<FileRole>) -> &'static str {
    match role {
        Some(FileRole::Main) => "main file",
        Some(FileRole::Extra) => "extra",
        Some(FileRole::Sample) => "sample",
        Some(FileRole::Disc) => "disc file",
        None => "file",
    }
}

/// Which episode of its series a file is, e.g. `S01E02`, or `episode 12`
/// when its season is unknown and it's numbered through the whole series.
pub fn episode_number(season: Option<u32>, episode: u32) -> String {
    match season {
        Some(season) => format!("S{season:02}E{episode:02}"),
        None => format!("episode {episode}"),
    }
}

/// E.g. `Blu-ray disc · 47 files · 58m 3s`: a disc, which its index file
/// stands for.
pub fn disc(kind: DiscKind, file_count: u64, duration: Option<Duration>) -> String {
    let kind = match kind {
        DiscKind::BluRay => "Blu-ray disc",
        DiscKind::Dvd => "DVD",
    };
    // A disc of one file is an image of it, e.g. an `.iso`: a disc folder has
    // its index file and at least one title's.
    let mut parts = if file_count == 1 {
        vec![format!("{kind} image")]
    } else {
        vec![kind.to_string(), format!("{file_count} files")]
    };
    if let Some(duration) = duration {
        parts.push(human_duration(duration));
    }
    parts.join(" · ")
}
