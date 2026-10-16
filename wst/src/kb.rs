use crate::cli::{ClientArgs, KbCommand, KbMovieArgs, KbSearchArgs, KbTvSeriesArgs};
use crate::client::client;
use crate::text::{count, external_id, print_json};
use wanshitong_api::v1::{
    ExternalId, KbMovie, KbSearchHit, KbSearchQuery, KbSearchResults, KbSeason, KbTvSeries,
    WorkKind,
};

/// How much of an overview a search result shows.
const OVERVIEW_CHARS: usize = 100;

pub async fn kb(command: KbCommand) -> anyhow::Result<()> {
    match command {
        KbCommand::Search(args) => search(args).await,
        KbCommand::Movie(args) => movie(args).await,
        KbCommand::TvSeries(args) => tv_series(args).await,
    }
}

async fn search(args: KbSearchArgs) -> anyhow::Result<()> {
    let KbSearchArgs {
        client: ClientArgs { url, json },
        query,
        tv,
        year,
    } = args;
    let query = KbSearchQuery {
        kind: if tv {
            WorkKind::TvSeries
        } else {
            WorkKind::Movie
        },
        query,
        year,
    };
    let found = client(&url).search_knowledge_base(&query).await?;
    if json {
        return print_json(&found);
    }
    let KbSearchResults { results } = found;
    if results.is_empty() {
        println!("No results.");
    }
    for KbSearchHit {
        id,
        title,
        original_title,
        year,
        overview,
    } in results
    {
        let original = if original_title != title {
            format!(" [{original_title}]")
        } else {
            String::new()
        };
        let year = year.map(|year| format!(" ({year})")).unwrap_or_default();
        let overview = if overview.chars().count() > OVERVIEW_CHARS {
            let start: String = overview.chars().take(OVERVIEW_CHARS).collect();
            format!("{}...", start.trim_end())
        } else {
            overview
        };
        println!("{}  {title}{original}{year}  {overview}", external_id(&id));
    }
    Ok(())
}

async fn movie(args: KbMovieArgs) -> anyhow::Result<()> {
    let KbMovieArgs {
        client: ClientArgs { url, json },
        tmdb_id,
    } = args;
    let movie = client(&url).get_kb_movie(tmdb_id).await?;
    if json {
        return print_json(&movie);
    }
    let KbMovie {
        ids,
        title,
        original_title,
        original_language,
        year,
        runtime_minutes,
        overview,
    } = movie;
    let year = year.map(|year| format!(" ({year})")).unwrap_or_default();
    println!("{title}{year}, {}", ids_text(&ids));
    println!("Original title: {original_title} ({original_language})");
    match runtime_minutes {
        Some(minutes) => println!("Runtime: {minutes} minutes"),
        None => println!("Runtime: unknown"),
    }
    println!("{overview}");
    Ok(())
}

async fn tv_series(args: KbTvSeriesArgs) -> anyhow::Result<()> {
    let KbTvSeriesArgs {
        client: ClientArgs { url, json },
        tmdb_id,
    } = args;
    let series = client(&url).get_kb_tv_series(tmdb_id).await?;
    if json {
        return print_json(&series);
    }
    let KbTvSeries {
        ids,
        title,
        original_title,
        original_language,
        first_air_year,
        last_air_year,
        overview,
        seasons,
    } = series;
    let years = match (first_air_year, last_air_year) {
        (Some(first), Some(last)) if first != last => format!(" ({first}-{last})"),
        (Some(first), _) => format!(" ({first})"),
        _ => String::new(),
    };
    println!("{title}{years}, {}", ids_text(&ids));
    println!("Original title: {original_title} ({original_language})");
    for KbSeason {
        number,
        episode_count,
        air_year,
    } in seasons
    {
        let name = match number {
            0 => "Specials".to_owned(),
            n => format!("Season {n}"),
        };
        let year = air_year
            .map(|year| format!(" ({year})"))
            .unwrap_or_default();
        println!("{name}{year}: {}", count(episode_count as i64, "episode"));
    }
    println!("{overview}");
    Ok(())
}

fn ids_text(ids: &[ExternalId]) -> String {
    ids.iter().map(external_id).collect::<Vec<_>>().join(", ")
}
