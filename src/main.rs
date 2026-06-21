use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use libloading::{Library, Symbol};
use simengine::core::{load_manifest, validate_manifest, LinkConfig, Manifest, SimulationConfig};
use simengine::network::{self, NetworkMessage};
use simengine::plugin_api::{GetSimApiFn, SimApi, SimContext, SimLogLevel, SIMENGINE_API_VERSION};
use std::{
    collections::{HashMap, HashSet},
    ffi::{c_char, c_void, CStr, CString},
    path::PathBuf,
    ptr,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(name = "simengine")]
#[command(about = "Headless simulation runtime for plugin-based simulations")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run { manifest: PathBuf },
    Check { manifest: PathBuf },
}

struct LoadedSim {
    _lib: Library,
    api: SimApi,
    instance: *mut c_void,
    _host_ctx: Box<HostContext>,
}

#[derive(Default)]
struct HostShared {
    /// Endpoint/input-or-output-name -> latest serialized value.
    /// Example key: "127.0.0.1:7002/cpu_usage".
    values: HashMap<String, Vec<u8>>,
}

struct HostContext {
    sim_name: String,
    endpoint: String,
    links: Vec<LinkConfig>,
    local_endpoints: HashSet<String>,
    shared: Arc<Mutex<HostShared>>,
}

extern "C" fn host_log(user_data: *mut c_void, level: SimLogLevel, message: *const c_char) {
    let sim_name = host_context(user_data)
        .map(|ctx| ctx.sim_name.as_str())
        .unwrap_or("unknown");
    let message = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    println!("[{level:?}] [{sim_name}] {message}");
}

extern "C" fn host_set_output(
    user_data: *mut c_void,
    name: *const c_char,
    payload: *const u8,
    payload_len: usize,
) {
    let Some(ctx) = host_context(user_data) else { return; };

    let output = unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned();
    let payload = unsafe { std::slice::from_raw_parts(payload, payload_len) }.to_vec();
    let output_key = value_key(&ctx.endpoint, &output);

    if let Ok(mut shared) = ctx.shared.lock() {
        shared.values.insert(output_key.clone(), payload.clone());
    }

    println!("[runner] set_output {output_key} = {} bytes", payload.len());

    for link in ctx
        .links
        .iter()
        .filter(|link| link.from.endpoint == ctx.endpoint && link.from.output == output)
    {
        let target_key = value_key(&link.to.endpoint, &link.to.input);

        if ctx.local_endpoints.contains(&link.to.endpoint) {
            if let Ok(mut shared) = ctx.shared.lock() {
                shared.values.insert(target_key.clone(), payload.clone());
            }
            println!("[runner] local delivery {output_key} -> {target_key}");
        } else {
            let message = NetworkMessage {
                input: link.to.input.clone(),
                payload: payload.clone(),
            };

            match network::send(&link.to.endpoint, &message) {
                Ok(()) => println!("[network] sent {output_key} -> {target_key}"),
                Err(err) => eprintln!("[network] failed to send {output_key} -> {target_key}: {err}"),
            }
        }
    }
}

extern "C" fn host_get_input(
    user_data: *mut c_void,
    name: *const c_char,
    out_payload: *mut u8,
    out_payload_len: usize,
) -> usize {
    let Some(ctx) = host_context(user_data) else { return 0; };

    let input = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let input_key = value_key(&ctx.endpoint, input.as_ref());

    let Ok(shared) = ctx.shared.lock() else { return 0; };
    let Some(value) = shared.values.get(&input_key) else {
        println!("[runner] get_input {input_key} -> no value yet");
        return 0;
    };

    let bytes_to_copy = value.len().min(out_payload_len);
    unsafe {
        ptr::copy_nonoverlapping(value.as_ptr(), out_payload, bytes_to_copy);
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Check { manifest } => {
            let manifest = load_manifest(manifest)?;
            validate_manifest(&manifest)?;
            println!("manifest ok");
        }
        Command::Run { manifest } => run(manifest)?,
    }

    Ok(())
}

