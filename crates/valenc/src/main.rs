//! `valenc` — the Valen compiler CLI.
//!
//! Pipeline:
//!   1. parse `.vln` → AST (valen-parser)
//!   2. lower AST → HIR (valen-hir: resolve + typeck + coherence + exhaustive)
//!   3. emit HIR → JVM `.class` (valen-codegen)

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use valen_ast::FileId;

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
        Command::Check { inputs } => check(&inputs),
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

        let resolve_result = valen_hir::resolve::resolve(&result.items);
        if resolve_result.diagnostics.has_errors() {
            for diag in resolve_result.diagnostics.iter() {
                eprintln!(
                    "{}: V{:04} {}..{}: {}",
                    input.display(),
                    diag.code.0,
                    diag.primary.start,
                    diag.primary.end,
                    diag.message,
                );
            }
            anyhow::bail!("resolve errors in {}", input.display());
        }
        let hir = &resolve_result.hir;

        let tc = valen_hir::ty::type_check(hir, &result.items);
        if tc.diagnostics.has_errors() {
            for diag in tc.diagnostics.iter() {
                eprintln!(
                    "{}: V{:04} {}..{}: {}",
                    input.display(),
                    diag.code.0,
                    diag.primary.start,
                    diag.primary.end,
                    diag.message,
                );
            }
            anyhow::bail!("type errors in {}", input.display());
        }

        let coherence_result = valen_hir::coherence::check_coherence(hir, &[]);
        if coherence_result.diagnostics.has_errors() {
            for diag in coherence_result.diagnostics.iter() {
                eprintln!(
                    "{}: V{:04} {}..{}: {}",
                    input.display(),
                    diag.code.0,
                    diag.primary.start,
                    diag.primary.end,
                    diag.message,
                );
            }
            anyhow::bail!("coherence errors in {}", input.display());
        }

        let outputs = valen_codegen::compile_hir(hir, &tc.bodies)?;
        for output in &outputs {
            let parts: Vec<&str> = output.internal_name.split('/').collect();
            if parts.len() > 1 {
                let dir = out_dir.join(parts[..parts.len() - 1].join("/"));
                std::fs::create_dir_all(&dir)?;
            }
            let class_path = out_dir.join(format!("{}.class", output.internal_name));
            std::fs::write(&class_path, &output.bytes)?;
            println!("  {} -> {}", input.display(), class_path.display());
        }
    }

    Ok(())
}

fn check(inputs: &[PathBuf]) -> anyhow::Result<()> {
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

        let resolve_result = valen_hir::resolve::resolve(&result.items);
        if resolve_result.diagnostics.has_errors() {
            for diag in resolve_result.diagnostics.iter() {
                eprintln!(
                    "{}: V{:04} {}..{}: {}",
                    input.display(),
                    diag.code.0,
                    diag.primary.start,
                    diag.primary.end,
                    diag.message,
                );
            }
            anyhow::bail!("resolve errors in {}", input.display());
        }

        let tc = valen_hir::ty::type_check(&resolve_result.hir, &result.items);
        if tc.diagnostics.has_errors() {
            for diag in tc.diagnostics.iter() {
                eprintln!(
                    "{}: V{:04} {}..{}: {}",
                    input.display(),
                    diag.code.0,
                    diag.primary.start,
                    diag.primary.end,
                    diag.message,
                );
            }
            anyhow::bail!("type errors in {}", input.display());
        }

        println!("  {} OK", input.display());
    }

    Ok(())
}
