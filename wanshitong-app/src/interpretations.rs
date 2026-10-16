//! Interpretations: deriving catalogs from ingests, and choosing which one is
//! canonical.

use crate::AppContext;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};
use wanshitong_core::domain::{Interpretation, InterpretationOutcome, InterpretationRowId};

/// An interpretation along with whether it's still being derived.
pub struct InterpretationStatus {
    pub interpretation: Interpretation,
    pub state: InterpretationState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterpretationState {
    Running,
    /// Stopped without finishing, e.g. because the server was restarted.
    Interrupted,
    Failed,
    Succeeded,
}

impl InterpretationStatus {
    pub(crate) fn new(ctx: &AppContext, interpretation: Interpretation) -> Self {
        // Running until its task is done, which may be a little after it's
        // recorded as finished, e.g. while it's made canonical.
        let state = match interpretation.outcome() {
            _ if ctx.running_interpretations.contains(interpretation.id) => {
                InterpretationState::Running
            }
            InterpretationOutcome::Pending => InterpretationState::Interrupted,
            InterpretationOutcome::Failed => InterpretationState::Failed,
            InterpretationOutcome::Success => InterpretationState::Succeeded,
        };
        Self {
            interpretation,
            state,
        }
    }
}

/// The interpretations whose catalogs are being derived by this process.
#[derive(Clone, Default)]
pub(crate) struct RunningInterpretations(Arc<Mutex<HashSet<InterpretationRowId>>>);

impl RunningInterpretations {
    fn contains(&self, id: InterpretationRowId) -> bool {
        self.0.lock().unwrap().contains(&id)
    }

    /// Marks an interpretation running until the returned guard is dropped.
    pub(crate) fn start(&self, id: InterpretationRowId) -> RunningInterpretation {
        self.0.lock().unwrap().insert(id);
        RunningInterpretation {
            running: self.clone(),
            id,
        }
    }
}

pub(crate) struct RunningInterpretation {
    running: RunningInterpretations,
    id: InterpretationRowId,
}

impl RunningInterpretation {
    pub(crate) fn id(&self) -> InterpretationRowId {
        self.id
    }
}

impl Drop for RunningInterpretation {
    fn drop(&mut self) {
        self.running.0.lock().unwrap().remove(&self.id);
    }
}
