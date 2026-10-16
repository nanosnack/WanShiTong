use crate::cli::{ClientArgs, WorkArgs};
use crate::client::client;
use crate::text::{
    count, disc_text, duration, external_id, interpretation_line, movie_part,
    movie_part_of_manifestation, print_json, size,
};
use uuid::Uuid;
use wanshitong_api::v1::{
    Assertion, Claim, ClaimSource, Directory, DirectoryFile, File, FileRole, KbLookupResult,
    KbQuery, KbSearchResult, Media, Placement, PlacementCount, PlacementGroup, Rank, RankedClaim,
    Work, WorkEvidence, WorkKind, WorkQuery,
};

pub async fn work(args: WorkArgs) -> anyhow::Result<()> {
    let WorkArgs {
        client: ClientArgs { url, json },
        uuid,
        inactive,
        directory_files,
    } = args;
    let query = WorkQuery { directory_files };
    let evidence = client(&url).get_work_evidence(uuid, &query).await?;
    if json {
        return print_json(&evidence);
    }

    let WorkEvidence {
        interpretation,
        work,
        files,
        top_directories,
        assertions,
    } = evidence;
    println!("{}", interpretation_line(&interpretation));
    print_work(&work);
    for file in &files {
        println!();
        print_file(file, inactive);
    }
    let work_uuid = match &work {
        Work::Movie { uuid, .. } | Work::TvSeries { uuid, .. } => *uuid,
    };
    if !top_directories.is_empty() {
        println!();
        println!("Top directories (* marks this work's files):");
        for top_directory in &top_directories {
            let prefix = format!("{}: ", top_directory.data_source.name);
            print_directory(&top_directory.root, &prefix, Some(work_uuid), 1);
        }
    }
    if !assertions.is_empty() {
        println!();
        println!("Assertions:");
        for assertion in &assertions {
            print_assertion(assertion);
        }
    }
    Ok(())
}

fn print_work(work: &Work) {
    let (kind, uuid, title, year, external_ids) = match work {
        Work::Movie {
            uuid,
            title,
            year,
            external_ids,
            ..
        } => ("Movie", uuid, title, year, external_ids),
        Work::TvSeries {
            uuid,
            title,
            year,
            external_ids,
        } => ("TV series", uuid, title, year, external_ids),
    };
    let year = year.map(|year| format!(" ({year})")).unwrap_or_default();
    let ids = if external_ids.is_empty() {
        "no external IDs".to_owned()
    } else {
        external_ids
            .iter()
            .map(external_id)
            .collect::<Vec<_>>()
            .join(", ")
    };
    println!("{kind} {uuid}: {title}{year}, {ids}");
    if let Work::Movie {
        runtime_seconds: Some(runtime),
        ..
    } = work
    {
        println!("Runtime: {}", duration(*runtime as f64));
    }
}

pub(crate) fn print_file(file: &File, inactive: bool) {
    let File {
        data_source,
        path,
        size_bytes,
        placements,
        media,
        claims,
        kb_queries,
        disc,
        part_of_disc,
    } = file;
    println!("{}: {path}", data_source.name);
    if let Some(disc) = disc {
        println!("  stands for a {}", disc_text(disc));
    }
    if let Some(index) = part_of_disc {
        println!("  is one of the files of the disc at {index}");
    }
    for placement in placements {
        println!("  is {}", placement_text(placement));
    }
    match media {
        Some(media) => println!("  {}, {}", media_text(media), size(*size_bytes)),
        None => println!("  not media, {}", size(*size_bytes)),
    }

    let (active, losers): (Vec<_>, Vec<_>) = claims.iter().partition(|claim| claim.active);
    if !active.is_empty() {
        println!("  claims:");
        for claim in active {
            println!("    {}", ranked_claim_text(claim));
        }
    }
    if !losers.is_empty() {
        if inactive {
            println!("  inactive claims:");
            for claim in losers {
                println!("    {}", ranked_claim_text(claim));
            }
        } else {
            println!("  {}", count(losers.len() as i64, "inactive claim"));
        }
    }
    for query in kb_queries {
        println!("  {}", kb_query_text(query));
    }
}

fn placement_text(placement: &Placement) -> String {
    match placement {
        Placement::Movie {
            title,
            year,
            role,
            manifestation,
            part,
            ..
        } => {
            let year = year.map(|year| format!(" ({year})")).unwrap_or_default();
            let role = movie_part_of_manifestation(*role, *manifestation, *part);
            format!("{role} of movie \"{title}\"{year}")
        }
        Placement::Episode {
            series_title,
            season,
            episode,
            title,
            ..
        } => {
            let number = match season {
                Some(season) => format!("S{season:02}E{episode:02}"),
                None => format!("episode {episode} of an unknown season"),
            };
            let title = title
                .as_ref()
                .map(|title| format!(" \"{title}\""))
                .unwrap_or_default();
            format!("{number}{title} of series \"{series_title}\"")
        }
        Placement::SeriesExtra { series_title, .. } => {
            format!("extra of series \"{series_title}\"")
        }
        Placement::SeriesDisc { series_title, .. } => {
            format!("disc of series \"{series_title}\"")
        }
    }
}

