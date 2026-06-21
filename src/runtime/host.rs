use crate::{
    core::{LinkConfig, Manifest, SimulationConfig},
    network::{self, NetworkMessage},
    plugin_api::SimLogLevel,
};
use anyhow::{Context, Result};
use std::{
    collections::{HashMap, HashSet},
    ffi::{CStr, c_char, c_void},
    ptr,
    sync::{Arc, Mutex},
    thread,
};

#[derive(Default)]
pub struct HostShared {
    /// Endpoint/input-or-output-name -> latest serialized value.
    /// Example key: "127.0.0.1:7002/cpu_usage".
    values: HashMap<String, Vec<u8>>,
}

pub struct HostContext {
    sim_name: String,
    endpoint: String,
    links: Vec<LinkConfig>,
    local_endpoints: HashSet<String>,
    shared: Arc<Mutex<HostShared>>,
}

impl HostContext {
    pub fn new(
        manifest: &Manifest,
        sim: &SimulationConfig,
        shared: Arc<Mutex<HostShared>>,
        local_endpoints: HashSet<String>,
    ) -> Self {
        Self {
            sim_name: sim.name.clone(),
            endpoint: sim.endpoint.clone(),
            links: manifest.links.clone(),
            local_endpoints,
            shared,
        }
    }
}

pub extern "C" fn host_log(user_data: *mut c_void, level: SimLogLevel, message: *const c_char) {
    let sim_name = host_context(user_data)
        .map(|ctx| ctx.sim_name.as_str())
        .unwrap_or("unknown");
    let message = c_string_lossy(message).unwrap_or_else(|| "<null message>".to_string());

    println!("[{level:?}] [{sim_name}] {message}");
}

pub extern "C" fn host_set_output(
    user_data: *mut c_void,
    name: *const c_char,
    payload: *const u8,
    payload_len: usize,
) {
    let Some(ctx) = host_context(user_data) else {
        return;
    };
    let Some(output) = c_string_lossy(name) else {
        return;
    };
    let Some(payload) = copy_payload(payload, payload_len) else {
        return;
    };

    store_output(ctx, &output, payload);
}

pub extern "C" fn host_get_input(
    user_data: *mut c_void,
    name: *const c_char,
    out_payload: *mut u8,
    out_payload_len: usize,
) -> usize {
    let Some(ctx) = host_context(user_data) else {
        return 0;
    };
    let Some(input) = c_string_lossy(name) else {
        return 0;
    };
    if out_payload.is_null() && out_payload_len > 0 {
        return 0;
    }

    read_input(ctx, &input, out_payload, out_payload_len)
}

pub fn start_network_listeners(
    manifest: &Manifest,
    shared: Arc<Mutex<HostShared>>,
) -> Result<Vec<thread::JoinHandle<()>>> {
    let mut handles = Vec::new();

    for endpoint in manifest.simulations.iter().map(|sim| sim.endpoint.clone()) {
        let shared = Arc::clone(&shared);
        let listener_endpoint = endpoint.clone();

        let handle = network::start_listener(endpoint.clone(), move |message| {
            let key = value_key(&listener_endpoint, &message.input);
            match shared.lock() {
                Ok(mut shared) => {
                    shared.values.insert(key.clone(), message.payload);
                    println!("[network] received {key}");
                }
                Err(err) => eprintln!("[network] failed to store received value: {err}"),
            }
        })
        .with_context(|| format!("failed to bind network listener on {endpoint}"))?;

        handles.push(handle);
    }

    Ok(handles)
}

fn store_output(ctx: &HostContext, output: &str, payload: Vec<u8>) {
    let output_key = value_key(&ctx.endpoint, output);

    if let Ok(mut shared) = ctx.shared.lock() {
        shared.values.insert(output_key.clone(), payload.clone());
    }

    println!("[runner] set_output {output_key} = {} bytes", payload.len());

    for link in ctx
        .links
        .iter()
        .filter(|link| link.from.endpoint == ctx.endpoint && link.from.output == output)
    {
        deliver_linked_output(ctx, link, &output_key, &payload);
    }
}

fn deliver_linked_output(ctx: &HostContext, link: &LinkConfig, output_key: &str, payload: &[u8]) {
    let target_key = value_key(&link.to.endpoint, &link.to.input);

    if ctx.local_endpoints.contains(&link.to.endpoint) {
        if let Ok(mut shared) = ctx.shared.lock() {
            shared.values.insert(target_key.clone(), payload.to_vec());
        }
        println!("[runner] local delivery {output_key} -> {target_key}");
        return;
    }

    let message = NetworkMessage {
        input: link.to.input.clone(),
        payload: payload.to_vec(),
    };

    match network::send(&link.to.endpoint, &message) {
        Ok(()) => println!("[network] sent {output_key} -> {target_key}"),
        Err(err) => eprintln!("[network] failed to send {output_key} -> {target_key}: {err}"),
    }
}

fn read_input(
    ctx: &HostContext,
    input: &str,
    out_payload: *mut u8,
    out_payload_len: usize,
) -> usize {
    let input_key = value_key(&ctx.endpoint, input);

    let Ok(shared) = ctx.shared.lock() else {
        return 0;
    };
    let Some(value) = shared.values.get(&input_key) else {
        println!("[runner] get_input {input_key} -> no value yet");
        return 0;
    };

    let bytes_to_copy = value.len().min(out_payload_len);
    if bytes_to_copy > 0 {
        unsafe {
            ptr::copy_nonoverlapping(value.as_ptr(), out_payload, bytes_to_copy);
        }
    }

    println!("[runner] get_input {input_key} = {bytes_to_copy} bytes");
    bytes_to_copy
}

fn host_context<'a>(user_data: *mut c_void) -> Option<&'a mut HostContext> {
    if user_data.is_null() {
        None
    } else {
        Some(unsafe { &mut *(user_data as *mut HostContext) })
    }
}

fn c_string_lossy(value: *const c_char) -> Option<String> {
    if value.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(value) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

fn copy_payload(payload: *const u8, payload_len: usize) -> Option<Vec<u8>> {
    if payload.is_null() && payload_len > 0 {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(payload, payload_len) }.to_vec())
    }
}

fn value_key(endpoint: &str, variable: &str) -> String {
    format!("{endpoint}/{variable}")
}
