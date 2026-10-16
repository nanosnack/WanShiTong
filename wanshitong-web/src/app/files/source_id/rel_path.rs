use super::SourceId;
use crate::{
    app::{
        corrections,
        data_sources::data_source_id::{self, DataSourceId},
        file_audit::audit_log,
        movies::movie_id::{self, MovieId},
        page_help::page_help,
        tv_series::series_id::{self, SeriesId},
    },
    context::app,
    display::{self, FileSize},
    styles::{H1, H2, LINK, MONO, NUMBER},
    ui::{
        alert::{alert, alert_description},
        card::{card, card_content, card_header, card_title},
        table::{table, table_body, table_cell, table_head, table_header, table_row},
    },
};
use jiff::Timestamp;
use std::path::{Path, PathBuf};
use topcoat::{
    Result,
    context::Cx,
    router::{error::RouterErrorExt, href, page, path_param},
    view::{View, attributes, component, view},
};
use wanshitong_app::domain::PathId;
use wanshitong_app::{
    domain::{DataSourceUuid, FileDetails, FileMediaMeasurements, is_disc_image},
    queries,
};

path_param!(pub *rel_path);

/// A path's segments, to link to its file's page.
pub fn segments(rel_path: &Path) -> Vec<String> {
    rel_path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect()
}

/// A file as the canonical interpretation sees it: what it is in the catalog,
/// how it was identified, and what was measured about it.
#[page]
pub async fn page(cx: &Cx) -> Result<impl View> {
    let source = DataSourceUuid(*path_param::<SourceId>(cx)?);
    let rel_path: PathBuf = path_param::<RelPath>(cx).collect();
    let file = queries::get_file(app(cx), source, &rel_path)
        .await?
        .ok_or_not_found()?;
    Ok(view! { file_page(file: file) })
}

