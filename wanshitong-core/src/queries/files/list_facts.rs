use crate::json;
use crate::{
    CoreContext, CoreError, CoreErrorContext,
    domain::{
        DataSourceUuid, DiscFacts, Facts, FileFacts, FileMeasurement, FileMeasurementRowId,
        InterpretationRowId, PathId,
    },
};
use error_stack::ResultExt;

pub async fn list_facts(
    ctx: &CoreContext,
    interpretation_id: InterpretationRowId,
) -> Result<Facts, CoreError> {
    let mut txn = ctx.reader.begin().await.map_err(CoreErrorContext::from)?;

    let file_records = sqlx::query!(
        r#"
        WITH fact_ingests AS (
            SELECT ingest_id
            FROM interpretation_ingests
            WHERE interpretation_id = ?
        )
        SELECT
            m.id AS "id!: FileMeasurementRowId",
            m.data_source_uuid AS "data_source_uuid!: DataSourceUuid",
            m.data_source_name,
            m.rel_path,
            m.device,
            m.inode,
            m.size,
            m.mime_type,
            m.mtime,
            m.ctime,
            m.hardlink_count,
            m.is_symlink,
            m.media_measurements AS "media_measurements?: String",
            d.kind AS "disc_kind?: String",
            d.root_path AS "disc_root?: String",
            d.size AS "disc_size?: i64",
            d.duration_ms AS "disc_duration_ms?: i64",
            (SELECT count(*) FROM disc_files df WHERE df.disc_id = d.id) AS "disc_member_count?: i64"
        FROM complete_file_measurements m
        JOIN fact_ingests fi
            ON fi.ingest_id = m.ingest_id
        LEFT JOIN discs d
            ON d.index_file_id = m.id
        -- A disc's other files: its index file stands for them.
        WHERE m.id NOT IN (SELECT file_id FROM disc_files)
        ORDER BY
            data_source_name,
            rel_path;
        "#,
        interpretation_id,
    )
    .fetch_all(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;

    let files: Vec<_> = file_records
        .into_iter()
        .map(|r| {
            let media = r
                .media_measurements
                .as_deref()
                .map(json::decode::<crate::domain::FileMediaMeasurements>)
                .transpose()
                .attach("decoding media measurements from SQL")?;
            Ok((
                PathId {
                    data_source: r.data_source_uuid,
                    rel_path: r.rel_path.into(),
                },
                FileFacts {
                    row_id: r.id,
                    file: FileMeasurement {
                        device: r.device.map(|x| x as u64),
                        inode: r.inode.map(|x| x as u64),
                        size_bytes: r.size as u64,
                        mtime: r.mtime,
                        ctime: r.ctime,
                        hardlink_count: r.hardlink_count.map(|c| c as u64),
                        is_symlink: r.is_symlink != 0,
                        media_measurement_id: None, // unused
                    },
                    mime_type: r.mime_type,
                    media,
                    disc: DiscFacts::from_db(
                        r.disc_kind,
                        r.disc_root,
                        r.disc_member_count,
                        r.disc_size,
                        r.disc_duration_ms,
                    )?,
                },
            ))
        })
        .collect::<Result<_, CoreError>>()?;

    txn.commit().await.map_err(CoreErrorContext::from)?;

    Ok(Facts { files })
}
