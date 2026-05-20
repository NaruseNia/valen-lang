//! `valenc` — the Valen compiler CLI.
//!
//! Pipeline:
//!   1. parse `.vln` → AST (valen-parser)
//!   2. lower AST → HIR (valen-hir: resolve + typeck + coherence + exhaustive)
//!   3. emit HIR → JVM `.class` (valen-codegen)

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use valen_ast::FileId;
use valen_codegen::JvmVersion;

/// Compile-phase error (exit code 1). Distinguished from IO/CLI errors (exit code 2).
#[derive(Debug)]
struct CompileError(String);

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CompileError {}

/// Parse and validate the --target flag into a JvmVersion.
fn parse_jvm_version(s: &str) -> Result<JvmVersion, String> {
    match s {
        "21" => Ok(JvmVersion::Java21),
        "25" => Ok(JvmVersion::Java25),
        _ => Err(format!("invalid JVM target `{s}`: must be 21 or 25")),
    }
}

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
        #[arg(short = 'o', long = "output", default_value = "build/classes/valen")]
        output: PathBuf,

        /// Target JVM version (21 = baseline, 25 = opt-in).
        #[arg(long, default_value = "21", value_parser = parse_jvm_version)]
        target: JvmVersion,

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
        let parse_file_map = [(input.as_path(), &line_idx)];
        emit_diagnostics(&result.diagnostics, &parse_file_map);
        if result.diagnostics.has_errors() {
            had_parse_errors = true;
        }
        all_items.extend(result.items);
        line_indexes.push(line_idx);
    }
    if had_parse_errors {
        return Err(CompileError("parse errors".into()).into());
    }

    let file_map: Vec<(&std::path::Path, &LineIndex)> = inputs
        .iter()
        .map(|p| p.as_path())
        .zip(line_indexes.iter())
        .collect();

    // --- Resolve (merged) ---
    let resolve_result = valen_hir::resolve::resolve_with_classpath(&all_items, classpath);
    emit_diagnostics(&resolve_result.diagnostics, &file_map);
    if resolve_result.diagnostics.has_errors() {
        return Err(CompileError("resolve errors".into()).into());
    }
    let hir = resolve_result.hir;

    // --- Type check (merged) ---
    let tc = valen_hir::ty::type_check(&hir, &all_items);
    emit_diagnostics(&tc.diagnostics, &file_map);
    if tc.diagnostics.has_errors() {
        return Err(CompileError("type errors".into()).into());
    }

    // --- Coherence ---
    let import_names: Vec<smol_str::SmolStr> = hir.imports.keys().cloned().collect();
    let coherence_result = valen_hir::coherence::check_coherence(&hir, &import_names);
    emit_diagnostics(&coherence_result.diagnostics, &file_map);
    if coherence_result.diagnostics.has_errors() {
        return Err(CompileError("coherence errors".into()).into());
    }

    // --- Exhaustiveness ---
    let exhaustiveness_result = valen_hir::exhaustive::check_exhaustiveness(&hir, &all_items);
    emit_diagnostics(&exhaustiveness_result.diagnostics, &file_map);
    if exhaustiveness_result.diagnostics.has_errors() {
        return Err(CompileError("exhaustiveness errors".into()).into());
    }

    Ok(FrontendResult {
        hir,
        bodies: tc.bodies,
    })
}

/// Emit diagnostics using FileId from each span to select the correct file path and line index.
fn emit_diagnostics(
    diags: &valen_diagnostics::Diagnostics,
    file_map: &[(&std::path::Path, &LineIndex)],
) {
    for diag in diags.iter() {
        let fid = diag.primary.file_id.0 as usize;
        let (path, line_idx): (&std::path::Path, &LineIndex) = if fid < file_map.len() {
            file_map[fid]
        } else {
            static UNKNOWN: std::sync::LazyLock<(PathBuf, LineIndex)> =
                std::sync::LazyLock::new(|| (PathBuf::from("<unknown>"), LineIndex::new("")));
            (&UNKNOWN.0, &UNKNOWN.1)
        };
        let (line, col) = line_idx.line_col(diag.primary.start);
        let severity = match diag.severity {
            valen_diagnostics::Severity::Error => "error",
            valen_diagnostics::Severity::Warning => "warning",
            valen_diagnostics::Severity::Hint => "hint",
        };
        eprintln!(
            "{}:{}:{}: {}: V{:04}: {}",
            path.display(),
            line,
            col,
            severity,
            diag.code.0,
            diag.message,
        );
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let result = match cli.command {
        Command::Compile {
            inputs,
            output,
            target,
            classpath,
        } => compile(&inputs, &output, target, &classpath),
        Command::Check {
            inputs, classpath, ..
        } => check(&inputs, &classpath),
        Command::EmitAnnotations { out } => emit_annotations(&out),
        Command::Version => {
            println!("valenc {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        let code = if e.downcast_ref::<CompileError>().is_some() {
            1
        } else {
            2
        };
        std::process::exit(code);
    }
}

fn compile(
    inputs: &[PathBuf],
    out_dir: &PathBuf,
    target: JvmVersion,
    classpath: &[PathBuf],
) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir)?;

    let frontend = run_pipeline_with_classpath(inputs, classpath)?;

    // Validate entry point: `fn main()` must exist for compiled executables.
    let entry_diags = valen_hir::ty::validate_entry_point(&frontend.hir);
    if entry_diags.has_errors() {
        // Build line indexes for error reporting
        let line_indexes: Vec<_> = inputs
            .iter()
            .map(|p| {
                let src = std::fs::read_to_string(p).unwrap_or_default();
                LineIndex::new(&src)
            })
            .collect();
        let file_map: Vec<(&std::path::Path, &LineIndex)> = inputs
            .iter()
            .map(|p| p.as_path())
            .zip(line_indexes.iter())
            .collect();
        emit_diagnostics(&entry_diags, &file_map);
        anyhow::bail!("entry point errors");
    }

    let outputs = valen_codegen::compile_hir(&frontend.hir, &frontend.bodies, target)?;
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
