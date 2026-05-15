//! `valenfmt` — Valen source formatter CLI.

use std::io::{Read as _, Write as _};
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
                Some(formatted) => {
                    if cli.check {
                        if source != formatted {
                            eprintln!("would reformat <stdin>");
                            needs_formatting = true;
                        }
                    } else {
                        print!("{formatted}");
                    }
                }
                None => {
                    eprintln!("valenfmt: <stdin>: parse errors, skipping");
                    if !cli.check {
                        print!("{source}");
                    }
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
                        atomic_write(path, &formatted)
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

/// Write `content` to `path` atomically via a temp file + fsync + rename.
fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir).context("creating temp file")?;
    tmp.write_all(content.as_bytes())
        .context("writing temp file")?;
    tmp.as_file().sync_all().context("fsyncing temp file")?;
    tmp.persist(path).context("renaming temp file")?;
    Ok(())
}
