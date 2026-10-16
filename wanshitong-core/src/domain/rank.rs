use std::cmp::Ordering;
use std::fmt;
use std::ops::Add;

/// How strongly a claim is believed, to resolve conflicts between claims: a
/// claim outranks those it conflicts with if its rank is greater.
///
/// Assertions are what the user says, so they outrank every other claim. A
/// knowledge base result found by an assertion, e.g. an asserted ID's
/// details, outranks heuristics and their results, but not assertions: it only
/// fills in what wasn't asserted.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum Rank {
    /// A heuristic's claim, or of a knowledge base result a heuristic found.
    Number(f32),
    /// Of a knowledge base result found by an assertion, by how much the
    /// knowledge base favored it, e.g. for matching a file's runtime, to tell
    /// several results of one search apart.
    FoundByAssertion(f32),
    /// An assertion's.
    Assertion,
}

impl Rank {
    /// A heuristic's rank, or of a knowledge base result a heuristic found;
    /// None for an assertion's, or of a result an assertion found.
    pub fn as_number(self) -> Option<f32> {
        match self {
            Self::Number(rank) => Some(rank),
            Self::FoundByAssertion(_) | Self::Assertion => None,
        }
    }

    /// A fixed rank for a claim of a knowledge base result about the file
    /// rather than the work, e.g. its role, which competes with other claims
    /// only once the result stands, so ranks as its kind of evidence rather
    /// than as what found the result: `fixed` in the tier of the result's
    /// other claims, so `fixed` for a heuristic's search, and `assertion
    /// +fixed` for an assertion's.
    pub fn fixed(self, fixed: f32) -> Rank {
        match self {
            Self::Number(_) => Self::Number(fixed),
            Self::FoundByAssertion(_) | Self::Assertion => Self::FoundByAssertion(fixed),
        }
    }

    fn tier(self) -> u8 {
        match self {
            Self::Number(_) => 0,
            Self::FoundByAssertion(_) => 1,
            Self::Assertion => 2,
        }
    }
}

impl From<f32> for Rank {
    fn from(rank: f32) -> Self {
        Self::Number(rank)
    }
}

impl Ord for Rank {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Number(a), Self::Number(b))
            | (Self::FoundByAssertion(a), Self::FoundByAssertion(b)) => a.total_cmp(b),
            _ => self.tier().cmp(&other.tier()),
        }
    }
}

impl PartialOrd for Rank {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Rank {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Rank {}

/// A knowledge base's bonus for a result, on the rank of the claim that found
/// it. An assertion's results rank by their bonus, below assertions.
impl Add<f32> for Rank {
    type Output = Rank;

    fn add(self, bonus: f32) -> Rank {
        match self {
            Self::Number(rank) => Self::Number(rank + bonus),
            Self::FoundByAssertion(rank) => Self::FoundByAssertion(rank + bonus),
            Self::Assertion => Self::FoundByAssertion(bonus),
        }
    }
}

/// E.g. `50.0`, `assertion`, or `assertion +2.0` for a result an assertion
/// found.
impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(rank) => write!(f, "{rank:.1}"),
            Self::FoundByAssertion(bonus) => write!(f, "assertion {bonus:+.1}"),
            Self::Assertion => f.write_str("assertion"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assertions_outrank_what_they_find_which_outranks_the_rest() {
        let mut ranks = vec![
            Rank::Assertion,
            Rank::Number(1000.0),
            Rank::Assertion + 2.0,
            Rank::Assertion + 61.5,
            Rank::Number(-1.0),
        ];
        ranks.sort();
        assert_eq!(
            ranks,
            [
                Rank::Number(-1.0),
                Rank::Number(1000.0),
                Rank::FoundByAssertion(2.0),
                Rank::FoundByAssertion(61.5),
                Rank::Assertion,
            ]
        );
        assert_eq!(Rank::Number(1.0) + 2.0, Rank::Number(3.0));
    }

    #[test]
    fn a_fixed_rank_is_in_the_tier_of_the_results_other_claims() {
        assert_eq!(Rank::Number(50.0).fixed(3.5), Rank::Number(3.5));
        assert_eq!(Rank::Assertion.fixed(3.5), Rank::FoundByAssertion(3.5));
        assert_eq!(
            Rank::FoundByAssertion(61.5).fixed(3.5),
            Rank::FoundByAssertion(3.5)
        );
    }
}
