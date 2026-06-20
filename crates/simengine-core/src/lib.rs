use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("failed to read manifest: {0}")]
    ReadManifest(#[from] std::io::Error),

    #[error("invalid manifest json: {0}")]
    InvalidManifest(#[from] serde_json::Error),

    #[error("input '{input}' required by simulation '{consumer}' has no matching output")]
    MissingInputProducer { consumer: String, input: String },

    #[error("input '{input}' required by simulation '{consumer}' is ambiguous; matching outputs: {matches:?}")]
    AmbiguousInputProducer {
        consumer: String,
        input: String,
        matches: Vec<String>,
    },

    #[error("duplicate output variable '{output}' on simulation '{simulation}'")]
    DuplicateOutput { simulation: String, output: String },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub framework: FrameworkConfig,

    #[serde(default)]
    pub network: NetworkConfig,

    pub simulations: Vec<SimulationConfig>,
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
pub struct NetworkConfig {
    #[serde(default = "default_network_mode")]
    pub mode: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mode: default_network_mode(),
        }
    }
}

fn default_network_mode() -> String {
    "in_process".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SimulationConfig {
    pub name: String,
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
    let mut outputs: HashMap<String, &OutputConfig> = HashMap::new();

    for sim in &manifest.simulations {
        let mut local_outputs: HashMap<&str, ()> = HashMap::new();

        for output in &sim.outputs {
            if local_outputs.insert(output.name.as_str(), ()).is_some() {
                return Err(CoreError::DuplicateOutput {
                    simulation: sim.name.clone(),
                    output: output.name.clone(),
                });
            }

            outputs.insert(format!("{}.{}", sim.name, output.name), output);
        }
    }

    for sim in &manifest.simulations {
        for input in &sim.inputs {
            let matches: Vec<String> = outputs
                .iter()
                .filter(|(_, output)| output.name == input.name && output.ty == input.ty)
                .map(|(key, _)| key.clone())
                .collect();

            match matches.len() {
                0 => {
                    return Err(CoreError::MissingInputProducer {
                        consumer: sim.name.clone(),
                        input: input.name.clone(),
                    });
                }
                1 => {}
                _ => {
                    return Err(CoreError::AmbiguousInputProducer {
                        consumer: sim.name.clone(),
                        input: input.name.clone(),
                        matches,
                    });
                }
            }
        }
    }

    Ok(())
}
