use crate::cli::{ClientArgs, UnidentifiedArgs};
use crate::client::client;
use crate::text::{disc_text, interpretation_line, print_json, size};
use wanshitong_api::v1::{FileSummary, UnidentifiedQuery, UnidentifiedVideos};

pub async fn unidentified(args: UnidentifiedArgs) -> anyhow::Result<()> {
    let UnidentifiedArgs {
        client: ClientArgs { url, json },
        interpretation,
        name,
        cursor,
        limit,
    } = args;
    let query = UnidentifiedQuery {
        interpretation,
        name,
        cursor,
        limit: Some(limit),
    };
    let page = client(&url).get_unidentified_videos(&query).await?;
    if json {
        return print_json(&page);
    }

    let UnidentifiedVideos {
        interpretation,
        files,
        next_cursor,
    } = page;
    println!("{}", interpretation_line(&interpretation));
    if files.is_empty() {
        println!("No unidentified videos.");
    }
    // Files come sorted by data source, then path, so each data source's, and
    // each top directory's, are together.
    let mut last: Option<(&str, Option<&str>)> = None;
    for FileSummary {
        data_source,
        path,
        size_bytes,
        disc,
    } in &files
    {
        let top_directory = path.split_once('/').map(|(top_directory, _)| top_directory);
        let new_data_source = last.is_none_or(|(name, _)| name != data_source.name);
        // Files at a data source's root are top directories of their own.
        let new_release =
            last.is_none_or(|(_, last)| last != top_directory) || top_directory.is_none();
        if new_data_source {
            println!();
            println!("Data source {} {}:", data_source.name, data_source.uuid);
        } else if new_release {
            println!();
        }
        last = Some((&data_source.name, top_directory));
        let disc = disc
            .as_ref()
            .map(|disc| format!("  {}", disc_text(disc)))
            .unwrap_or_default();
        match size_bytes {
            Some(bytes) => println!("  {path}  {}{disc}", size(*bytes)),
            None => println!("  {path}{disc}"),
        }
    }
    if let Some(cursor) = next_cursor {
        println!();
        println!("Next page: repeat with --cursor {cursor}");
    }
    Ok(())
}
