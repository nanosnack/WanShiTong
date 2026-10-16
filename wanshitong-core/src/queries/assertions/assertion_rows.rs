//! Assertions as their rows come back, one per file, grouped into
//! assertions.

use crate::time::stored_timestamp;
use crate::{
    CoreError,
    domain::{AssertedFile, Assertion, AssertionUuid, DataSourceUuid},
    json,
};

/// One assertion's claim about one file.
pub(super) struct AssertionRow {
    pub(super) uuid: AssertionUuid,
    pub(super) created_at: i64,
    pub(super) rationale: Option<String>,
    pub(super) confidence: Option<f64>,
    pub(super) claim: String,
    pub(super) data_source_uuid: DataSourceUuid,
    pub(super) data_source_name: String,
    pub(super) rel_path: String,
    pub(super) retracted_at: Option<i64>,
}

/// Rows ordered so that each assertion's are together, as assertions. Files
/// keep the order they were asserted in, e.g. a movie's main file first.
pub(super) fn group_rows(rows: Vec<AssertionRow>) -> Result<Vec<Assertion>, CoreError> {
    let mut assertions: Vec<Assertion> = Vec::new();
    for row in rows {
        let file = AssertedFile {
            data_source_uuid: row.data_source_uuid,
            data_source_name: row.data_source_name,
            rel_path: row.rel_path.into(),
            retracted_at: row.retracted_at.map(stored_timestamp),
        };
        match assertions.last_mut() {
            Some(assertion) if assertion.uuid == row.uuid => assertion.files.push(file),
            _ => {
                // Every file of an assertion has the same claim.
                let claim = json::decode(&row.claim)?;
                assertions.push(Assertion {
                    uuid: row.uuid,
                    claim,
                    files: vec![file],
                    created_at: stored_timestamp(row.created_at),
                    rationale: row.rationale,
                    confidence: row.confidence.map(|c| c as f32),
                });
            }
        }
    }
    Ok(assertions)
}
