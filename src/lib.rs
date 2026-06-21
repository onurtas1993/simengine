//! SimEngine framework library.
//!
//! This single crate contains the runtime-facing core model, TCP network layer,
//! and plugin API/ABI. The package also ships the `simengine` CLI binary.

pub mod cli;
pub mod core;
pub mod network;
pub mod plugin_api;
pub mod runtime;

pub use plugin_api::{
    GetSimApiFn, SIMENGINE_API_VERSION, SimApi, SimContext, SimLogLevel, Simulation,
    SimulationContext,
};
