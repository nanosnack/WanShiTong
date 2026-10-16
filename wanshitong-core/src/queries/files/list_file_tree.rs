use crate::domain::{DataSourceUuid, DiscSummary, InterpretationRowId};
use crate::{CoreContext, CoreError, CoreErrorContext};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A folder's children, as the files page's tree shows them, by name: its
/// subfolders that hold any of the files searched for, each with how many, and
/// the files directly in it. Like [`super::list_files`],
/// the files are the unidentified videos unless the query adds more.
pub async fn list_file_tree(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
    query: &FileTreeQuery,
) -> Result<FileTreeList, CoreError> {
    let FileTreeQuery {
        folder,
        work,
        identified,
        non_videos,
        after,
        limit,
    } = query;
    let (data_source, prefix) = match folder {
        Some((data_source, path)) => (Some(*data_source), format!("{path}/")),
        None => (None, String::new()),
    };
    let (after_data_source, after_name) = match after {
        Some(FileTreeCursor {
            data_source_name,
            name,
        }) => (Some(data_source_name), Some(name)),
        None => (None, None),
    };
    let query_limit = limit + 1;
    let rows = sqlx::query!(
        r#"
        -- ?1 interpretation, ?2 work, ?3 identified, ?4 non_videos say which
        -- files, in the clause list_files shares; ?5 and ?6 the folder's data
        -- source and path prefix, ?7 and ?8 the cursor's data source name and
        -- entry name, ?9 the limit.
        WITH fact_ingests AS (
            SELECT ingest_id, data_source_id
            FROM interpretation_ingests
            WHERE interpretation_id = ?1
        ),
        -- The files searched for, in the folder, by their paths in it: a range
        -- of paths from the prefix up to it followed by the last code point,
        -- which no path starts with in practice, so that the files' index
        -- serves it (`substr` would make it a scan). A disc
        -- is where its files are, e.g. `Film/BDMV`, rather than its index file,
        -- and a disc image, e.g. `Film.iso`, is where it is.
        matching AS (
            SELECT
                m.data_source_uuid,
                m.data_source_name,
                m.rel_path,
                coalesce(d.size, m.size) AS size,
                d.kind AS disc_kind,
                d.duration_ms AS disc_duration_ms,
                (SELECT count(*) FROM disc_files df WHERE df.disc_id = d.id) AS disc_member_count,
                substr(
                    CASE
                        WHEN d.id IS NOT NULL AND (
                            m.rel_path LIKE '%/index.bdmv' OR m.rel_path LIKE '%/VIDEO_TS.IFO'
                        ) THEN substr(
                            m.rel_path,
                            1,
                            length(m.rel_path)
                                - CASE d.kind WHEN 'dvd' THEN length('VIDEO_TS.IFO') ELSE length('index.bdmv') END
                                - 1
                        )
                        ELSE m.rel_path
                    END,
                    length(?6) + 1
                ) AS rest
            FROM complete_file_measurements m
            JOIN fact_ingests fi
                ON fi.ingest_id = m.ingest_id
                AND fi.data_source_id = m.data_source_id
            LEFT JOIN discs d
                ON d.index_file_id = m.id
            WHERE
                (?5 IS NULL OR m.data_source_uuid = ?5)
                AND m.rel_path >= ?6
                AND m.rel_path < ?6 || char(1114111)
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
                        AND m.id IN (
                            SELECT file_id FROM identified_files WHERE interpretation_id = ?1
                        )
                        AND m.id IN (SELECT file_id FROM video_files WHERE interpretation_id = ?1)
                    )
                    OR (
                        ?4
                        AND m.id NOT IN (
                            SELECT file_id FROM video_files WHERE interpretation_id = ?1
                        )
                        AND m.id NOT IN (
                            SELECT file_id FROM identified_files WHERE interpretation_id = ?1
                        )
                    )
                )
        ),
        children AS (
            SELECT
                data_source_uuid,
                data_source_name,
                CASE instr(rest, '/')
                    WHEN 0 THEN rest
                    ELSE substr(rest, 1, instr(rest, '/') - 1)
                END AS name,
                instr(rest, '/') > 0 AS is_folder,
                count(*) AS file_count,
                max(size) AS size,
                max(rel_path) AS file_path,
                max(disc_kind) AS disc_kind,
                max(disc_duration_ms) AS disc_duration_ms,
                max(disc_member_count) AS disc_member_count
            FROM matching
            GROUP BY data_source_uuid, data_source_name, name, is_folder
        )
        SELECT
            data_source_uuid AS "data_source_uuid!: DataSourceUuid",
            data_source_name AS "data_source_name!: String",
            name AS "name!: String",
            CASE WHEN is_folder THEN ?6 || name ELSE file_path END AS "path!: String",
            is_folder AS "is_folder!: bool",
            file_count AS "file_count!: i64",
            size AS "size!: i64",
            disc_kind AS "disc_kind?: String",
            disc_duration_ms AS "disc_duration_ms?: i64",
            disc_member_count AS "disc_member_count?: i64"
        FROM children
        WHERE
            ?8 IS NULL
            OR (lower(name), name, data_source_name) > (lower(?8), ?8, ?7)
        -- By name, ignoring case, folders and files together, whatever data
        -- source they're in. A folder can't share a file's name.
        ORDER BY
            lower(name),
            name,
            data_source_name,
            is_folder
        LIMIT ?9
        "#,
        interpretation_id,
        work,
        identified,
        non_videos,
        data_source,
        prefix,
        after_data_source,
        after_name,
        query_limit,
    )
    .fetch_all(&ctx.reader)
    .await.map_err(CoreErrorContext::from)?;
    let mut entries: Vec<FileTreeEntry> = rows
        .into_iter()
        .map(|row| {
            Ok(FileTreeEntry {
                data_source_uuid: row.data_source_uuid,
                data_source_name: row.data_source_name,
                name: row.name,
                path: row.path,
                is_folder: row.is_folder,
                file_count: row.file_count,
                size: row.size,
                disc: DiscSummary::from_db(
                    row.disc_kind,
                    row.disc_member_count,
                    row.disc_duration_ms,
                )?,
            })
        })
        .collect::<Result<_, CoreError>>()?;

    let next = if entries.len() == query_limit as usize {
        entries.truncate(*limit as usize);
        entries.last().map(|entry| FileTreeCursor {
            data_source_name: entry.data_source_name.clone(),
            name: entry.name.clone(),
        })
    } else {
        None
    };
    Ok(FileTreeList { entries, next })
}

