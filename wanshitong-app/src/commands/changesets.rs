//! Changesets: making assertions and retractions together, seeing what they
//! would change, then accepting or discarding them. See
//! `spec/domain-model.md`. The draft is the open changeset that the web UI's
//! corrections go into, to be reviewed and accepted together.

mod accept_changeset;
mod add_assertions;
mod changeset_interpretation;
mod correct_in_draft;
mod create_changeset;
mod diff_changeset;
mod discard_changeset;
mod find_draft;
mod import_assertions;
mod interpret_changeset;
mod open_draft;
mod retract_in_changeset;
mod take_back_from_changeset;

pub use accept_changeset::accept_changeset;
pub use add_assertions::add_assertions;
pub use correct_in_draft::correct_in_draft;
pub use create_changeset::create_changeset;
pub use diff_changeset::diff_changeset;
pub use discard_changeset::discard_changeset;
pub use find_draft::find_draft;
pub use import_assertions::{
    AssertionImport, ImportedAssertion, ImportedAssertions, ImportedFile, import_assertions,
};
pub use interpret_changeset::{
    current_changeset_interpretation, interpret_changeset, start_changeset_interpretation,
};
pub use open_draft::open_draft;
pub use retract_in_changeset::retract_in_changeset;
pub use take_back_from_changeset::take_back_from_changeset;

/// Who the web UI's changesets are by.
pub const USER: &str = "user";
