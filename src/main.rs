use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use libloading::{Library, Symbol};
use simengine::core::{load_manifest, validate_manifest, Manifest, SimulationConfig};
use simengine::plugin_api::{GetSimApiFn, SimApi, SimContext, SimLogLevel, SIMENGINE_API_VERSION};
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr, CString},
    path::PathBuf,
    ptr,
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
    /// Fully qualified variable name -> latest serialized value.
    /// Example key: "basic-sim.counter".
    values: HashMap<String, Vec<u8>>,
}

struct HostContext {
    sim_name: String,
    input_sources: HashMap<String, String>,
    shared: *mut HostShared,
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

    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let key = format!("{}.{}", ctx.sim_name, name);
    let payload = unsafe { std::slice::from_raw_parts(payload, payload_len) };
    let shared = unsafe { &mut *ctx.shared };

    shared.values.insert(key.clone(), payload.to_vec());
    println!("[runner] set_output {key} = {} bytes", payload.len());
}

extern "C" fn host_get_input(
    user_data: *mut c_void,
    name: *const c_char,
    out_payload: *mut u8,
    out_payload_len: usize,
) -> usize {
    let Some(ctx) = host_context(user_data) else { return 0; };

    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let Some(source_key) = ctx.input_sources.get(name.as_ref()) else {
        println!("[runner] get_input {}.{} -> no source configured", ctx.sim_name, name);
        return 0;
    };

    let shared = unsafe { &mut *ctx.shared };
    let Some(value) = shared.values.get(source_key) else {
        println!("[runner] get_input {}.{} <- {source_key} -> no value yet", ctx.sim_name, name);
        return 0;
    };

    let bytes_to_copy = value.len().min(out_payload_len);
    unsafe {
        ptr::copy_nonoverlapping(value.as_ptr(), out_payload, bytes_to_copy);
    }

    println!(
        "[runner] get_input {}.{} <- {source_key} = {bytes_to_copy} bytes",
        ctx.sim_name,
        name
    );

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
    let mut shared = Box::new(HostShared::default());
    let shared_ptr: *mut HostShared = &mut *shared;
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

        println!("[runner] loading simulation '{}' from {}", sim.name, plugin_path.display());

        let config_json = CString::new(serde_json::to_string(&sim.params)?)?;
        let mut host_ctx = Box::new(make_host_context(&manifest, sim, shared_ptr));
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

    drop(sims);
    drop(shared);

    Ok(())
}

fn make_host_context(
    manifest: &Manifest,
    sim: &SimulationConfig,
    shared: *mut HostShared,
) -> HostContext {
    let mut input_sources = HashMap::new();

    for input in &sim.inputs {
        let matches: Vec<String> = manifest
            .simulations
            .iter()
            .flat_map(|producer_sim| {
                producer_sim
                    .outputs
                    .iter()
                    .map(move |output| (producer_sim, output))
            })
            .filter(|(_, output)| output.name == input.name && output.ty == input.ty)
            .map(|(producer_sim, output)| format!("{}.{}", producer_sim.name, output.name))
            .collect();

        // validate_manifest guarantees exactly one match for every declared input.
        let source = matches
            .first()
            .expect("validated input should have exactly one matching output")
            .clone();

        input_sources.insert(input.name.clone(), source);
    }

    HostContext {
        sim_name: sim.name.clone(),
        input_sources,
        shared,
    }
}