#[component]
async fn file_page(cx: &Cx, file: FileDetails) -> Result<impl View> {
    let here = href!(
        page,
        SourceId(file.data_source_uuid.0),
        RelPath(segments(&file.rel_path)),
    )
    .resolve(cx);
    let path = file.rel_path.to_string_lossy().into_owned();
    let name = file.rel_path.file_name().map_or_else(
        || file.rel_path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    Ok(view! {
        <header class="flex flex-col gap-1">
            <div class="flex items-center gap-2">
                <h1 class=(format!("{H1} break-all")) translate="no">(name)</h1>
                page_help(
                    <p>
                        "A file as the canonical interpretation sees it: what it is in the "
                        "catalog, how it was identified, and what was measured about it."
                    </p>
                    <p>
                        "A file that's in the catalog as nothing is either not media, or "
                        "its claims didn't add up to a movie or an episode."
                    </p>
                )
            </div>
            <p class=(format!("{MONO} break-all text-muted-foreground")) translate="no">
                (file.rel_path.display().to_string())
            </p>
        </header>

        if let Some(disc) = &file.disc {
            alert(
                alert_description(
                    if is_disc_image(&file.rel_path.to_string_lossy()) {
                        (format!(
                            "This file is a {}.",
                            display::disc(disc.kind, disc.file_count, disc.duration),
                        ))
                    } else {
                        (format!(
                            "This file stands for a {}, in ",
                            display::disc(disc.kind, disc.file_count, disc.duration),
                        ))
                        if disc.root.is_empty() {
                            "its data source's root"
                        } else {
                            <span translate="no">(format!("{}/", disc.root))</span>
                        }
                        "."
                    }
                )
            )
        }
        if let Some(index) = &file.part_of_disc {
            alert(
                alert_description(
                    "One of the files of the disc identified by this index file: "
                    <a
                        class=(LINK)
                        translate="no"
                        href=(href!(
                            page,
                            SourceId(file.data_source_uuid.0),
                            RelPath(segments(index)),
                        ))
                    >
                        (index.display().to_string())
                    </a>
                    ". Do not attempt to mark this. "
                    "Corrections should be made to the disc's index file."
                )
            )
        }

        corrections::marking::mark_control(
            kind: corrections::marking::MarkKind::File,
            uuid: file.data_source_uuid.0,
            path: Some(path.clone()),
            files: vec![PathId::new(file.data_source_uuid, &file.rel_path)],
            return_to: here
        )
        card(
            card_header(card_title("In the Catalog"))
            card_content(
                if file.movies.is_empty()
                    && file.episodes.is_empty()
                    && file.series_extras.is_empty() {
                    <p class="text-muted-foreground">
                        if file.audit_log.is_some() {
                            "Not in the catalog. "
                            "Its claims, below, didn't add up to a movie or an episode."
                        } else {
                            "Not in the catalog. Nothing claimed anything about it."
                        }
                    </p>
                } else {
                    <ul class="flex flex-col gap-1">
                        for movie in &file.movies {
                            <li>
                                "Movie "
                                <a
                                    class=(LINK)
                                    translate="no"
                                    href=(href!(movie_id::page, MovieId(movie.uuid.0)))
                                >
                                    (movie.title.as_str())
                                </a>
                                if let Some(year) = movie.release_year {
                                    <span class="text-muted-foreground">
                                        (format!(" ({year})"))
                                    </span>
                                }
                                <span class="text-muted-foreground">
                                    (format!(
                                        " · {}",
                                        movie
                                            .role
                                            .as_ref()
                                            .map_or("Unknown role", display::file_role),
                                    ))
                                </span>
                            </li>
                        }
                        for episode in &file.episodes {
                            <li>
                                "Episode of "
                                <a
                                    class=(LINK)
                                    translate="no"
                                    href=(href!(
                                        series_id::page,
                                        SeriesId(episode.series_uuid.0),
                                    ))
                                >
                                    (episode.series_name.as_str())
                                </a>
                                <span class="text-muted-foreground">
                                    " · "
                                    <span translate="no">
                                        (display::episode_number(
                                            episode.season_number,
                                            episode.episode_number,
                                        ))
                                    </span>
                                    if let Some(title) = &episode.title {
                                        " · "
                                        <span translate="no">(title.as_str())</span>
                                    }
                                </span>
                            </li>
                        }
                        for extra in &file.series_extras {
                            <li>
                                (if extra.disc { "Disc of " } else { "Extra of " })
                                <a
                                    class=(LINK)
                                    translate="no"
                                    href=(href!(series_id::page, SeriesId(extra.series_uuid.0)))
                                >
                                    (extra.series_name.as_str())
                                </a>
                            </li>
                        }
                    </ul>
                }
            )
        )

        card(
            card_header(card_title("Details"))
            card_content(
                <dl class="kv-list">
                    <div>
                        <dt>"Data Source"</dt>
                        <dd>
                            <a
                                class=(LINK)
                                translate="no"
                                href=(href!(
                                    data_source_id::page,
                                    DataSourceId(file.data_source_uuid.0),
                                ))
                            >
                                (file.data_source_name.as_str())
                            </a>
                        </dd>
                    </div>
                    <div>
                        <dt>"Size"</dt>
                        <dd>(FileSize(Some(file.size)).to_string())</dd>
                    </div>
                    if let Ok(mtime) = Timestamp::from_second(file.mtime) {
                        <div>
                            <dt>"Modified"</dt>
                            <dd translate="no">(display::ts(&mtime).to_string())</dd>
                        </div>
                    }
                    if let Some(mime_type) = &file.mime_type {
                        <div>
                            <dt>"MIME Type"</dt>
                            <dd translate="no">(mime_type.as_str())</dd>
                        </div>
                    }
                    if let Some(container) = file
                        .media
                        .as_ref()
                        .and_then(|m| m.container.as_ref()) {
                        <div>
                            <dt>"Container"</dt>
                            <dd translate="no">(container.format.as_str())</dd>
                        </div>
                        if let Some(duration) = container.duration() {
                            <div>
                                <dt>"Duration"</dt>
                                <dd>(display::human_duration(duration))</dd>
                            </div>
                        }
                        if let Some(bitrate) = container.bitrate {
                            <div>
                                <dt>"Bitrate"</dt>
                                <dd>(bitrate_string(bitrate))</dd>
                            </div>
                        }
                        if let Some(title) = container
                            .tags
                            .as_ref()
                            .and_then(|t| t.get("title").or(t.get("TITLE"))) {
                            <div>
                                <dt>"Title Tag"</dt>
                                <dd translate="no">(title.as_str())</dd>
                            </div>
                        }
                    }
                </dl>
            )
        )

        if let Some(media) = &file.media {
            streams(media: media)
        }

        <section class="flex flex-col gap-2">
            <h2 class=(H2)>"Identification"</h2>
            if let Some(audit) = &file.audit_log {
                audit_log(audit: audit)
            } else {
                <p class="text-sm text-muted-foreground">
                    "Nothing claimed anything about this file."
                </p>
            }
        </section>
    })
}

/// A stream detail: data, e.g. a language code, which translators leave
/// alone, or our words.
struct Detail {
    text: String,
    is_data: bool,
}

impl Detail {
    fn data(text: String) -> Self {
        Self {
            text,
            is_data: true,
        }
    }

    fn words(text: String) -> Self {
        Self {
            text,
            is_data: false,
        }
    }
}

/// A row of the streams table.
struct StreamRow<'a> {
    index: i32,
    kind: &'static str,
    codec: &'a str,
    details: Vec<Detail>,
}

