//! `valenc` — the Valen compiler CLI.
//!
//! Pipeline:
//!   1. parse `.vln` → AST (valen-parser)
//!   2. lower AST → HIR (valen-hir: resolve + typeck + coherence + exhaustive)
//!   3. emit HIR → JVM `.class` (valen-codegen)

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "valenc", version, about = "Valen language compiler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile one or more `.vln` files to `.class`.
    Build {
        /// Source files or directories.
        #[arg(required = true)]
        inputs: Vec<std::path::PathBuf>,

        /// Output directory for `.class` files.
        #[arg(short, long, default_value = "build/classes/valen")]
        out: std::path::PathBuf,

        /// Target JVM version (21 = baseline, 25 = opt-in).
        #[arg(long, default_value = "21")]
        target: String,
    },
    /// Check without emitting bytecode.
    Check {
        #[arg(required = true)]
        inputs: Vec<std::path::PathBuf>,
    },
    /// Print version info.
    Version,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Build { .. } => todo!("end-to-end compile: parse → HIR → bytecode"),
        Command::Check { .. } => todo!("parse + HIR only, no emit"),
        Command::Version => {
            println!("valenc {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
