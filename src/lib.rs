//! SimEngine framework library.
//!
//! This single crate contains the runtime-facing core model, network layer,
//! and plugin API/ABI. The package also ships the `simengine` CLI binary.

pub mod core;
pub mod network;
pub mod plugin_api;

pub use plugin_api::{
    GetSimApiFn, SimApi, SimContext, SimLogLevel, Simulation, SimulationContext,
    SIMENGINE_API_VERSION,
};
