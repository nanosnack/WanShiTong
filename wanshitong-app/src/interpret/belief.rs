//! Belief policy: resolving conflicts between claims about the same subject.
//! Files aren't settled against each other: two numbered as one episode are
//! two manifestations of it (see *Belief Policy* in `spec/domain-model.md`).

use std::collections::HashMap;
use wanshitong_core::domain::{RankedClaimPredicate, SubjectIndex};

pub(super) fn deactivate_duplicate_claims(
    claims: &mut HashMap<SubjectIndex, Vec<RankedClaimPredicate>>,
) {
    for claims in claims.values_mut() {
        // PERF: O(N^2) loop.
        //
        // Assuming the set of claims on a single subject is relatively small N.
        //
        for i in 0..claims.len() {
            for j in i + 1..claims.len() {
                let l = &claims[i];
                let r = &claims[j];
                if l.pred == r.pred {
                    if l.rank > r.rank {
                        claims[j].active = false;
                    } else {
                        claims[i].active = false;
                    }
                }
            }
        }
    }
}

pub(super) fn apply_belief_policy(claims: &mut HashMap<SubjectIndex, Vec<RankedClaimPredicate>>) {
    // Choose the best claims by resolving conflicts on a per-subject basis.
    for subject_claims in claims.values_mut() {
        keep_best_kb_result(subject_claims);
        resolve_conflicts(subject_claims);
    }
}

/// A file is at most one work, so at most one knowledge base result about it
/// can be right. Keeps the highest-ranked result's claims and deactivates all
/// of the others', so that e.g. one result's IMDB ID can't fill in for
/// another's missing one.
fn keep_best_kb_result(claims: &mut [RankedClaimPredicate]) {
    let best = claims
        .iter()
        .filter(|claim| claim.active)
        .filter_map(|claim| Some((claim.kb_result()?, claim.rank)))
        .max_by_key(|&(_, rank)| rank)
        .map(|(kb_result, _)| kb_result);
    for claim in claims {
        if claim
            .kb_result()
            .is_some_and(|kb_result| Some(kb_result) != best)
        {
            claim.active = false;
        }
    }
}

// PERF: O(N^2) loop.
//
// Assuming the set of claims on a single subject is relatively small N.
//
/// From the highest-ranked claim down, a claim stays active unless it
/// conflicts with one that did. Claims that are already inactive defeat
/// nothing.
fn resolve_conflicts(claims: &mut [RankedClaimPredicate]) {
    let mut by_rank: Vec<usize> = (0..claims.len()).collect();
    by_rank.sort_by_key(|&i| std::cmp::Reverse(claims[i].rank));
    let mut accepted: Vec<usize> = Vec::new();
    for i in by_rank {
        if !claims[i].active {
            continue;
        }
        if accepted
            .iter()
            .any(|&a| claims[a].pred.conflicts_with(&claims[i].pred))
        {
            claims[i].active = false;
        } else {
            accepted.push(i);
        }
    }
}