pub struct FileTreeQuery {
    /// Only a movie's or series' files, all of them, by its UUID, rather than
    /// those `identified` and `non_videos` select.
    pub work: Option<Uuid>,
    /// A data source and a folder's path in it. None for the top level: every
    /// data source's top-level folders and files.
    pub folder: Option<(DataSourceUuid, String)>,
    /// Also the files of identified works; see [`super::FileSearchQuery`].
    pub identified: bool,
    /// Also the files without moving pictures.
    pub non_videos: bool,
    pub after: Option<FileTreeCursor>,
    pub limit: i64,
}

/// A folder's subfolder, or a file directly in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTreeEntry {
    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    pub name: String,
    /// Its path in its data source.
    pub path: String,
    pub is_folder: bool,
    /// How many of the searched-for files are under this entry: 1 for a file.
    pub file_count: i64,
    /// A file's size, in bytes: a disc's, all of its files'. The largest
    /// file's, for a folder.
    pub size: i64,
    /// The disc it stands for, if it's a disc's index file. Its name is then
    /// the disc's folder's, e.g. `BDMV`, and its path still its index file's.
    pub disc: Option<DiscSummary>,
}

/// Where the next page of a folder's children starts.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct FileTreeCursor {
    pub data_source_name: String,
    pub name: String,
}

pub struct FileTreeList {
    pub entries: Vec<FileTreeEntry>,
    pub next: Option<FileTreeCursor>,
}
