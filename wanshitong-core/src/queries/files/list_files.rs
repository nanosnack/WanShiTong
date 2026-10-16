use crate::domain::{DataSourceUuid, DiscSummary, FileSummary, InterpretationRowId};
use crate::{CoreContext, CoreError, CoreErrorContext};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub async fn list_files(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
    query: &FileSearchQuery,
) -> Result<FileSummaryList, CoreError> {
    let FileSearchQuery {
        name,
        folder,
        work,
        identified,
        non_videos,
        cursor,
        limit,
    } = query;
    let (folder_data_source, folder_prefix) = match folder {
        Some((data_source, path)) => (*data_source, format!("{path}/")),
        None => (None, String::new()),
    };
    let (cursor_data_source, cursor_path) = match cursor {
        Some(FileCursor {
            data_source_name,
            rel_path,
        }) => (Some(data_source_name), Some(rel_path)),
        None => (None, None),
    };
    let query_limit = limit + 1;
    let rows = sqlx::query!(
        r#"
        -- ?1 interpretation, ?2 work, ?3 identified, ?4 non_videos say which
        -- files, in the clause list_file_tree shares; ?5 name, ?6 and ?7 the
        -- folder's data source and path prefix (empty for every folder), ?8
        -- and ?9 the cursor's data source name and path, ?10 the limit.
        WITH fact_ingests AS (
            SELECT ingest_id, data_source_id
            FROM interpretation_ingests
            WHERE interpretation_id = ?1
        )
        SELECT
            m.data_source_uuid AS "data_source_uuid!: DataSourceUuid",
            m.data_source_name,
            m.rel_path,
            coalesce(d.size, m.size) AS "size?: i64",
            d.kind AS "disc_kind?: String",
            d.duration_ms AS "disc_duration_ms?: i64",
            (SELECT count(*) FROM disc_files df WHERE df.disc_id = d.id) AS "disc_member_count?: i64"
        FROM complete_file_measurements m
        JOIN fact_ingests fi
            ON fi.ingest_id = m.ingest_id
            AND fi.data_source_id = m.data_source_id
        LEFT JOIN discs d
            ON d.index_file_id = m.id
        WHERE
            (?5 IS NULL OR lower(m.rel_path) LIKE '%' || lower(?5) || '%')
            -- The folder's files are a range of paths, which the files' index
            -- serves: from the prefix up to it followed by the last code point,
            -- which no path starts with in practice.
            AND (?6 IS NULL OR m.data_source_uuid = ?6)
            AND m.rel_path >= ?7
            AND m.rel_path < ?7 || char(1114111)
            -- A disc's other files: its index file stands for them.
            AND m.id NOT IN (SELECT file_id FROM disc_files)
            -- A work's files, all of them, or else those the filter selects.
            AND (?2 IS NULL OR m.id IN (SELECT file_id FROM work_files WHERE work_uuid = ?2))
            AND (
                ?2 IS NOT NULL
                OR m.id IN (
                    SELECT file_id
                    FROM unidentified_videos
                    WHERE interpretation_id = ?1
                )
                OR (?3 AND ?4)
                OR (
                    ?3
                    AND m.id IN (SELECT file_id FROM identified_files WHERE interpretation_id = ?1)
                    AND m.id IN (SELECT file_id FROM video_files WHERE interpretation_id = ?1)
                )
                OR (
                    ?4
                    AND m.id NOT IN (SELECT file_id FROM video_files WHERE interpretation_id = ?1)
                    AND m.id NOT IN (
                        SELECT file_id FROM identified_files WHERE interpretation_id = ?1
                    )
                )
            )
            AND (
                ?8 IS NULL
                OR (lower(rel_path), rel_path, data_source_name) >= (lower(?9), ?9, ?8)
            )
        -- By path, ignoring case, whatever data source it's in.
        ORDER BY
            lower(rel_path),
            rel_path,
            data_source_name
        LIMIT ?10;
        "#,
        interpretation_id,
        work,
        identified,
        non_videos,
        name,
        folder_data_source,
        folder_prefix,
        cursor_data_source,
        cursor_path,
        query_limit,
    )
    .fetch_all(&ctx.reader)
    .await
    .map_err(CoreErrorContext::from)?;
    let mut files: Vec<FileSummary> = rows
        .into_iter()
        .map(|row| {
            Ok(FileSummary {
                data_source_uuid: row.data_source_uuid,
                data_source_name: row.data_source_name,
                rel_path: row.rel_path,
                size: row.size,
                disc: DiscSummary::from_db(
                    row.disc_kind,
                    row.disc_member_count,
                    row.disc_duration_ms,
                )?,
            })
        })
        .collect::<Result<_, CoreError>>()?;

    let next_cursor = if files.len() == query_limit as usize {
        files.last().map(|file| FileCursor {
            data_source_name: file.data_source_name.clone(),
            rel_path: file.rel_path.clone(),
        })
    } else {
        None
    };
    files.truncate(*limit as usize);

    Ok(FileSummaryList { files, next_cursor })
}

pub struct FileSearchQuery {
    pub name: Option<String>,
    /// Only the files under this folder: a path, in a data source, or in any
    /// of them.
    pub folder: Option<(Option<DataSourceUuid>, String)>,
    /// Only a movie's or series' files, all of them, by its UUID, rather than
    /// those `identified` and `non_videos` select.
    pub work: Option<Uuid>,
    /// Also the files of identified works, which are otherwise left out, as
    /// only the unidentified videos are what's left to identify.
    pub identified: bool,
    /// Also the files without moving pictures, e.g. subtitles, scans or a
    /// disc's playlists. With `identified`, every file is listed, even one
    /// excluded from the catalog.
    pub non_videos: bool,
    pub cursor: Option<FileCursor>,
    pub limit: i64,
}

impl Default for FileSearchQuery {
    fn default() -> Self {
        Self {
            name: Default::default(),
            folder: None,
            work: None,
            identified: false,
            non_videos: false,
            cursor: Default::default(),
            limit: 50,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct FileCursor {
    pub data_source_name: String,
    pub rel_path: String,
}

pub struct FileSummaryList {
    pub files: Vec<FileSummary>,
    pub next_cursor: Option<FileCursor>,
}
