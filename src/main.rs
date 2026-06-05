use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "openpart", version = "0.1.1", about = "OpenPart disk manager")]
struct Root {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Subcommand, Debug)]
enum RootCommand {
    Gui,
    Cli {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Service,
    Tests,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if !args.is_empty() {
        if let Some(exe_path) = args.get(0) {
            let exe_name = Path::new(exe_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if exe_name.contains("cli") {
                return openpart_cli::run_from(std::env::args()).context("openpart cli failed");
            }
        }
    }

    let root = Root::parse();
    match root.command {
        RootCommand::Gui => launch_gui(),
        RootCommand::Cli { args } => run_cli(args),
        RootCommand::Service => openpart_service::run(),
        RootCommand::Tests => run_tests(),
    }
}

fn run_cli(args: Vec<String>) -> Result<()> {
    let mut argv = vec!["openpart-cli".to_string()];
    argv.extend(args);
    openpart_cli::run_from(argv).context("openpart cli failed")
}

fn launch_gui() -> Result<()> {
    // Always run the GUI crate via cargo so config/UI changes are picked up.
    let manifest = Path::new("src-tauri").join("Cargo.toml");
    let manifest = manifest
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("src-tauri/Cargo.toml"))
        .to_string_lossy()
        .to_string();

    let status = Command::new("cargo")
        .args(["run", "--manifest-path", &manifest])
        .status()
        .context("failed to spawn cargo run for GUI crate")?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("Failed to run GUI crate via cargo run --manifest-path src-tauri/Cargo.toml")
    }
}

fn run_tests() -> Result<()> {
    let status = Command::new("cargo").arg("test").status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("cargo test failed")
    }
}
