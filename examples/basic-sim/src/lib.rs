use simengine_plugin_api::{simengine_plugin, Simulation, SimulationContext};

struct BasicSim {
    ctx: SimulationContext,
    counter: i32,
}

impl Simulation for BasicSim {
    fn create(ctx: SimulationContext, config_json: &str) -> Self {
        ctx.info(format!("create() config_json={config_json}"));
        Self { ctx, counter: 0 }
    }

    fn pre_step(&mut self, dt_seconds: f64) {
        self.ctx.info(format!(
            "pre_step() counter={} dt={dt_seconds:.6}",
            self.counter,
        ));
    }

    fn step(&mut self, dt_seconds: f64) {
        self.counter += 1;
        self.ctx.info(format!(
            "step() counter={} dt={dt_seconds:.6}",
            self.counter,
        ));

        self.ctx.set_output_i32("counter", self.counter);
    }

    fn post_step(&mut self, dt_seconds: f64) {
        self.ctx.info(format!(
            "post_step() counter={} dt={dt_seconds:.6}",
            self.counter,
        ));
    }

    fn destroy(&mut self) {
        self.ctx.info(format!("destroy() final_counter={}", self.counter));
    }
}

simengine_plugin!(BasicSim);
