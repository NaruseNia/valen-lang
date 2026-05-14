//! `valenc` — the Valen compiler CLI.
//!
//! Pipeline:
//!   1. parse `.vln` → AST (valen-parser)
//!   2. lower AST → HIR (valen-hir: resolve + typeck + coherence + exhaustive)
//!   3. emit HIR → JVM `.class` (valen-codegen)

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
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
    Compile {
        /// Source files or directories.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,

        /// Output directory for `.class` files.
        #[arg(short, long, default_value = "build/classes/valen")]
        out: PathBuf,

        /// Target JVM version (21 = baseline, 25 = opt-in).
        #[arg(long, default_value = "21")]
        target: String,

        /// Classpath entries (directories or JARs) for Java import resolution.
        #[arg(long)]
        classpath: Vec<PathBuf>,
    },
    /// Check without emitting bytecode.
    Check {
        #[arg(required = true)]
        inputs: Vec<PathBuf>,

        /// Classpath entries (directories or JARs) for Java import resolution.
        #[arg(long)]
        classpath: Vec<PathBuf>,
    },
    /// Emit `valen-annotations.jar` (contains `@valen.Closed`).
    EmitAnnotations {
        /// Output directory.
        #[arg(short, long, default_value = "build")]
        out: PathBuf,
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
    bodies: indexmap::IndexMap<valen_hir::DefId, valen_hir::TypedBody>,
}

fn run_pipeline_with_classpath(
    inputs: &[PathBuf],
    classpath: &[PathBuf],
) -> anyhow::Result<FrontendResult> {
    let mut all_items: Vec<valen_ast::Item> = Vec::new();
    let mut line_indexes: Vec<LineIndex> = Vec::new();
    let mut had_parse_errors = false;

    for (idx, input) in inputs.iter().enumerate() {
        let source = std::fs::read_to_string(input)?;
        let file_id = FileId(idx as u32);
        let line_idx = LineIndex::new(&source);
        let result = valen_parser::parse(&source, file_id);
        emit_diagnostics(&result.diagnostics, input, &line_idx);
        if result.diagnostics.has_errors() {
            had_parse_errors = true;
        }
        all_items.extend(result.items);
        line_indexes.push(line_idx);
    }
    if had_parse_errors {
        anyhow::bail!("parse errors");
    }

    let first_path = inputs
        .first()
        .map(|p| p.as_path())
        .unwrap_or(std::path::Path::new("<unknown>"));
    let first_line_idx = line_indexes.first();

    // --- Resolve (merged) ---
    let resolve_result = valen_hir::resolve::resolve_with_classpath(&all_items, classpath);
    if let Some(li) = first_line_idx {
        emit_diagnostics(&resolve_result.diagnostics, first_path, li);
    }
    if resolve_result.diagnostics.has_errors() {
        anyhow::bail!("resolve errors");
    }
    let hir = resolve_result.hir;

    // --- Type check (merged) ---
    let tc = valen_hir::ty::type_check(&hir, &all_items);
    if let Some(li) = first_line_idx {
        emit_diagnostics(&tc.diagnostics, first_path, li);
    }
    if tc.diagnostics.has_errors() {
        anyhow::bail!("type errors");
    }

    // --- Coherence ---
    let coherence_result = valen_hir::coherence::check_coherence(&hir, &[]);
    if let Some(li) = first_line_idx {
        emit_diagnostics(&coherence_result.diagnostics, first_path, li);
    }
    if coherence_result.diagnostics.has_errors() {
        anyhow::bail!("coherence errors");
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
        Command::Compile {
            inputs,
            out,
            classpath,
            ..
        } => compile(&inputs, &out, &classpath),
        Command::Check {
            inputs, classpath, ..
        } => check(&inputs, &classpath),
        Command::EmitAnnotations { out } => emit_annotations(&out),
        Command::Version => {
            println!("valenc {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn compile(inputs: &[PathBuf], out_dir: &PathBuf, classpath: &[PathBuf]) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir)?;

    let frontend = run_pipeline_with_classpath(inputs, classpath)?;

    let outputs = valen_codegen::compile_hir(&frontend.hir, &frontend.bodies)?;
    for output in &outputs {
        let parts: Vec<&str> = output.internal_name.split('/').collect();
        if parts.len() > 1 {
            let dir = out_dir.join(parts[..parts.len() - 1].join("/"));
            std::fs::create_dir_all(&dir)?;
        }
        let class_path = out_dir.join(format!("{}.class", output.internal_name));
        std::fs::write(&class_path, &output.bytes)?;
        println!("  {} -> {}", input_display(inputs), class_path.display());
    }

    Ok(())
}

fn emit_annotations(out_dir: &Path) -> anyhow::Result<()> {
    let output = valen_codegen::generate_closed_annotation()?;
    let class_path = out_dir.join(format!("{}.class", output.internal_name));
    if let Some(parent) = class_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&class_path, &output.bytes)?;
    println!("  {}", class_path.display());
    Ok(())
}

fn check(inputs: &[PathBuf], classpath: &[PathBuf]) -> anyhow::Result<()> {
    run_pipeline_with_classpath(inputs, classpath)?;
    for input in inputs {
        println!("  {} OK", input.display());
    }
    Ok(())
}

fn input_display(inputs: &[PathBuf]) -> String {
    if inputs.len() == 1 {
        inputs[0].display().to_string()
    } else {
        format!("{} files", inputs.len())
    }
}
