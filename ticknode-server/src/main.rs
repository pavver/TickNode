mod actor;
mod plugins;
mod storage;
mod ws;

use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use ticknode_core::{TickBudget, TickEngine};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::storage::Storage;

struct Config {
    bind: String,
    data_dir: PathBuf,
    web_dir: PathBuf,
    bundled_plugins_dir: Option<PathBuf>,
}

impl Config {
    fn from_env() -> Self {
        let var =
            |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_string());
        Self {
            bind: var("TICKNODE_BIND", "0.0.0.0:8099"),
            data_dir: var("TICKNODE_DATA_DIR", "data").into(),
            web_dir: var("TICKNODE_WEB_DIR", "ticknode-editor/dist").into(),
            bundled_plugins_dir: std::env::var("TICKNODE_BUNDLED_PLUGINS_DIR")
                .ok()
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env();
    std::fs::create_dir_all(&config.data_dir)?;

    let storage = Storage::new(config.data_dir.clone());
    let server_settings = storage.load_settings()?;

    let runtime = tokio::runtime::Handle::current();
    let (data_dir, bundled_dir, tick_ms, tick_budget) = (
        config.data_dir.clone(),
        config.bundled_plugins_dir.clone(),
        server_settings.tick_ms,
        server_settings.tick_budget,
    );
    // Plugin calls (configure, node creation) may use synchronous WASI (files), which panics on a
    // tokio worker thread: build the engine on a plain OS thread, like the actor that runs it.
    let (engine, plugins, loaded, storage) = std::thread::spawn(move || -> anyhow::Result<_> {
        let mut engine = TickEngine::new();
        engine.budget = TickBudget {
            limit: (tick_budget != 0).then_some(tick_budget),
        };
        // Plugins may open connections already in `configure` while loading
        engine.host_state.runtime = Some(runtime);
        engine.host_state.tick_duration = Duration::from_millis(tick_ms as u64);
        // Load plugins before the graph so nodes of their types are instantiated live rather than as stubs
        let mut plugins = plugins::Plugins::new(&data_dir).with_bundled_dir(bundled_dir);
        plugins.load_all(&mut engine);
        let loaded = storage.load(&mut engine)?;
        Ok((engine, plugins, loaded, storage))
    })
    .join()
    .map_err(|_| anyhow::anyhow!("engine initialization panicked"))??;
    let handle = actor::spawn(engine, loaded, storage, plugins, server_settings)?;

    let app = router(handle.clone(), &config.web_dir);

    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    info!(bind = %config.bind, data = %config.data_dir.display(), "TickNode server started");

    let shutdown_handle = handle.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            info!("Stop signal received, saving the graph");
            shutdown_handle.shutdown().await;
            // Open WebSocket connections must not keep the process alive.
            tokio::spawn(async {
                tokio::time::sleep(Duration::from_secs(2)).await;
                std::process::exit(0);
            });
        })
        .await?;
    Ok(())
}

fn router(handle: actor::EngineHandle, web_dir: &Path) -> Router {
    let index = web_dir.join("index.html");
    Router::new()
        .route("/ws", get(ws::handler))
        .fallback_service(ServeDir::new(web_dir).fallback(ServeFile::new(index)))
        .with_state(handle)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
}

#[cfg(test)]
#[path = "main/tests/mod.rs"]
mod tests;
