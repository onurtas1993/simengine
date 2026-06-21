//! Public API used by simulation plugins.
//!
//! Simulation authors normally use [`Simulation`], [`SimulationContext`], and
//! [`simengine_plugin!`]. The C ABI is split into `abi.rs` so normal simulation
//! code does not need to touch raw pointers or `extern "C"` functions.

mod abi;
mod context;
mod macros;

pub use abi::{GetSimApiFn, SIMENGINE_API_VERSION, SimApi, SimContext, SimLogLevel};
pub use context::SimulationContext;

/// Trait implemented by simulation plugins.
///
/// The runner calls these lifecycle methods through generated C ABI glue.
pub trait Simulation: Sized {
    fn create(ctx: SimulationContext, config_json: &str) -> Self;

    fn pre_step(&mut self, _dt_seconds: f64) {}

    fn step(&mut self, dt_seconds: f64);

    fn post_step(&mut self, _dt_seconds: f64) {}

    fn destroy(&mut self) {}
}
