use crate::runtime::{check_manifest, run_manifest};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Check { manifest } => {
            check_manifest(manifest)?;
            println!("manifest ok");
        }
        Command::Run { manifest } => run_manifest(manifest)?,
    }

    Ok(())
}
