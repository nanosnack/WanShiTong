use crate::domain::{ExternalId, RankedClaimPredicate};
use serde::{Deserialize, Serialize};

/// A per-subject audit log.
///
/// This gets persisted in SQL on a record per (interpretation, subject).
///
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SubjectAuditLog {
    /// Sorted descending by (active, rank).
    pub claims: Vec<RankedClaimPredicate>,
    pub kb_queries: Vec<KbQueryEvent>,
    // TODO: represent conflicts between claims
    // This could be done as a graph or more simply as a ranked list for each
    // type of claim.
}

#[derive(Clone, Serialize, Deserialize)]
pub enum KbQueryEvent {
    /// A work by another knowledge base's ID, e.g. an IMDb ID.
    Find {
        id: ExternalId,
        /// What it found, if anything. None too in logs from before it was
        /// recorded.
        #[serde(default)]
        found: Option<KbLookupResult>,
    },
    MovieSearch {
        query: String,
        /// The first results, in the knowledge base's order.
        results: Vec<KbSearchResult>,
    },
    TvSeriesSearch {
        query: String,
        /// The first results, in the knowledge base's order.
        results: Vec<KbSearchResult>,
    },
    MovieDetails {
        id: ExternalId,
        /// The movie, if there's one by the ID. None too in logs from before
        /// it was recorded.
        #[serde(default)]
        found: Option<KbLookupResult>,
    },
    TvSeriesDetails {
        id: ExternalId,
        /// The series, if there's one by the ID. None too in logs from before
        /// it was recorded.
        #[serde(default)]
        found: Option<KbLookupResult>,
    },
    /// A season of a series, with its episodes, e.g. season 0, its specials.
    TvSeasonDetails { id: ExternalId, season: u32 },
    /// A movie's or a series' other titles, e.g. in other languages or
    /// romanized, by which its search may have found it.
    AlternativeTitles { id: ExternalId },
}

/// A work that a knowledge base search found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KbSearchResult {
    pub id: ExternalId,
    pub title: String,
    pub year: Option<u32>,
}

/// A work that a knowledge base lookup by its ID found, with what was
/// weighed of it: its runtime, if it's a movie and the knowledge base says,
/// as runtime matching goes by it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KbLookupResult {
    pub id: ExternalId,
    pub title: String,
    pub year: Option<u32>,
    pub runtime_minutes: Option<u32>,
}

/// How many of a search's results an audit log keeps.
pub const KB_SEARCH_RESULTS_LOGGED: usize = 5;

/// Every version an audit log has been stored as.
#[derive(Deserialize)]
pub(crate) enum VersionedSubjectAuditLog {
    V1(SubjectAuditLog),
}

impl From<VersionedSubjectAuditLog> for SubjectAuditLog {
    fn from(VersionedSubjectAuditLog::V1(log): VersionedSubjectAuditLog) -> Self {
        log
    }
}

impl crate::json::Stored for SubjectAuditLog {
    type Versions = VersionedSubjectAuditLog;
    const VERSION: &'static str = "V1";
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A log stored before lookups recorded what they found still reads.
    #[test]
    fn a_log_from_before_lookups_recorded_their_results_reads() {
        let stored = r#"{"V1":{"claims":[],"kb_queries":[{"MovieDetails":{"id":{"Tmdb":603}}}]}}"#;
        let log: SubjectAuditLog = crate::json::decode(stored).unwrap();
        assert!(matches!(
            &log.kb_queries[..],
            [KbQueryEvent::MovieDetails {
                id: ExternalId::Tmdb(603),
                found: None
            }]
        ));
    }
}
