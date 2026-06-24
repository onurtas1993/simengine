use super::{CoreError, SimulationConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateTransitionConfig {
    pub when: StateTransitionCondition,
    pub engine: EngineState,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateTransitionCondition {
    #[serde(default)]
    pub all: Vec<SimulationStateCondition>,

    #[serde(default)]
    pub any: Vec<SimulationStateCondition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SimulationStateCondition {
    pub sim: String,
    pub state: SimulationState,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum EngineState {
    START,
    READY,
    RUNNING,
    FINISHED,
    ERROR,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum SimulationState {
    _START,
    _READY,
    _RUNNING,
    _FINISHED,
    _ERROR,
}

pub fn validate_state_transitions(
    transitions: &[StateTransitionConfig],
    simulations: &[SimulationConfig],
) -> Result<(), CoreError> {
    for transition in transitions {
        validate_state_transition(transition, simulations)?;
    }

    Ok(())
}

fn validate_state_transition(
    transition: &StateTransitionConfig,
    simulations: &[SimulationConfig],
) -> Result<(), CoreError> {
    let has_all = !transition.when.all.is_empty();
    let has_any = !transition.when.any.is_empty();

    match (has_all, has_any) {
        (false, false) => return Err(CoreError::EmptyStateTransitionCondition),
        (true, true) => return Err(CoreError::AmbiguousStateTransitionCondition),
        _ => {}
    }

    for condition in transition.when.all.iter().chain(&transition.when.any) {
        if !simulations.iter().any(|sim| sim.name == condition.sim) {
            return Err(CoreError::UnknownTransitionSimulation {
                simulation: condition.sim.clone(),
            });
        }
    }

    Ok(())
}
