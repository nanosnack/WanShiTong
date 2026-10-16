use crate::domain::{ClaimPredicate, DataSourceUuid};
use derive_more::Display;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Display, PartialEq, Eq, Hash, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct AssertionUuid(pub Uuid);

/// A user-authored claim about a set of files, e.g. all of a movie's files,
/// made in a changeset.
#[derive(Clone, Debug)]
pub struct Assertion {
    pub uuid: AssertionUuid,
    pub claim: ClaimPredicate,
    pub files: Vec<AssertedFile>,
    pub created_at: Timestamp,
    /// Why it was made, shown in audit logs.
    pub rationale: Option<String>,
    /// How sure its author was, from 0 to 1, for ordering reviews. It never
    /// affects belief.
    pub confidence: Option<f32>,
}

impl Assertion {
    /// When the assertion was retracted from its last file, if it has been.
    pub fn retracted_at(&self) -> Option<Timestamp> {
        // None while any file's is still active.
        let retracted: Option<Vec<Timestamp>> =
            self.files.iter().map(|file| file.retracted_at).collect();
        retracted?.into_iter().max()
    }
}

/// A file that an assertion is about, named by its path in a data source so
/// that the assertion applies to it in every ingest.
#[derive(Clone, Debug)]
pub struct AssertedFile {
    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    pub rel_path: PathBuf,
    /// Set when the assertion was removed, or replaced for this file by a newer
    /// one of the same kind.
    pub retracted_at: Option<Timestamp>,
}

/// Every version an assertion's claim has been stored as.
#[derive(serde::Deserialize)]
pub(crate) enum VersionedClaimPredicate {
    V1(ClaimPredicate),
}

impl From<VersionedClaimPredicate> for ClaimPredicate {
    fn from(VersionedClaimPredicate::V1(claim): VersionedClaimPredicate) -> Self {
        claim
    }
}

impl crate::json::Stored for ClaimPredicate {
    type Versions = VersionedClaimPredicate;
    const VERSION: &'static str = "V1";
}
