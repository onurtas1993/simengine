use simengine_plugin_api::{simengine_plugin, Simulation, SimulationContext};

struct BasicSim2 {
    ctx: SimulationContext,
    last_seen: Option<i32>,
}

impl Simulation for BasicSim2 {
    fn create(ctx: SimulationContext, config_json: &str) -> Self {
        ctx.info(format!("create() config_json={config_json}"));
        Self { ctx, last_seen: None }
    }

    fn pre_step(&mut self, dt_seconds: f64) {
        self.ctx.info(format!("pre_step() dt={dt_seconds:.6}"));
    }

    fn step(&mut self, dt_seconds: f64) {
        match self.ctx.get_input_i32("counter") {
            Some(counter) => {
                self.last_seen = Some(counter);
                let doubled = counter * 2;

                self.ctx.info(format!(
                    "step() dt={dt_seconds:.6} got input counter={counter}; setting output double_counter={doubled}"
                ));

                self.ctx.set_output_i32("double_counter", doubled);
            }
            None => {
                self.ctx.info(format!(
                    "step() dt={dt_seconds:.6} no input value for counter yet"
                ));
            }
        }
    }

    fn post_step(&mut self, dt_seconds: f64) {
        self.ctx.info(format!(
            "post_step() dt={dt_seconds:.6} last_seen={:?}",
            self.last_seen
        ));
    }

    fn destroy(&mut self) {
        self.ctx.info(format!("destroy() last_seen={:?}", self.last_seen));
    }
}

simengine_plugin!(BasicSim2);