fn media_text(media: &Media) -> String {
    let Media {
        format,
        duration_seconds,
        title_tag,
        video,
        audio,
        subtitles,
    } = media;
    let mut parts = Vec::new();
    if let Some(seconds) = duration_seconds {
        parts.push(duration(*seconds));
    }
    if let Some(format) = format {
        parts.push(format.clone());
    }
    for stream in video {
        parts.push(format!(
            "video {} {}x{}",
            stream.codec, stream.width, stream.height
        ));
    }
    for stream in audio {
        let language = stream.language.as_deref().unwrap_or("und");
        parts.push(format!(
            "audio {language} {} {}ch",
            stream.codec, stream.channels
        ));
    }
    if !subtitles.is_empty() {
        let languages: Vec<_> = subtitles
            .iter()
            .map(|stream| {
                let language = stream.language.as_deref().unwrap_or("und");
                match stream.forced {
                    Some(true) => format!("{language} (forced)"),
                    _ => language.to_owned(),
                }
            })
            .collect();
        parts.push(format!("subtitles {}", languages.join(" ")));
    }
    if let Some(title) = title_tag {
        parts.push(format!("title tag \"{title}\""));
    }
    parts.join(", ")
}

fn ranked_claim_text(claim: &RankedClaim) -> String {
    let RankedClaim {
        claim,
        source,
        rank,
        active: _,
    } = claim;
    let source = match source {
        ClaimSource::Heuristic { name } => name.clone(),
        ClaimSource::Assertion => "assertion".to_owned(),
        ClaimSource::KnowledgeBase { result } => format!("kb#{result}"),
    };
    let rank = match rank {
        Rank::Number(rank) => format!("{rank:.1}"),
        // A knowledge base result an assertion found, by its bonus.
        Rank::FoundByAssertion(bonus) => format!("assertion{bonus:+.1}"),
        Rank::Assertion => "assertion".to_owned(),
    };
    format!("{rank:>13}  {}  ({source})", claim_text(claim))
}

/// A claim as `kind value`, with the kinds the JSON API names them by.
pub fn claim_text(claim: &Claim) -> String {
    match claim {
        Claim::DurationSeconds(seconds) => format!("duration_seconds {seconds}"),
        Claim::FileRole(role) => format!(
            "file_role {}",
            match role {
                FileRole::Main => "main",
                FileRole::Extra => "extra",
                FileRole::Sample => "sample",
                FileRole::Disc => "disc",
            }
        ),
        Claim::MovieExternalId(id) => format!("movie_external_id {}", external_id(id)),
        Claim::MovieTitle(title) => format!("movie_title \"{title}\""),
        Claim::MovieReleaseYear(year) => format!("movie_release_year {year}"),
        Claim::TvSeriesExternalId(id) => format!("tv_series_external_id {}", external_id(id)),
        Claim::TvSeriesName(name) => format!("tv_series_name \"{name}\""),
        Claim::TvSeriesYear(year) => format!("tv_series_year {year}"),
        Claim::Season(season) => format!("season {season}"),
        Claim::EpisodeRange([first, last]) => format!("episode_range {first}-{last}"),
        Claim::AbsoluteEpisodeRange([first, last]) => {
            format!("absolute_episode_range {first}-{last}")
        }
        Claim::Special => "special".to_owned(),
        Claim::Part(part) => format!("part {part}"),
        Claim::SpecialOf(name) => format!("special_of \"{name}\""),
        Claim::ExtraOf(path) => format!("extra_of \"{path}\""),
        Claim::DiscOf(path) => format!("disc_of \"{path}\""),
        Claim::Excluded => "excluded".to_owned(),
        Claim::Edition(edition) => format!("edition \"{edition}\""),
        Claim::EpisodeTitle(title) => format!("episode_title \"{title}\""),
        Claim::WorkKind(kind) => format!(
            "work_kind {}",
            match kind {
                WorkKind::Movie => "movie",
                WorkKind::TvSeries => "tv_series",
            }
        ),
    }
}

