use std::cmp::Reverse;
use std::collections::HashMap;
use wanshitong_core::domain::{KbQueryEvent, RankedClaimPredicate, SubjectAuditLog, SubjectIndex};

/// Captures events from the interpretation process.
#[derive(Default)]
pub struct AuditLogger {
    kb_queries: Vec<(SubjectIndex, KbQueryEvent)>,
}

impl AuditLogger {
    pub fn log_kb_query(&mut self, subject: SubjectIndex, query: KbQueryEvent) {
        self.kb_queries.push((subject, query));
    }

    pub fn finish(
        self,
        claims: HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
    ) -> HashMap<SubjectIndex, SubjectAuditLog> {
        let Self { kb_queries } = self;

        let mut audit_logs = HashMap::with_capacity(claims.len());
        for (subject, claims) in claims.into_iter() {
            let log: &mut SubjectAuditLog = audit_logs.entry(subject).or_default();
            log.claims = claims;
            log.claims
                .sort_by_key(|claim| (!claim.active, Reverse(claim.rank)));
        }

        for (subject, query) in kb_queries {
            let Some(log) = audit_logs.get_mut(&subject) else {
                continue;
            };
            log.kb_queries.push(query);
        }

        audit_logs
    }
}
