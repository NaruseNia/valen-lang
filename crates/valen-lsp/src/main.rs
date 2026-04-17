//! `valen-lsp` — Language Server Protocol implementation.
//!
//! MVP scope: syntax errors, type diagnostics, goto definition.
//! Phase 1.5: completion, hover, refactor, semantic tokens.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    todo!("instantiate tower_lsp::LspService with Valen backend and serve stdio")
}
