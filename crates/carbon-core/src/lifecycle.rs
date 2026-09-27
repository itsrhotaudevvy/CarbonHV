use std::collections::BTreeMap;

use crate::{Generation, VmState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAction {
    Start,
    Started,
    Pause,
    Resume,
    Stop,
    Stopped,
    Crash,
    Recover,
    Recovered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    StaleGeneration {
        expected: Generation,
        got: Generation,
    },
    InvalidTransition {
        from: VmState,
        action: LifecycleAction,
    },
}

#[derive(Debug, Clone)]
pub struct LifecycleMachine {
    generation: Generation,
    state: VmState,
    completed_requests: BTreeMap<u64, VmState>,
}

impl LifecycleMachine {
    pub fn new(generation: Generation, state: VmState) -> Self {
        Self {
            generation,
            state,
            completed_requests: BTreeMap::new(),
        }
    }

    pub fn state(&self) -> VmState {
        self.state
    }

    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub fn apply(
        &mut self,
        generation: Generation,
        request_id: u64,
        action: LifecycleAction,
    ) -> Result<VmState, LifecycleError> {
        if generation != self.generation {
            return Err(LifecycleError::StaleGeneration {
                expected: self.generation,
                got: generation,
            });
        }

        if let Some(state) = self.completed_requests.get(&request_id) {
            return Ok(*state);
        }

        let next = match (self.state, action) {
            (VmState::Stopped, LifecycleAction::Start) => VmState::Starting,
            (VmState::Starting, LifecycleAction::Started) => VmState::Running,
            (VmState::Running, LifecycleAction::Pause) => VmState::Paused,
            (VmState::Paused, LifecycleAction::Resume) => VmState::Running,
            (VmState::Running | VmState::Paused, LifecycleAction::Stop) => VmState::Stopping,
            (VmState::Stopping, LifecycleAction::Stopped) => VmState::Stopped,
            (
                VmState::Starting | VmState::Running | VmState::Paused | VmState::Stopping,
                LifecycleAction::Crash,
            ) => VmState::Crashed,
            (VmState::Crashed, LifecycleAction::Recover) => VmState::Recovering,
            (VmState::Recovering, LifecycleAction::Recovered) => VmState::Stopped,
            _ => {
                return Err(LifecycleError::InvalidTransition {
                    from: self.state,
                    action,
                });
            }
        };

        self.state = next;
        self.completed_requests.insert(request_id, next);
        Ok(next)
    }
}
