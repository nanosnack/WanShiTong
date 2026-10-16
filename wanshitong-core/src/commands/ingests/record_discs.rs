use crate::{
    CoreError, CoreErrorContext,
    domain::{
        DetectedDisc, FileMeasurementRowId, FileMediaMeasurements, IngestRowId, detect_discs,
        disc_duration,
    },
    json,
};
use sqlx::SqliteConnection;
use std::collections::BTreeMap;

/// Detects the discs among an ingest's files, from its file listing, and
/// records them, with their sizes and durations. A disc image, e.g. an `.iso`,
/// is known by its media measurements instead: it's a disc of its own. See
/// *Disc* in `spec/domain-model.md`.
pub(crate) async fn record_discs(
    txn: &mut SqliteConnection,
    ingest_id: IngestRowId,
) -> Result<(), CoreError> {
    let files = sqlx::query!(
        r#"
        SELECT
            m.id AS "id!: FileMeasurementRowId",
            m.data_source_id,
            m.rel_path,
            m.size,
            m.media_measurements AS "media_measurements?: String"
        FROM complete_file_measurements m
        WHERE m.ingest_id = ?
        "#,
        ingest_id,
    )
    .fetch_all(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    let media = |media_measurements: Option<&str>| {
        media_measurements
            .map(json::decode::<FileMediaMeasurements>)
            .transpose()
    };

    // Each data source's files, as paths are only unique within one.
    let mut by_data_source = BTreeMap::<i64, Vec<(FileMeasurementRowId, &str)>>::new();
    for file in &files {
        by_data_source
            .entry(file.data_source_id)
            .or_default()
            .push((file.id, file.rel_path.as_str()));
    }
    let by_id: BTreeMap<i64, _> = files.iter().map(|file| (file.id.0, file)).collect();

    for paths in by_data_source.values() {
        for disc in detect_discs(paths) {
            let mut size = by_id[&disc.index.0].size;
            let mut members = Vec::with_capacity(disc.members.len());
            for member in &disc.members {
                let file = by_id[&member.0];
                size += file.size;
                let duration =
                    media(file.media_measurements.as_deref())?.and_then(|media| media.duration());
                members.push((file.rel_path.as_str(), duration));
            }
            let duration_ms =
                disc_duration(disc.kind, &members).map(|duration| duration.as_millis() as i64);
            insert_disc(txn, &disc, size, duration_ms).await?;
        }
    }

    for file in &files {
        let Some(media) = media(file.media_measurements.as_deref())? else {
            continue;
        };
        if let Some(kind) = media.disc_image {
            // Its streams are its longest title's.
            let duration_ms = media.duration().map(|duration| duration.as_millis() as i64);
            let disc = DetectedDisc {
                kind,
                root: file.rel_path.clone(),
                index: file.id,
                members: Vec::new(),
            };
            insert_disc(txn, &disc, file.size, duration_ms).await?;
        }
    }

    Ok(())
}

/// Records a disc, with the size of its files together and its duration.
async fn insert_disc(
    txn: &mut SqliteConnection,
    disc: &DetectedDisc<FileMeasurementRowId>,
    size: i64,
    duration_ms: Option<i64>,
) -> Result<(), CoreError> {
    let DetectedDisc {
        kind,
        root,
        index,
        members,
    } = disc;
    let kind = kind.as_db_str();
    let disc_id = sqlx::query_scalar!(
        r#"
        INSERT INTO discs (index_file_id, kind, root_path, size, duration_ms)
        VALUES (?, ?, ?, ?, ?)
        RETURNING id AS "id!"
        "#,
        index,
        kind,
        root,
        size,
        duration_ms,
    )
    .fetch_one(&mut *txn)
    .await
    .map_err(CoreErrorContext::from)?;
    for member in members {
        sqlx::query!(
            "INSERT INTO disc_files (disc_id, file_id) VALUES (?, ?)",
            disc_id,
            member,
        )
        .execute(&mut *txn)
        .await
        .map_err(CoreErrorContext::from)?;
    }
    Ok(())
}
