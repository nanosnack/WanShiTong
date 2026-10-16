use crate::cli::{ClientArgs, DiffArgs};
use crate::client::client;
use crate::text::{
    count, external_id, interpretation_line, movie_part_of_manifestation, print_json,
};
use wanshitong_api::v1::{
    CatalogDiff, DiffPlacement, DiffQuery, DiffWork, FileChange, FilePart, WorkChange,
    WorkChangeKind, WorkKind,
};

pub async fn diff(args: DiffArgs) -> anyhow::Result<()> {
    let DiffArgs {
        client: ClientArgs { url, json },
        other,
        base,
    } = args;
    let diff = client(&url).diff(&DiffQuery { base, other }).await?;
    if json {
        return print_json(&diff);
    }
    print_diff(&diff);
    Ok(())
}

pub fn print_diff(diff: &CatalogDiff) {
    let CatalogDiff {
        base,
        other,
        changes,
        unidentified,
    } = diff;
    println!("Base:  {}", interpretation_line(base));
    println!("Other: {}", interpretation_line(other));
    for (heading, files) in [
        ("No longer unidentified", &unidentified.left),
        ("Newly unidentified", &unidentified.joined),
    ] {
        if !files.is_empty() {
            println!();
            println!("{heading}:");
            for file in files {
                println!("  {}: {}", file.data_source.name, file.path);
            }
        }
    }
    if changes.is_empty() {
        println!();
        println!("No works differ.");
        return;
    }
    for change in changes {
        println!();
        print_change(change);
    }
    println!();
    let mut kinds: Vec<(WorkChangeKind, usize)> = Vec::new();
    for change in changes {
        match kinds.iter_mut().find(|(kind, _)| *kind == change.kind) {
            Some((_, n)) => *n += 1,
            None => kinds.push((change.kind, 1)),
        }
    }
    let kinds: Vec<_> = kinds
        .into_iter()
        .map(|(kind, n)| format!("{n} {}", kind_text(kind)))
        .collect();
    println!(
        "{}: {}",
        count(changes.len() as i64, "change"),
        kinds.join(", ")
    );
}

fn print_change(change: &WorkChange) {
    let WorkChange {
        kind,
        before,
        after,
        files,
        disc_files,
    } = change;
    let works = |works: &[DiffWork]| works.iter().map(work_text).collect::<Vec<_>>().join(", ");
    match kind {
        WorkChangeKind::Added => println!("Added {}", works(after)),
        WorkChangeKind::Removed => println!("Removed {}", works(before)),
        // One work, the same but for its files, needs naming once.
        WorkChangeKind::Changed if works(before) == works(after) => {
            println!("Changed files of {}", works(before))
        }
        _ => println!(
            "{} {} -> {}",
            capitalized(kind_text(*kind)),
            works(before),
            works(after)
        ),
    }
    for FileChange {
        data_source,
        path,
        before: file_before,
        after: file_after,
    } in files
    {
        println!("  {}: {path}", data_source.name);
        if !file_before.is_empty() {
            println!("    - {}", placements_text(file_before, before));
        } else {
            println!("    - unidentified");
        }
        if !file_after.is_empty() {
            println!("    + {}", placements_text(file_after, after));
        } else {
            println!("    + unidentified");
        }
    }
    if *disc_files > 0 {
        println!("  and {}", count(*disc_files as i64, "disc file"));
    }
}

/// E.g. `movie "Heat" (1995) tmdb:949`.
fn work_text(work: &DiffWork) -> String {
    let DiffWork {
        kind,
        uuid: _,
        title,
        year,
        external_ids,
    } = work;
    let kind = match kind {
        WorkKind::Movie => "movie",
        WorkKind::TvSeries => "series",
    };
    let year = year.map(|year| format!(" ({year})")).unwrap_or_default();
    let ids: String = external_ids
        .iter()
        .map(|id| format!(" {}", external_id(id)))
        .collect();
    format!("{kind} \"{title}\"{year}{ids}")
}

/// E.g. `main file of "Heat"`, or `S01E02 of "The Wire"`.
fn placements_text(placements: &[DiffPlacement], works: &[DiffWork]) -> String {
    placements
        .iter()
        .map(|DiffPlacement { work, part }| {
            let part = match part {
                FilePart::Movie {
                    role,
                    manifestation,
                    part,
                } => movie_part_of_manifestation(*role, *manifestation, *part),
                FilePart::Episode { season, episode } => match season {
                    Some(season) => format!("S{season:02}E{episode:02}"),
                    None => format!("episode {episode}"),
                },
                FilePart::SeriesExtra => "extra".to_owned(),
                FilePart::SeriesDisc => "disc".to_owned(),
            };
            format!("{part} of \"{}\"", works[*work].title)
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn kind_text(kind: WorkChangeKind) -> &'static str {
    match kind {
        WorkChangeKind::Added => "added",
        WorkChangeKind::Removed => "removed",
        WorkChangeKind::Changed => "changed",
        WorkChangeKind::Split => "split",
        WorkChangeKind::Merged => "merged",
        WorkChangeKind::Regrouped => "regrouped",
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
