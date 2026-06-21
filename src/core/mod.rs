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

    #[error("input '{input}' on endpoint '{endpoint}' has no link")]
    MissingInputLink { endpoint: String, input: String },

    #[error("input '{input}' on endpoint '{endpoint}' has multiple links")]
    AmbiguousInputLink { endpoint: String, input: String },

    #[error("link source output '{output}' does not exist on endpoint '{endpoint}'")]
    MissingLinkOutput { endpoint: String, output: String },

    #[error("link target input '{input}' does not exist on endpoint '{endpoint}'")]
    MissingLinkInput { endpoint: String, input: String },

    #[error(
        "link type mismatch: {from_endpoint}/{output} is {output_type:?}, but {to_endpoint}/{input} is {input_type:?}"
    )]
    LinkTypeMismatch {
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
    pub links: Vec<LinkConfig>,
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
pub struct LinkConfig {
    pub from: LinkFrom,
    pub to: LinkTo,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinkFrom {
    pub endpoint: String,
    pub output: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinkTo {
    pub endpoint: String,
    pub input: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PrimitiveType {
    Bool,
    Int32,
    Int64,
    Float32,
    Float64,
    String,
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

    for link in &manifest.links {
        let output = outputs.get(&(link.from.endpoint.clone(), link.from.output.clone()));
        let input = inputs.get(&(link.to.endpoint.clone(), link.to.input.clone()));

        if endpoints.contains_key(link.from.endpoint.as_str()) && output.is_none() {
            return Err(CoreError::MissingLinkOutput {
                endpoint: link.from.endpoint.clone(),
                output: link.from.output.clone(),
            });
        }

        if endpoints.contains_key(link.to.endpoint.as_str()) && input.is_none() {
            return Err(CoreError::MissingLinkInput {
                endpoint: link.to.endpoint.clone(),
                input: link.to.input.clone(),
            });
        }

        if let (Some(output), Some(input)) = (output, input) {
            if output.ty != input.ty {
                return Err(CoreError::LinkTypeMismatch {
                    from_endpoint: link.from.endpoint.clone(),
                    output: link.from.output.clone(),
                    output_type: output.ty.clone(),
                    to_endpoint: link.to.endpoint.clone(),
                    input: link.to.input.clone(),
                    input_type: input.ty.clone(),
                });
            }
        }
    }

    for sim in &manifest.simulations {
        for input in &sim.inputs {
            let count = manifest
                .links
                .iter()
                .filter(|link| link.to.endpoint == sim.endpoint && link.to.input == input.name)
                .count();

            match count {
                0 => {
                    return Err(CoreError::MissingInputLink {
                        endpoint: sim.endpoint.clone(),
                        input: input.name.clone(),
                    });
                }
                1 => {}
                _ => {
                    return Err(CoreError::AmbiguousInputLink {
                        endpoint: sim.endpoint.clone(),
                        input: input.name.clone(),
                    });
                }
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
            links: Vec::new(),
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
                    ty: PrimitiveType::Int32,
                },
                InputConfig {
                    name: "value".to_string(),
                    ty: PrimitiveType::Int32,
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
                    ty: PrimitiveType::Int32,
                },
                OutputConfig {
                    name: "value".to_string(),
                    ty: PrimitiveType::Int32,
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
}
