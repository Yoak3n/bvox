//! bvox terminal frontend entry point.

mod cli;
mod cover;
mod mpv;
mod run;
mod tui_app;
mod ui;

/// Entry point; argument handling and headless modes live in `cli`.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli::run().await
}