fn run(path: PathBuf) -> Result<()> {
    let manifest = load_manifest(&path)?;
    validate_manifest(&manifest)?;

    let base_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let shared = Arc::new(Mutex::new(HostShared::default()));
    let local_endpoints: HashSet<String> = manifest
        .simulations
        .iter()
        .map(|sim| sim.endpoint.clone())
        .collect();

    let _listeners = start_network_listeners(&manifest, Arc::clone(&shared))?;
    let mut sims = Vec::new();

    for sim in &manifest.simulations {
        let plugin_path = base_dir.join(&sim.plugin);
        let lib = unsafe { Library::new(&plugin_path) }
            .with_context(|| format!("failed to load plugin {}", plugin_path.display()))?;

        let api = unsafe {
            let get_api: Symbol<GetSimApiFn> = lib.get(b"simengine_get_api")?;
            get_api()
        };

        if api.api_version != SIMENGINE_API_VERSION {
            anyhow::bail!(
                "plugin '{}' uses API version {}, but simengine expects {}",
                sim.name,
                api.api_version,
                SIMENGINE_API_VERSION
            );
        }

        println!(
            "[runner] loading simulation '{}' at {} from {}",
            sim.name,
            sim.endpoint,
            plugin_path.display()
        );

        let config_json = CString::new(serde_json::to_string(&sim.params)?)?;
        let mut host_ctx = Box::new(make_host_context(
            &manifest,
            sim,
            Arc::clone(&shared),
            local_endpoints.clone(),
        ));
        let ctx = SimContext {
            user_data: (&mut *host_ctx) as *mut HostContext as *mut c_void,
            log: host_log,
            set_output: host_set_output,
            get_input: host_get_input,
        };

        let instance = (api.create)(ctx, config_json.as_ptr());
        sims.push(LoadedSim { _lib: lib, api, instance, _host_ctx: host_ctx });
    }

    let fps = manifest.framework.fps.max(1);
    let dt = 1.0 / fps as f64;
    let frame_duration = Duration::from_secs_f64(dt);
    let max_frames = manifest.framework.max_frames;

    match max_frames {
        Some(max_frames) => println!(
            "[runner] starting run: fps={} dt={:.6}s max_frames={}",
            fps, dt, max_frames
        ),
        None => println!(
            "[runner] starting run: fps={} dt={:.6}s max_frames=unbounded",
            fps, dt
        ),
    }

    let run_started = Instant::now();
    let mut frame: u64 = 0;

    loop {
        if let Some(max_frames) = max_frames {
            if frame >= max_frames {
                break;
            }
        }

        let frame_started = Instant::now();
        println!("[runner] frame {frame} begin");

        for sim in &mut sims { (sim.api.pre_step)(sim.instance, dt); }
        for sim in &mut sims { (sim.api.step)(sim.instance, dt); }
        for sim in &mut sims { (sim.api.post_step)(sim.instance, dt); }

        let elapsed = frame_started.elapsed();
        if elapsed < frame_duration {
            thread::sleep(frame_duration - elapsed);
        }

        let actual_frame_time = frame_started.elapsed().as_secs_f64();
        let actual_fps = if actual_frame_time > 0.0 { 1.0 / actual_frame_time } else { 0.0 };
        println!(
            "[runner] frame {frame} end actual_dt={actual_frame_time:.6}s actual_fps={actual_fps:.2}"
        );

        frame += 1;
    }

    let total = run_started.elapsed().as_secs_f64();
    println!(
        "[runner] finished: frames={} total_time={total:.3}s average_fps={:.2}",
        frame,
        frame as f64 / total.max(f64::EPSILON)
    );

    for sim in &mut sims {
        (sim.api.destroy)(sim.instance);
    }

    Ok(())
}

fn make_host_context(
    manifest: &Manifest,
    sim: &SimulationConfig,
    shared: Arc<Mutex<HostShared>>,
    local_endpoints: HashSet<String>,
) -> HostContext {
    HostContext {
        sim_name: sim.name.clone(),
        endpoint: sim.endpoint.clone(),
        links: manifest.links.clone(),
        local_endpoints,
        shared,
    }
}

fn start_network_listeners(
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

fn value_key(endpoint: &str, variable: &str) -> String {
    format!("{endpoint}/{variable}")
}
