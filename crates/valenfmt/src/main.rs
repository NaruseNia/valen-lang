//! `valenfmt` — Valen source formatter CLI.

use std::io::Read as _;
use std::path::Path;

use anyhow::{Context, Result};
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut needs_formatting = false;

    for path in &cli.inputs {
        if path == Path::new("-") {
            let mut source = String::new();
            std::io::stdin()
                .read_to_string(&mut source)
                .context("reading stdin")?;
            match valenfmt::format_source(&source) {
                Some(formatted) => print!("{formatted}"),
                None => {
                    eprintln!("valenfmt: <stdin>: parse errors, skipping");
                    print!("{source}");
                }
            }
        } else {
            let source = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            match valenfmt::format_source(&source) {
                Some(formatted) => {
                    if cli.check {
                        if source != formatted {
                            eprintln!("would reformat {}", path.display());
                            needs_formatting = true;
                        }
                    } else if source != formatted {
                        std::fs::write(path, &formatted)
                            .with_context(|| format!("writing {}", path.display()))?;
                    }
                }
                None => {
                    eprintln!("valenfmt: {}: parse errors, skipping", path.display());
                }
            }
        }
    }

    if cli.check && needs_formatting {
        std::process::exit(1);
    }

    Ok(())
}
