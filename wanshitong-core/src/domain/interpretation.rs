use crate::domain::{ChangesetState, ChangesetUuid, DataSourceUuid, IngestUuid, PathId};
use derive_more::Display;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Display, PartialEq, Eq, Hash, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct InterpretationRowId(pub i64);

#[derive(Clone, Copy, Debug, Display, PartialEq, Eq, Hash, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct InterpretationUuid(pub Uuid);

pub struct Interpretation {
    pub id: InterpretationRowId,
    pub uuid: InterpretationUuid,
    pub name: String,
    pub started_at: Timestamp,
    pub stopped_at: Option<Timestamp>,
    /// Why deriving the catalog failed.
    pub error: Option<String>,
    pub is_canonical: bool,
    pub movie_count: u64,
    pub episode_count: u64,
    /// How many things it couldn't do and went on without: see
    /// [`InterpretationError`].
    pub error_count: u64,
    /// The ingest chosen for each data source, by data source name.
    pub ingests: Vec<InterpretationIngest>,
    /// The changeset it's of, if it includes one's changes: what accepting it
    /// would make, or made. None for an interpretation of only the accepted
    /// assertions.
    pub changeset: Option<InterpretationChangeset>,
}

impl Interpretation {
    /// How the interpretation went: pending until it stopped, as recorded.
    pub fn outcome(&self) -> InterpretationOutcome {
        match (&self.stopped_at, &self.error) {
            (None, _) => InterpretationOutcome::Pending,
            (Some(_), Some(_)) => InterpretationOutcome::Failed,
            (Some(_), None) => InterpretationOutcome::Success,
        }
    }
}

/// Something an interpretation couldn't do, and went on without, e.g. a
/// knowledge base search for a file. Its catalog is complete but for these.
/// Unlike [`Interpretation::error`], which is why it failed altogether.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterpretationError {
    /// The file it was about, if it was about one and that file still exists.
    pub file: Option<PathId>,
    /// What was being done, e.g. "Searching the knowledge base".
    pub subject: String,
    pub error: String,
}

/// The changeset an interpretation is of, as it stands now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterpretationChangeset {
    pub uuid: ChangesetUuid,
    pub author: String,
    pub description: String,
    pub state: ChangesetState,
}

pub struct InterpretationIngest {
    pub data_source_uuid: DataSourceUuid,
    pub data_source_name: String,
    pub ingest_uuid: IngestUuid,
    pub ingest_started_at: Timestamp,
}

#[derive(Clone, Copy, Debug, Display, PartialEq, Eq)]
pub enum InterpretationOutcome {
    /// NOTE: An interpretation whose task died without finishing stays
    /// pending forever. Only the app knows whether its task is still running.
    Pending,
    Failed,
    Success,
}
