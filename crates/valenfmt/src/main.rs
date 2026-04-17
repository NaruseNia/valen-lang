//! `valenfmt` — Valen source formatter.
//!
//! MVP: brace style, indent, trailing `;` normalization.
//! Phase 1.5: richer rules, IDE integration.

use clap::Parser;

#[derive(Parser)]
#[command(name = "valenfmt", version, about = "Valen source formatter")]
struct Cli {
    /// Files to format. Use `-` to read from stdin.
    #[arg(required = true)]
    inputs: Vec<std::path::PathBuf>,

    /// Check mode: do not modify files, exit non-zero if changes would be made.
    #[arg(long)]
    check: bool,
}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    todo!("parse each input, pretty-print, diff vs original, either write or check")
}
