use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("failed to read manifest: {0}")]
    ReadManifest(#[from] std::io::Error),

    #[error("invalid manifest json: {0}")]
    InvalidManifest(#[from] serde_json::Error),

    #[error("duplicate simulation endpoint '{endpoint}'")]
    DuplicateEndpoint { endpoint: String },

    #[error("duplicate output variable '{output}' on endpoint '{endpoint}'")]
    DuplicateOutput { endpoint: String, output: String },

    #[error("duplicate input variable '{input}' on endpoint '{endpoint}'")]
    DuplicateInput { endpoint: String, input: String },

    #[error("route source endpoint '{endpoint}' is not declared in this config")]
    RouteSourceNotLocal { endpoint: String },

    #[error("route source output '{output}' does not exist on endpoint '{endpoint}'")]
    MissingRouteOutput { endpoint: String, output: String },

    #[error("route target input '{input}' does not exist on local endpoint '{endpoint}'")]
    MissingRouteInput { endpoint: String, input: String },

    #[error(
        "route type mismatch: {from_endpoint}/{output} is {output_type:?}, but {to_endpoint}/{input} is {input_type:?}"
    )]
    RouteTypeMismatch {
        from_endpoint: String,
        output: String,
        output_type: PrimitiveType,
        to_endpoint: String,
        input: String,
        input_type: PrimitiveType,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub framework: FrameworkConfig,

    #[serde(default)]
    pub simulations: Vec<SimulationConfig>,

    #[serde(default)]
    pub routes: Vec<RouteConfig>,
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
    pub inputs: Vec<InputConfig>,

    #[serde(default)]
    pub outputs: Vec<OutputConfig>,

    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InputConfig {
    pub name: String,

    #[serde(rename = "type")]
    pub ty: PrimitiveType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputConfig {
    pub name: String,

    #[serde(rename = "type")]
    pub ty: PrimitiveType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteConfig {
    pub from: RouteFrom,
    pub to: RouteTo,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteFrom {
    pub endpoint: String,
    pub output: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteTo {
    pub endpoint: String,
    pub input: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PrimitiveType {
    Float32,
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<Manifest, CoreError> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

pub fn validate_manifest(manifest: &Manifest) -> Result<(), CoreError> {
    let mut endpoints: HashMap<&str, &SimulationConfig> = HashMap::new();
    let mut outputs: HashMap<(String, String), &OutputConfig> = HashMap::new();
    let mut inputs: HashMap<(String, String), &InputConfig> = HashMap::new();

    for sim in &manifest.simulations {
        if endpoints.insert(sim.endpoint.as_str(), sim).is_some() {
            return Err(CoreError::DuplicateEndpoint {
                endpoint: sim.endpoint.clone(),
            });
        }

        let mut local_outputs: HashMap<&str, ()> = HashMap::new();
        for output in &sim.outputs {
            if local_outputs.insert(output.name.as_str(), ()).is_some() {
                return Err(CoreError::DuplicateOutput {
                    endpoint: sim.endpoint.clone(),
                    output: output.name.clone(),
                });
            }
            outputs.insert((sim.endpoint.clone(), output.name.clone()), output);
        }

        let mut local_inputs: HashMap<&str, ()> = HashMap::new();
        for input in &sim.inputs {
            if local_inputs.insert(input.name.as_str(), ()).is_some() {
                return Err(CoreError::DuplicateInput {
                    endpoint: sim.endpoint.clone(),
                    input: input.name.clone(),
                });
            }
            inputs.insert((sim.endpoint.clone(), input.name.clone()), input);
        }
    }

    for route in &manifest.routes {
        if !endpoints.contains_key(route.from.endpoint.as_str()) {
            return Err(CoreError::RouteSourceNotLocal {
                endpoint: route.from.endpoint.clone(),
            });
        }

        let output = outputs
            .get(&(route.from.endpoint.clone(), route.from.output.clone()))
            .ok_or_else(|| CoreError::MissingRouteOutput {
                endpoint: route.from.endpoint.clone(),
                output: route.from.output.clone(),
            })?;

        let input = inputs.get(&(route.to.endpoint.clone(), route.to.input.clone()));

        if endpoints.contains_key(route.to.endpoint.as_str()) && input.is_none() {
            return Err(CoreError::MissingRouteInput {
                endpoint: route.to.endpoint.clone(),
                input: route.to.input.clone(),
            });
        }

        if let Some(input) = input {
            if output.ty != input.ty {
                return Err(CoreError::RouteTypeMismatch {
                    from_endpoint: route.from.endpoint.clone(),
                    output: route.from.output.clone(),
                    output_type: output.ty.clone(),
                    to_endpoint: route.to.endpoint.clone(),
                    input: route.to.input.clone(),
                    input_type: input.ty.clone(),
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_with_sim(sim: SimulationConfig) -> Manifest {
        Manifest {
            framework: FrameworkConfig {
                fps: 1,
                log_level: "info".to_string(),
                max_frames: Some(1),
            },
            simulations: vec![sim],
            routes: Vec::new(),
        }
    }

    fn simulation(inputs: Vec<InputConfig>, outputs: Vec<OutputConfig>) -> SimulationConfig {
        SimulationConfig {
            name: "sim".to_string(),
            endpoint: "127.0.0.1:7001".to_string(),
            plugin: "sim.dll".to_string(),
            inputs,
            outputs,
            params: serde_json::Value::Null,
        }
    }

    #[test]
    fn rejects_duplicate_inputs_on_same_endpoint() {
        let manifest = manifest_with_sim(simulation(
            vec![
                InputConfig {
                    name: "value".to_string(),
                    ty: PrimitiveType::Float32,
                },
                InputConfig {
                    name: "value".to_string(),
                    ty: PrimitiveType::Float32,
                },
            ],
            Vec::new(),
        ));

        let err = validate_manifest(&manifest).expect_err("duplicate input should fail");

        assert!(matches!(
            err,
            CoreError::DuplicateInput { endpoint, input }
                if endpoint == "127.0.0.1:7001" && input == "value"
        ));
    }

    #[test]
    fn rejects_duplicate_outputs_on_same_endpoint() {
        let manifest = manifest_with_sim(simulation(
            Vec::new(),
            vec![
                OutputConfig {
                    name: "value".to_string(),
                    ty: PrimitiveType::Float32,
                },
                OutputConfig {
                    name: "value".to_string(),
                    ty: PrimitiveType::Float32,
                },
            ],
        ));

        let err = validate_manifest(&manifest).expect_err("duplicate output should fail");

        assert!(matches!(
            err,
            CoreError::DuplicateOutput { endpoint, output }
                if endpoint == "127.0.0.1:7001" && output == "value"
        ));
    }

    #[test]
    fn allows_local_inputs_without_routes() {
        let manifest = manifest_with_sim(simulation(
            vec![InputConfig {
                name: "remote_value".to_string(),
                ty: PrimitiveType::Float32,
            }],
            Vec::new(),
        ));

        validate_manifest(&manifest).expect("input may be fed by another process");
    }

    #[test]
    fn rejects_routes_from_remote_sources() {
        let mut manifest = manifest_with_sim(simulation(
            Vec::new(),
            vec![OutputConfig {
                name: "value".to_string(),
                ty: PrimitiveType::Float32,
            }],
        ));
        manifest.routes.push(RouteConfig {
            from: RouteFrom {
                endpoint: "127.0.0.2:7001".to_string(),
                output: "value".to_string(),
            },
            to: RouteTo {
                endpoint: "127.0.0.1:7001".to_string(),
                input: "value".to_string(),
            },
        });

        let err = validate_manifest(&manifest).expect_err("remote source should fail");

        assert!(matches!(
            err,
            CoreError::RouteSourceNotLocal { endpoint }
                if endpoint == "127.0.0.2:7001"
        ));
    }
}