/// The file's streams, in the order ffprobe found them.
#[component]
async fn streams(media: &FileMediaMeasurements) -> Result<impl View> {
    let mut rows: Vec<StreamRow> = Vec::new();
    for v in &media.video {
        let mut details = vec![Detail::data(format!("{}x{}", v.width, v.height))];
        if let (Some(num), Some(den)) = (v.frame_rate_num, v.frame_rate_den)
            && den != 0
        {
            details.push(Detail::words(format!(
                "{:.3} fps",
                f64::from(num) / f64::from(den)
            )));
        }
        if let Some(depth) = v.bit_depth {
            details.push(Detail::words(format!("{depth}-bit")));
        }
        if let Some(profile) = &v.profile {
            details.push(Detail::data(profile.clone()));
        }
        rows.push(StreamRow {
            index: v.stream_index,
            kind: "Video",
            codec: &v.codec,
            details,
        });
    }
    for a in &media.audio {
        let mut details = vec![match &a.channel_layout {
            Some(layout) => Detail::data(layout.clone()),
            None => Detail::words(format!("{} channels", a.channels)),
        }];
        if let Some(language) = &a.language {
            details.push(Detail::data(language.clone()));
        }
        if let Some(rate) = a.sample_rate {
            details.push(Detail::words(format!("{rate} Hz")));
        }
        rows.push(StreamRow {
            index: a.stream_index,
            kind: "Audio",
            codec: &a.codec,
            details,
        });
    }
    for t in &media.text {
        let mut details = Vec::new();
        if let Some(language) = &t.language {
            details.push(Detail::data(language.clone()));
        }
        if t.is_default == Some(true) {
            details.push(Detail::words("default".into()));
        }
        if t.is_forced == Some(true) {
            details.push(Detail::words("forced".into()));
        }
        rows.push(StreamRow {
            index: t.stream_index,
            kind: "Subtitles",
            codec: &t.codec,
            details,
        });
    }
    rows.sort_by_key(|row| row.index);
    let others = media.unclassified.len();

    Ok(view! {
        if !rows.is_empty() || others > 0 {
            <section class="flex flex-col gap-2">
                <h2 class=(H2)>"Streams"</h2>
                table(
                    attrs: attributes! { class="stacked-table" },
                    table_header(
                        table_row(
                            table_head(attrs: attributes! { class=(NUMBER) }, "#")
                            table_head("Type")
                            table_head("Codec")
                            table_head("Details")
                        )
                    )
                    table_body(
                        for StreamRow { index, kind, codec, details } in &rows {
                            table_row(
                                table_cell(
                                    attrs: attributes! { class=(NUMBER) data-label="#" },
                                    (index.to_string())
                                )
                                table_cell(
                                    attrs: attributes! { data-label="Type" },
                                    (*kind)
                                )
                                table_cell(
                                    attrs: attributes! { data-label="Codec" translate="no" },
                                    (*codec)
                                )
                                table_cell(
                                    attrs: attributes! { data-label="Details" },
                                    for (i, Detail { text, is_data }) in details
                                        .iter()
                                        .enumerate() {
                                        if i > 0 {
                                            ", "
                                        }
                                        if *is_data {
                                            <span translate="no">(text.as_str())</span>
                                        } else {
                                            (text.as_str())
                                        }
                                    }
                                )
                            )
                        }
                    )
                )
                if others > 0 {
                    <p class="text-sm text-muted-foreground">
                        (match others {
                            1 => "And 1 other stream, e.g. an attachment.".to_string(),
                            n => format!("And {n} other streams, e.g. attachments."),
                        })
                    </p>
                }
            </section>
        }
    })
}

fn bitrate_string(bits_per_second: i64) -> String {
    if bits_per_second >= 1_000_000 {
        format!("{:.1} Mb/s", bits_per_second as f64 / 1_000_000.0)
    } else {
        format!("{} kb/s", bits_per_second / 1000)
    }
}
