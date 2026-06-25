mod state;

pub use state::{
    EngineState, SimulationState, SimulationStateCondition, StateTransitionCondition,
    StateTransitionConfig,
};

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use thiserror::Error;

type EndpointMap<'a> = HashMap<&'a str, &'a SimulationConfig>;
type InstrumentMap<'a> = HashMap<&'a str, &'a InstrumentConfig>;

struct ManifestDeclarations<'a> {
    endpoints: EndpointMap<'a>,
    instruments: InstrumentMap<'a>,
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("failed to read manifest: {0}")]
    ReadManifest(std::io::Error),

    #[error("invalid manifest json: {0}")]
    InvalidManifest(serde_json::Error),

    #[error("failed to read instruments file '{path}': {source}")]
    ReadInstruments {
        path: String,
        source: std::io::Error,
    },

    #[error("invalid instruments json in '{path}': {source}")]
    InvalidInstruments {
        path: String,
        source: serde_json::Error,
    },

    #[error("failed to read instrument flows file '{path}': {source}")]
    ReadInstrumentFlows {
        path: String,
        source: std::io::Error,
    },

    #[error("invalid instrument flows json in '{path}': {source}")]
    InvalidInstrumentFlows {
        path: String,
        source: serde_json::Error,
    },

    #[error("duplicate simulation endpoint '{endpoint}'")]
    DuplicateEndpoint { endpoint: String },

    #[error("duplicate instrument '{instrument}'")]
    DuplicateInstrument { instrument: String },

    #[error("instrument flow source endpoint '{endpoint}' is not declared in this config")]
    FlowSourceNotLocal { endpoint: String },

    #[error("instrument flow references unknown instrument '{instrument}'")]
    UnknownFlowInstrument { instrument: String },

    #[error("state transition references unknown simulation '{simulation}'")]
    UnknownTransitionSimulation { simulation: String },

    #[error("state transition has an empty condition")]
    EmptyStateTransitionCondition,

    #[error("state transition condition must use either 'all' or 'any', not both")]
    AmbiguousStateTransitionCondition,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub framework: FrameworkConfig,

    #[serde(default)]
    pub simulations: Vec<SimulationConfig>,

    #[serde(default)]
    pub state_transitions: Vec<StateTransitionConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FrameworkConfig {
    pub fps: u32,

    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default)]
    pub max_frames: Option<u64>,
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SimulationConfig {
    pub name: String,
    pub endpoint: String,
    pub plugin: String,

    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstrumentConfig {
    pub value: String,

    #[serde(rename = "type")]
    pub ty: PrimitiveType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstrumentFlow {
    pub instrument: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PrimitiveType {
    Float32,
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<Manifest, CoreError> {
    let text = fs::read_to_string(path).map_err(CoreError::ReadManifest)?;
    serde_json::from_str(&text).map_err(CoreError::InvalidManifest)
}

pub fn load_instruments(path: impl AsRef<Path>) -> Result<Vec<InstrumentConfig>, CoreError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| CoreError::ReadInstruments {
        path: path.display().to_string(),
        source,
    })?;

    serde_json::from_str(&text).map_err(|source| CoreError::InvalidInstruments {
        path: path.display().to_string(),
        source,
    })
}

pub fn load_instrument_flows(path: impl AsRef<Path>) -> Result<Vec<InstrumentFlow>, CoreError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| CoreError::ReadInstrumentFlows {
        path: path.display().to_string(),
        source,
    })?;

    serde_json::from_str(&text).map_err(|source| CoreError::InvalidInstrumentFlows {
        path: path.display().to_string(),
        source,
    })
}

pub fn validate_manifest(
    manifest: &Manifest,
    instruments: &[InstrumentConfig],
    flows: &[InstrumentFlow],
) -> Result<(), CoreError> {
    let declarations = collect_declarations(&manifest.simulations, instruments)?;

    validate_flows(flows, &declarations)?;

    state::validate_state_transitions(&manifest.state_transitions, &manifest.simulations)?;

    Ok(())
}

fn collect_declarations<'a>(
    simulations: &'a [SimulationConfig],
    instruments: &'a [InstrumentConfig],
) -> Result<ManifestDeclarations<'a>, CoreError> {
    let mut endpoints = HashMap::new();
    let mut instrument_map = HashMap::new();

    for sim in simulations {
        if endpoints.insert(sim.endpoint.as_str(), sim).is_some() {
            return Err(CoreError::DuplicateEndpoint {
                endpoint: sim.endpoint.clone(),
            });
        }
    }

    for instrument in instruments {
        if instrument_map
            .insert(instrument.value.as_str(), instrument)
            .is_some()
        {
            return Err(CoreError::DuplicateInstrument {
                instrument: instrument.value.clone(),
            });
        }
    }

    Ok(ManifestDeclarations {
        endpoints,
        instruments: instrument_map,
    })
}

fn validate_flows(
    flows: &[InstrumentFlow],
    declarations: &ManifestDeclarations<'_>,
) -> Result<(), CoreError> {
    for flow in flows {
        validate_flow(flow, declarations)?;
    }

    Ok(())
}

