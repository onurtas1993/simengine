use crate::core::{
    EngineState, Manifest, SimulationState, StateTransitionCondition, StateTransitionConfig,
};
use std::collections::HashMap;

pub struct StateMachine {
    engine_state: EngineState,
    simulation_states: HashMap<String, SimulationState>,
    transitions: Vec<StateTransitionConfig>,
}

impl StateMachine {
    pub fn new(manifest: &Manifest) -> Self {
        let simulation_states = manifest
            .simulations
            .iter()
            .map(|sim| (sim.name.clone(), SimulationState::_START))
            .collect();

        Self {
            engine_state: EngineState::START,
            simulation_states,
            transitions: manifest.state_transitions.clone(),
        }
    }

    pub fn engine_state(&self) -> EngineState {
        self.engine_state
    }

    pub fn should_step(&self) -> bool {
        self.engine_state == EngineState::RUNNING
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.engine_state,
            EngineState::ERROR | EngineState::FINISHED
        )
    }

    pub fn set_simulation_state(&mut self, sim_name: impl Into<String>, state: SimulationState) {
        self.simulation_states.insert(sim_name.into(), state);
    }

    pub fn apply_transitions(&mut self) {
        for transition in &self.transitions {
            if self.transition_matches(transition) && self.engine_state != transition.engine {
                println!(
                    "[runner] engine state transition {:?} -> {:?}",
                    self.engine_state, transition.engine
                );
                self.engine_state = transition.engine;
            }
        }
    }

    fn transition_matches(&self, transition: &StateTransitionConfig) -> bool {
        self.condition_matches(&transition.when)
    }

    fn condition_matches(&self, condition: &StateTransitionCondition) -> bool {
        match (!condition.all.is_empty(), !condition.any.is_empty()) {
            (true, _) => condition
                .all
                .iter()
                .all(|item| self.simulation_state(&item.sim) == item.state),
            (false, true) => condition
                .any
                .iter()
                .any(|item| self.simulation_state(&item.sim) == item.state),
            (false, false) => false,
        }
    }

    fn simulation_state(&self, sim_name: &str) -> SimulationState {
        self.simulation_states
            .get(sim_name)
            .copied()
            .unwrap_or(SimulationState::_START)
    }
}
