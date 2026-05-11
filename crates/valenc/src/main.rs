//! `valenc` — the Valen compiler CLI.
//!
//! Pipeline:
//!   1. parse `.vln` → AST (valen-parser)
//!   2. lower AST → HIR (valen-hir: resolve + typeck + coherence + exhaustive)
//!   3. emit HIR → JVM `.class` (valen-codegen)
//!
//! Phase 0 PoC: steps 1 + 3 only (no HIR lowering), class declarations only.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use valen_ast::{FileId, Item};
use valen_codegen::class_emit;

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
        inputs: Vec<PathBuf>,

        /// Output directory for `.class` files.
        #[arg(short, long, default_value = "build/classes/valen")]
        out: PathBuf,

        /// Target JVM version (21 = baseline, 25 = opt-in).
        #[arg(long, default_value = "21")]
        target: String,
    },
    /// Check without emitting bytecode.
    Check {
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
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
        Command::Build { inputs, out, .. } => compile(&inputs, &out),
        Command::Check { .. } => todo!("parse + HIR only, no emit"),
        Command::Version => {
            println!("valenc {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn compile(inputs: &[PathBuf], out_dir: &PathBuf) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir)?;

    for input in inputs {
        let source = std::fs::read_to_string(input)?;
        let file_id = FileId(0);
        let result = valen_parser::parse(&source, file_id);

        if result.diagnostics.has_errors() {
            for diag in result.diagnostics.iter() {
                eprintln!(
                    "{}: V{:04} {}..{}: {}",
                    input.display(),
                    diag.code.0,
                    diag.primary.start,
                    diag.primary.end,
                    diag.message,
                );
            }
            anyhow::bail!("parse errors in {}", input.display());
        }

        for item in &result.items {
            if let Item::Class(class_decl) = item {
                let output = class_emit::emit_class(&class_decl.name)?;
                let class_path = out_dir.join(format!("{}.class", output.internal_name));
                std::fs::write(&class_path, &output.bytes)?;
                println!("  {} -> {}", input.display(), class_path.display());
            }
        }
    }

    Ok(())
}
