use super::host::{
    HostContext, HostShared, host_get_input, host_log, host_set_output, host_set_state,
};
use super::state_machine::StateMachine;
use crate::{
    core::{Manifest, SimulationConfig},
    plugin_api::{GetSimApiFn, SIMENGINE_API_VERSION, SimApi, SimContext},
};
use anyhow::{Context, Result};
use libloading::{Library, Symbol};
use std::{
    collections::HashSet,
    ffi::{CString, c_void},
    path::Path,
    sync::{Arc, Mutex},
};

pub struct LoadedSim {
    _lib: Library,
    api: SimApi,
    instance: *mut c_void,
    _host_ctx: Box<HostContext>,
    destroyed: bool,
}

impl LoadedSim {
    pub fn load(
        manifest: &Manifest,
        sim: &SimulationConfig,
        plugin_path: &Path,
        shared: Arc<Mutex<HostShared>>,
        state_machine: Arc<Mutex<StateMachine>>,
        local_endpoints: HashSet<String>,
    ) -> Result<Self> {
        let lib = unsafe { Library::new(plugin_path) }
            .with_context(|| format!("failed to load plugin {}", plugin_path.display()))?;
        let api = load_api(&lib)?;

        ensure_api_version(sim, &api)?;

        let config_json = CString::new(serde_json::to_string(&sim.params)?)?;
        let mut host_ctx = Box::new(HostContext::new(
            manifest,
            sim,
            shared,
            state_machine,
            local_endpoints,
        ));
        let ctx = SimContext {
            user_data: (&mut *host_ctx) as *mut HostContext as *mut c_void,
            log: host_log,
            set_output: host_set_output,
            get_input: host_get_input,
            set_state: host_set_state,
        };

        let instance = (api.create)(ctx, config_json.as_ptr());
        if instance.is_null() {
            anyhow::bail!("plugin '{}' returned a null simulation instance", sim.name);
        }

        Ok(Self {
            _lib: lib,
            api,
            instance,
            _host_ctx: host_ctx,
            destroyed: false,
        })
    }

    pub fn pre_step(&mut self, dt_seconds: f64) {
        (self.api.pre_step)(self.instance, dt_seconds);
    }

    pub fn step(&mut self, dt_seconds: f64) {
        (self.api.step)(self.instance, dt_seconds);
    }

    pub fn post_step(&mut self, dt_seconds: f64) {
        (self.api.post_step)(self.instance, dt_seconds);
    }

    pub fn destroy(&mut self) {
        if !self.destroyed {
            (self.api.destroy)(self.instance);
            self.destroyed = true;
        }
    }
}

impl Drop for LoadedSim {
    fn drop(&mut self) {
        self.destroy();
    }
}

fn load_api(lib: &Library) -> Result<SimApi> {
    let api = unsafe {
        let get_api: Symbol<GetSimApiFn> = lib.get(b"simengine_get_api")?;
        get_api()
    };

    Ok(api)
}

fn ensure_api_version(sim: &SimulationConfig, api: &SimApi) -> Result<()> {
    if api.api_version == SIMENGINE_API_VERSION {
        return Ok(());
    }

    anyhow::bail!(
        "plugin '{}' uses API version {}, but simengine expects {}",
        sim.name,
        api.api_version,
        SIMENGINE_API_VERSION
    );
}
