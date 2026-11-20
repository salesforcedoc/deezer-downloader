mod config;
mod crypto;
mod deezer;
mod download;
mod logging;
mod queue;
mod routes;
mod tags;

use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let config = config::load()?;

    if std::env::var("DEEZER_FLAC_QUALITY")
        .map(|value| !value.is_empty() && value != "false")
        .unwrap_or(false)
    {
        logging::log("FLAC was requested, but the Rust app currently downloads MP3 only");
    }

    let address = format!("{}:{}", config.host, config.port);

    let deezer = deezer::Deezer::new(&config.cookie_arl)?;
    let state = queue::AppState::new(config, deezer);

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("could not bind {address} (is another instance running?)"))?;

    logging::log(format!("deezer-downloader listening on http://{address}"));

    if std::env::var("NO_OPEN").is_err() {
        let _ = webbrowser::open(&format!("http://{address}"));
    }

    axum::serve(listener, routes::router(state))
        .await
        .context("server stopped")?;

    Ok(())
}