fn validate_flow(
    flow: &InstrumentFlow,
    declarations: &ManifestDeclarations<'_>,
) -> Result<(), CoreError> {
    if !declarations.endpoints.contains_key(flow.from.as_str()) {
        return Err(CoreError::FlowSourceNotLocal {
            endpoint: flow.from.clone(),
        });
    }

    if !declarations
        .instruments
        .contains_key(flow.instrument.as_str())
    {
        return Err(CoreError::UnknownFlowInstrument {
            instrument: flow.instrument.clone(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_with_sims(simulations: Vec<SimulationConfig>) -> Manifest {
        Manifest {
            framework: FrameworkConfig {
                fps: 1,
                log_level: "info".to_string(),
                max_frames: Some(1),
            },
            simulations,
            state_transitions: Vec::new(),
        }
    }

    fn simulation(name: &str, endpoint: &str) -> SimulationConfig {
        SimulationConfig {
            name: name.to_string(),
            endpoint: endpoint.to_string(),
            plugin: "sim.dll".to_string(),
            params: serde_json::Value::Null,
        }
    }

    fn instrument(value: &str, ty: PrimitiveType) -> InstrumentConfig {
        InstrumentConfig {
            value: value.to_string(),
            ty,
        }
    }

    #[test]
    fn rejects_duplicate_instruments() {
        let manifest = manifest_with_sims(vec![simulation("sensor", "127.0.0.1:7001")]);
        let instruments = vec![
            instrument("temperature", PrimitiveType::Float32),
            instrument("temperature", PrimitiveType::Float32),
        ];
        let flows = Vec::new();

        let err =
            validate_manifest(&manifest, &instruments, &flows).expect_err("duplicate should fail");

        assert!(matches!(
            err,
            CoreError::DuplicateInstrument { instrument } if instrument == "temperature"
        ));
    }

    #[test]
    fn accepts_flow_when_source_and_instrument_exist() {
        let manifest = manifest_with_sims(vec![
            simulation("sensor", "127.0.0.1:7001"),
            simulation("controller", "127.0.0.1:7002"),
        ]);
        let instruments = vec![instrument("temperature", PrimitiveType::Float32)];
        let flows = vec![InstrumentFlow {
            instrument: "temperature".to_string(),
            from: "127.0.0.1:7001".to_string(),
            to: "127.0.0.1:7002".to_string(),
        }];

        validate_manifest(&manifest, &instruments, &flows).expect("flow should be valid");
    }

    #[test]
    fn rejects_flow_with_unknown_instrument() {
        let manifest = manifest_with_sims(vec![simulation("sensor", "127.0.0.1:7001")]);
        let instruments = Vec::new();
        let flows = vec![InstrumentFlow {
            instrument: "temperature".to_string(),
            from: "127.0.0.1:7001".to_string(),
            to: "127.0.0.1:7002".to_string(),
        }];

        let err =
            validate_manifest(&manifest, &instruments, &flows).expect_err("unknown instrument");

        assert!(matches!(
            err,
            CoreError::UnknownFlowInstrument { instrument } if instrument == "temperature"
        ));
    }

    #[test]
    fn rejects_flows_from_remote_sources() {
        let manifest = manifest_with_sims(vec![simulation("sensor", "127.0.0.1:7001")]);
        let instruments = vec![instrument("value", PrimitiveType::Float32)];
        let flows = vec![InstrumentFlow {
            instrument: "value".to_string(),
            from: "127.0.0.2:7001".to_string(),
            to: "127.0.0.1:7001".to_string(),
        }];

        let err = validate_manifest(&manifest, &instruments, &flows)
            .expect_err("remote source should fail");

        assert!(matches!(
            err,
            CoreError::FlowSourceNotLocal { endpoint }
                if endpoint == "127.0.0.2:7001"
        ));
    }

    #[test]
    fn rejects_state_transition_for_unknown_simulation() {
        let mut manifest = manifest_with_sims(vec![simulation("sim", "127.0.0.1:7001")]);
        manifest.state_transitions.push(StateTransitionConfig {
            when: StateTransitionCondition {
                all: vec![SimulationStateCondition {
                    sim: "missing".to_string(),
                    state: SimulationState::_READY,
                }],
                any: Vec::new(),
            },
            engine: EngineState::RUNNING,
        });
        let instruments = Vec::new();
        let flows = Vec::new();

        let err = validate_manifest(&manifest, &instruments, &flows)
            .expect_err("unknown simulation should fail");

        assert!(matches!(
            err,
            CoreError::UnknownTransitionSimulation { simulation }
                if simulation == "missing"
        ));
    }

    #[test]
    fn rejects_empty_state_transition_condition() {
        let mut manifest = manifest_with_sims(vec![simulation("sim", "127.0.0.1:7001")]);
        manifest.state_transitions.push(StateTransitionConfig {
            when: StateTransitionCondition {
                all: Vec::new(),
                any: Vec::new(),
            },
            engine: EngineState::RUNNING,
        });
        let instruments = Vec::new();
        let flows = Vec::new();

        let err = validate_manifest(&manifest, &instruments, &flows)
            .expect_err("empty condition should fail");

        assert!(matches!(err, CoreError::EmptyStateTransitionCondition));
    }
}