fn kb_query_text(query: &KbQuery) -> String {
    let results = |results: &[KbSearchResult]| {
        if results.is_empty() {
            return "no results".to_owned();
        }
        results
            .iter()
            .enumerate()
            .map(|(i, KbSearchResult { id, title, year })| {
                let year = year.map(|year| format!(" ({year})")).unwrap_or_default();
                format!("{}. {title}{year} {}", i + 1, external_id(id))
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    let lookup = |found: &Option<KbLookupResult>| match found {
        Some(KbLookupResult {
            title,
            year,
            runtime_minutes,
            ..
        }) => {
            let year = year.map(|year| format!(" ({year})")).unwrap_or_default();
            let runtime = runtime_minutes
                .map(|minutes| format!(", {minutes} min"))
                .unwrap_or_default();
            format!(": {title}{year}{runtime}")
        }
        None => String::new(),
    };
    match query {
        KbQuery::Find { id, found } => format!("kb find {}{}", external_id(id), lookup(found)),
        KbQuery::MovieSearch { query, results: r } => {
            format!("kb movie search \"{query}\": {}", results(r))
        }
        KbQuery::TvSeriesSearch { query, results: r } => {
            format!("kb tv series search \"{query}\": {}", results(r))
        }
        KbQuery::MovieDetails { id, found } => {
            format!("kb movie details {}{}", external_id(id), lookup(found))
        }
        KbQuery::TvSeriesDetails { id, found } => {
            format!("kb tv series details {}{}", external_id(id), lookup(found))
        }
        KbQuery::TvSeasonDetails { id, season } => {
            format!("kb tv season details {} season {season}", external_id(id))
        }
        KbQuery::AlternativeTitles { id } => format!("kb alternative titles {}", external_id(id)),
    }
}

pub(crate) fn print_assertion(assertion: &Assertion) {
    let Assertion {
        uuid,
        claim,
        files,
        created_at: _,
        rationale,
        confidence,
    } = assertion;
    let confidence = confidence
        .map(|confidence| format!(" (confidence {confidence:.2})"))
        .unwrap_or_default();
    println!("  {uuid}  {}{confidence}", claim_text(claim));
    if let Some(rationale) = rationale {
        println!("    because: {rationale}");
    }
    for file in files {
        let retracted = if file.retracted_at.is_some() {
            " (retracted since)"
        } else {
            ""
        };
        println!("    {}: {}{retracted}", file.data_source.name, file.path);
    }
}

/// A directory, with a summary of its own files or a list of them,
/// then its subdirectories, indented below it. Placements in `work`, the
/// one whose evidence this is, if any, are said as `this movie` or `this
/// series`.
pub(crate) fn print_directory(dir: &Directory, prefix: &str, work: Option<Uuid>, depth: usize) {
    let Directory {
        name,
        has_work_files,
        file_count,
        placements,
        files,
        directories,
        folded_directories,
    } = dir;
    let indent = "  ".repeat(depth);
    let mark = if *has_work_files { "* " } else { "  " };
    // Folded directories' files count as this one's.
    let folded = match folded_directories {
        0 => String::new(),
        n => format!(" (+{})", count(*n as i64, "directory")),
    };
    let summary = if *file_count > 0 {
        let placements: Vec<_> = placements
            .iter()
            .map(|count| placement_count_text(count, work))
            .collect();
        format!(
            "  {}: {}",
            count(*file_count as i64, "file"),
            placements.join(", ")
        )
    } else {
        String::new()
    };
    println!("{indent}{mark}{prefix}{name}/{folded}{summary}");
    for file in files.iter().flatten() {
        print_directory_file(file, work, depth + 1);
    }
    for dir in directories {
        print_directory(dir, "", work, depth + 1);
    }
}

fn placement_count_text(count: &PlacementCount, work: Option<Uuid>) -> String {
    let PlacementCount { placement, files } = count;
    let n = *files;
    let files = |noun: &str| crate::text::count(n as i64, noun);
    let this_or = |uuid: &Uuid, kind: &str, title: &str| {
        if Some(*uuid) == work {
            format!("this {kind}")
        } else {
            format!("{kind} \"{title}\"")
        }
    };
    match placement {
        PlacementGroup::Movie {
            uuid, title, role, ..
        } => {
            let noun = movie_part(*role);
            format!("{} of {}", files(noun), this_or(uuid, "movie", title))
        }
        PlacementGroup::Episodes {
            series_uuid,
            series_title,
        } => format!(
            "{} of {}",
            files("episode file"),
            this_or(series_uuid, "series", series_title)
        ),
        PlacementGroup::SeriesExtras {
            series_uuid,
            series_title,
        } => format!(
            "{} of {}",
            files("extra"),
            this_or(series_uuid, "series", series_title)
        ),
        PlacementGroup::SeriesDiscs {
            series_uuid,
            series_title,
        } => format!(
            "{} of {}",
            files("disc"),
            this_or(series_uuid, "series", series_title)
        ),
        PlacementGroup::UnidentifiedVideos => files("unidentified video"),
        PlacementGroup::Other => files("other file"),
    }
}

fn print_directory_file(file: &DirectoryFile, work: Option<Uuid>, depth: usize) {
    let DirectoryFile {
        name,
        size_bytes,
        duration_seconds,
        placements,
    } = file;
    let indent = "  ".repeat(depth);
    let mut parts = vec![size(*size_bytes)];
    if let Some(seconds) = duration_seconds {
        parts.push(duration(*seconds));
    }
    let own = placements.iter().any(|placement| match placement {
        Placement::Movie { uuid, .. } => Some(*uuid) == work,
        Placement::Episode { series_uuid, .. }
        | Placement::SeriesExtra { series_uuid, .. }
        | Placement::SeriesDisc { series_uuid, .. } => Some(*series_uuid) == work,
    });
    let mark = if own { "* " } else { "  " };
    let placements = if placements.is_empty() {
        "in no work".to_owned()
    } else {
        placements
            .iter()
            .map(placement_text)
            .collect::<Vec<_>>()
            .join("; ")
    };
    println!("{indent}{mark}{name}  {}  {placements}", parts.join(", "));
}
