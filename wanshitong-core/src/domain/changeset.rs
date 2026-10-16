use crate::domain::{Assertion, AssertionUuid};
use crate::{CoreError, CoreErrorContext};
use derive_more::Display;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Display, PartialEq, Eq, Hash, Deserialize, Serialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct ChangesetUuid(pub Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct ChangesetRowId(pub i64);

/// Assertions and retractions made together, e.g. one correction in the web
/// UI, or one agent run. See `spec/domain-model.md`.
#[derive(Clone, Debug)]
pub struct ChangesetDetails {
    pub uuid: ChangesetUuid,
    /// Who made it, e.g. "user", or an agent run's name.
    pub author: String,
    pub description: String,
    pub state: ChangesetState,
    pub created_at: Timestamp,
    /// When it was accepted or discarded.
    pub closed_at: Option<Timestamp>,
    /// The assertions it makes, newest first, including those withdrawn while
    /// it was open, whose files are all retracted.
    pub assertions: Vec<Assertion>,
    /// The accepted assertions it retracts, or retracted.
    pub retractions: Vec<AssertionUuid>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ChangesetState {
    /// Its changes are pending: only its own interpretations include them.
    Open,
    /// Its changes took effect.
    Accepted,
    /// Its pending changes were dropped.
    Discarded,
}

impl ChangesetState {
    pub(crate) fn as_db_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Accepted => "accepted",
            Self::Discarded => "discarded",
        }
    }

    pub(crate) fn from_db_str(s: &str) -> Result<Self, CoreError> {
        match s {
            "open" => Ok(Self::Open),
            "accepted" => Ok(Self::Accepted),
            "discarded" => Ok(Self::Discarded),
            _ => Err(CoreErrorContext::UnknownChangesetState(s.to_string()).into()),
        }
    }
}
