//! Runs the service. `ETHAOR_ADDR` sets where it listens (127.0.0.1:8080),
//! `ETHAOR_ROOT` the static build it serves (`web/build`); the rest is in
//! `config.rs`. Each is looked up in the environment, then in `.env`.

// A panic must unwind to be caught and answered 500: built to abort, the
// first one would take every caller down with it.
#[cfg(panic = "abort")]
compile_error!("ethaor-server must unwind on panic: build it with the release profile, not `wasm`");

use std::net::SocketAddr;
use std::path::PathBuf;

use ethaor_server::{app, config, fault, logs};

#[tokio::main]
async fn main() {
    let env = PathBuf::from(std::env::var("ETHAOR_ENV").unwrap_or_else(|_| ".env".into()));
    let (vars, file) = config::Vars::load(&env);
    logs::init_with(|name| vars.get(name));
    match &file {
        config::EnvFile::Loaded { path, settings } => tracing::info!(path = %path.display(), settings, "settings file read"),
        config::EnvFile::Missing { path } => tracing::info!(path = %path.display(), "no settings file: environment only"),
        config::EnvFile::Broken { path, error } => {
            tracing::warn!(path = %path.display(), %error, "settings file ignored: environment and defaults only")
        }
    }
    let addr = vars.get("ETHAOR_ADDR").unwrap_or_else(|| "127.0.0.1:8080".into());
    let root = PathBuf::from(vars.get("ETHAOR_ROOT").unwrap_or_else(|| "web/build".into()));
    let config = config::Config::from_vars(|name| vars.get(name));
    // From here a panic is a fault: logged once, by whoever catches it.
    fault::install_panic_hook();
    fault::set_verbose(config.verbose);
    if !root.join("index.html").is_file() {
        // Degraded, not down: the API answers without the page.
        tracing::warn!(root = %root.display(), "no index.html: the API answers, the page does not");
    }
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(e) => {
            // The one failure there is no serving around.
            tracing::error!(%addr, error = %e, "cannot listen");
            logs::flush();
            std::process::exit(1);
        }
    };
    tracing::info!(
        %addr,
        root = %root.display(),
        engines = config.engines,
        max_cost_ms = config.max_cost_ms,
        logs_endpoint = config.logs_token.is_some(),
        verbose = config.verbose,
        "listening"
    );
    // Where to open it, as a link a terminal makes clickable: the address
    // really bound, an unspecified one named as this machine.
    if let Ok(bound) = listener.local_addr() {
        let url = ethaor_server::browse_url(bound);
        tracing::info!(%url, "running on {url}");
        if config.logs_token.is_some() {
            tracing::info!("logs at {url}logs/ (the page asks the token, or add ?token=)");
        }
        // Degraded, not down: no browser, and the address above still opens it.
        if config.open
            && let Err(e) = ethaor_server::open_browser(&url)
        {
            tracing::warn!(%url, error = %e, "cannot open the browser: open the address by hand");
        }
    }
    let service = app(&root, &config).into_make_service_with_connect_info::<SocketAddr>();
    if let Err(e) = axum::serve(listener, service).with_graceful_shutdown(stopped()).await {
        tracing::error!(error = %e, "the server stopped");
    }
    tracing::info!("stopped");
    // The last lines written before the process goes.
    logs::flush();
}

/// Ctrl-C, or SIGTERM where there is one: requests in flight finish first.
async fn stopped() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = term => {}
    }
    tracing::info!("stopping: finishing the requests in flight");
}
