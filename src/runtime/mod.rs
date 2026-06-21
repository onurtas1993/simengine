mod host;
mod plugin;

use crate::core::{Manifest, load_manifest, validate_manifest};
use anyhow::{Context, Result};
use host::{HostShared, start_network_listeners};
use plugin::LoadedSim;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub fn check_manifest(path: impl AsRef<Path>) -> Result<()> {
    let manifest = load_manifest(path)?;
    validate_manifest(&manifest)?;
    Ok(())
}

pub fn run_manifest(path: PathBuf) -> Result<()> {
    let manifest = load_manifest(&path)?;
    validate_manifest(&manifest)?;
    run(manifest, manifest_base_dir(&path))
}

fn run(manifest: Manifest, base_dir: &Path) -> Result<()> {
    let shared = Arc::new(Mutex::new(HostShared::default()));
    let local_endpoints = local_endpoints(&manifest);

    let _listeners = start_network_listeners(&manifest, Arc::clone(&shared))?;
    let mut sims = load_simulations(&manifest, base_dir, Arc::clone(&shared), local_endpoints)?;

    run_frames(&manifest, &mut sims);
    destroy_simulations(&mut sims);

    Ok(())
}

fn manifest_base_dir(path: &Path) -> &Path {
    path.parent().unwrap_or_else(|| Path::new("."))
}

fn local_endpoints(manifest: &Manifest) -> HashSet<String> {
    manifest
        .simulations
        .iter()
        .map(|sim| sim.endpoint.clone())
        .collect()
}

fn load_simulations(
    manifest: &Manifest,
    base_dir: &Path,
    shared: Arc<Mutex<HostShared>>,
    local_endpoints: HashSet<String>,
) -> Result<Vec<LoadedSim>> {
    let mut sims = Vec::new();

    for sim in &manifest.simulations {
        let plugin_path = base_dir.join(&sim.plugin);
        println!(
            "[runner] loading simulation '{}' at {} from {}",
            sim.name,
            sim.endpoint,
            plugin_path.display()
        );

        let loaded = LoadedSim::load(
            manifest,
            sim,
            &plugin_path,
            Arc::clone(&shared),
            local_endpoints.clone(),
        )
        .with_context(|| format!("failed to load simulation '{}'", sim.name))?;

        sims.push(loaded);
    }

    Ok(sims)
}

fn run_frames(manifest: &Manifest, sims: &mut [LoadedSim]) {
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

    while max_frames.is_none_or(|max_frames| frame < max_frames) {
        let frame_started = Instant::now();
        println!("[runner] frame {frame} begin");

        for sim in sims.iter_mut() {
            sim.pre_step(dt);
        }
        for sim in sims.iter_mut() {
            sim.step(dt);
        }
        for sim in sims.iter_mut() {
            sim.post_step(dt);
        }

        sleep_until_next_frame(frame_started, frame_duration);
        log_frame_end(frame, frame_started);

        frame += 1;
    }

    log_run_end(frame, run_started);
}

fn sleep_until_next_frame(frame_started: Instant, frame_duration: Duration) {
    let elapsed = frame_started.elapsed();
    if elapsed < frame_duration {
        thread::sleep(frame_duration - elapsed);
    }
}

fn log_frame_end(frame: u64, frame_started: Instant) {
    let actual_frame_time = frame_started.elapsed().as_secs_f64();
    let actual_fps = if actual_frame_time > 0.0 {
        1.0 / actual_frame_time
    } else {
        0.0
    };

    println!(
        "[runner] frame {frame} end actual_dt={actual_frame_time:.6}s actual_fps={actual_fps:.2}"
    );
}

fn log_run_end(frame: u64, run_started: Instant) {
    let total = run_started.elapsed().as_secs_f64();
    println!(
        "[runner] finished: frames={} total_time={total:.3}s average_fps={:.2}",
        frame,
        frame as f64 / total.max(f64::EPSILON)
    );
}

fn destroy_simulations(sims: &mut [LoadedSim]) {
    for sim in sims {
        sim.destroy();
    }
}
