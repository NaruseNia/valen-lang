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

// ---------------------------------------------------------------------------
// LineIndex — converts byte offsets to 1-based line:column
// ---------------------------------------------------------------------------

/// Pre-computed line offset table for converting byte offsets to line:column.
struct LineIndex {
    /// Byte offset of the start of each line (0-indexed line number).
    line_starts: Vec<u32>,
}

impl LineIndex {
    /// Build a line index from the full source text.
    fn new(source: &str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self { line_starts }
    }

    /// Convert a byte offset to a 1-based (line, column) pair.
    fn line_col(&self, offset: u32) -> (usize, usize) {
        let line_0 = match self.line_starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(next) => next.saturating_sub(1),
        };
        let col_0 = offset.saturating_sub(self.line_starts[line_0]) as usize;
        (line_0 + 1, col_0 + 1)
    }
}

// ---------------------------------------------------------------------------
// Shared frontend pipeline
// ---------------------------------------------------------------------------

/// Result of the shared frontend pipeline (parse → resolve → type_check → coherence).
struct FrontendResult {
    hir: valen_hir::Hir,
    bodies: indexmap::IndexMap<smol_str::SmolStr, valen_hir::TypedBody>,
}

/// Run parse → resolve → type_check → coherence and report all diagnostics.
///
/// Returns `Ok(FrontendResult)` if no errors, `Err` on the first phase that fails.
fn run_frontend_pipeline(
    source: &str,
    file_id: FileId,
    path: &std::path::Path,
) -> anyhow::Result<FrontendResult> {
    let line_idx = LineIndex::new(source);

    // --- Parse ---
    let result = valen_parser::parse(source, file_id);
    emit_diagnostics(&result.diagnostics, path, &line_idx);
    if result.diagnostics.has_errors() {
        anyhow::bail!("parse errors in {}", path.display());
    }

    // --- Resolve ---
    let resolve_result = valen_hir::resolve::resolve(&result.items);
    emit_diagnostics(&resolve_result.diagnostics, path, &line_idx);
    if resolve_result.diagnostics.has_errors() {
        anyhow::bail!("resolve errors in {}", path.display());
    }
    let hir = resolve_result.hir;

    // --- Type check ---
    let tc = valen_hir::ty::type_check(&hir, &result.items);
    emit_diagnostics(&tc.diagnostics, path, &line_idx);
    if tc.diagnostics.has_errors() {
        anyhow::bail!("type errors in {}", path.display());
    }

    // --- Coherence ---
    let coherence_result = valen_hir::coherence::check_coherence(&hir, &[]);
    emit_diagnostics(&coherence_result.diagnostics, path, &line_idx);
    if coherence_result.diagnostics.has_errors() {
        anyhow::bail!("coherence errors in {}", path.display());
    }

    Ok(FrontendResult {
        hir,
        bodies: tc.bodies,
    })
}

/// Emit diagnostics as `file:line:col: V0xxx: message`.
fn emit_diagnostics(
    diags: &valen_diagnostics::Diagnostics,
    path: &std::path::Path,
    line_idx: &LineIndex,
) {
    for diag in diags.iter() {
        let (line, col) = line_idx.line_col(diag.primary.start);
        eprintln!(
            "{}:{}:{}: V{:04}: {}",
            path.display(),
            line,
            col,
            diag.code.0,
            diag.message,
        );
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

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

    for (idx, input) in inputs.iter().enumerate() {
        let source = std::fs::read_to_string(input)?;
        let file_id = FileId(idx as u32);

        let frontend = run_frontend_pipeline(&source, file_id, input)?;

        let outputs = valen_codegen::compile_hir(&frontend.hir, &frontend.bodies)?;
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
    for (idx, input) in inputs.iter().enumerate() {
        let source = std::fs::read_to_string(input)?;
        let file_id = FileId(idx as u32);

        run_frontend_pipeline(&source, file_id, input)?;

        println!("  {} OK", input.display());
    }

    Ok(())
}
