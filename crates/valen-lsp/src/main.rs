//! `valen-lsp` — Valen Language Server Protocol implementation.
//!
//! MVP: diagnostics (parse + type errors) and goto-definition.
//! Uses async-lsp with the omnitrait `LanguageServer` API.

mod convert;
mod server;

use async_lsp::MainLoop;
use server::ServerState;
use tower::ServiceBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let (server, _) = MainLoop::new_server(|client| {
        ServiceBuilder::new()
            .layer(async_lsp::tracing::TracingLayer::default())
            .layer(async_lsp::server::LifecycleLayer::default())
            .layer(async_lsp::panic::CatchUnwindLayer::default())
            .layer(async_lsp::concurrency::ConcurrencyLayer::default())
            .service(ServerState::new_router(client))
    });

    let (stdin, stdout) = (
        async_lsp::stdio::PipeStdin::lock_tokio().unwrap(),
        async_lsp::stdio::PipeStdout::lock_tokio().unwrap(),
    );

    server.run_buffered(stdin, stdout).await?;

    Ok(())
}
