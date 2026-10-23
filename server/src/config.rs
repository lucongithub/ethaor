//! Everything the service can be tuned by, in one place: read once at start,
//! defaulted when unset or unreadable — a bad value never stops the service,
//! it falls back and says so in the log.
//!
//! A setting comes from the process' environment, or else from a `.env`
//! file (`ETHAOR_ENV`, `.env` by default, never versioned — `.env.example`
//! is): the environment always wins, so a host's secrets override a local
//! file. The file is read, never written into the environment.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// What became of the `.env` file.
#[derive(Debug, Clone, PartialEq)]
pub enum EnvFile {
    Loaded { path: PathBuf, settings: usize },
    /// None there: the environment alone, which is fine.
    Missing { path: PathBuf },
    /// Unreadable or malformed: ignored whole — half a file is worse than
    /// none — and the service starts on the environment and the defaults.
    Broken { path: PathBuf, error: String },
}

/// Where settings are looked up: the environment, then the file.
#[derive(Debug, Clone, Default)]
pub struct Vars {
    file: HashMap<String, String>,
}

impl Vars {
    pub fn load(path: &Path) -> (Vars, EnvFile) {
        let path_buf = path.to_path_buf();
        let items = match dotenvy::from_path_iter(path) {
            Ok(items) => items,
            Err(e) if e.not_found() => return (Vars::default(), EnvFile::Missing { path: path_buf }),
            Err(e) => return (Vars::default(), EnvFile::Broken { path: path_buf, error: e.to_string() }),
        };
        match items.collect::<Result<HashMap<_, _>, _>>() {
            Ok(file) => {
                let settings = file.len();
                (Vars { file }, EnvFile::Loaded { path: path_buf, settings })
            }
            Err(e) => (Vars::default(), EnvFile::Broken { path: path_buf, error: e.to_string() }),
        }
    }

    pub fn get(&self, name: &str) -> Option<String> {
        std::env::var(name).ok().or_else(|| self.file.get(name).cloned())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Searches at once, one engine each (`ETHAOR_ENGINES`): one per core.
    pub engines: usize,
    /// How long a caller may queue for an engine before a 503 `busy`
    /// (`ETHAOR_WAIT_MS`).
    pub wait: Duration,
    /// Bytes of JSON answers kept, shared by every caller (`ETHAOR_CACHE_MB`).
    pub cache_bytes: u64,
    /// The heaviest search admitted, as the cost model estimates it in V8
    /// milliseconds (`ETHAOR_MAX_COST_MS`). Past it: 400 `too_costly`.
    pub max_cost_ms: f64,
    /// What one client may spend at once, and earns back per second, in the
    /// same milliseconds (`ETHAOR_IP_BURST_MS`, `ETHAOR_IP_RATE_MS`). Every
    /// request costs one; a search not in the cache its estimate too.
    pub ip_burst_ms: f64,
    pub ip_rate_ms: f64,
    /// Whether the client is the first address of `CF-Connecting-IP` or
    /// `X-Forwarded-For` — only behind a proxy that sets them
    /// (`ETHAOR_TRUST_PROXY=1`); otherwise anyone could pick their own.
    pub trust_proxy: bool,
    /// Whether faults log their whole escalation and stack
    /// (`ETHAOR_VERBOSE=1`), not only their incident, code and place.
    pub verbose: bool,
    /// The bearer token `GET /api/logs` asks for (`ETHAOR_LOGS_TOKEN`).
    /// Unset, the endpoint is off: logs name clients.
    pub logs_token: Option<String>,
    /// Whether the default browser opens the page once the service listens
    /// (`ETHAOR_OPEN=1`): for a copy started by a double click.
    pub open: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            engines: std::thread::available_parallelism().map_or(4, |n| n.get()),
            wait: Duration::from_millis(2000),
            cache_bytes: 256 << 20,
            max_cost_ms: 300.0,
            ip_burst_ms: 3000.0,
            ip_rate_ms: 300.0,
            trust_proxy: false,
            verbose: false,
            logs_token: None,
            open: false,
        }
    }
}

impl Config {
    pub fn from_env() -> Config {
        Config::from_vars(|name| std::env::var(name).ok())
    }

    /// The same, from any source of variables: what the tests drive.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Config {
        let d = Config::default();
        let number = |name: &str, default: f64| -> f64 {
            match var(name) {
                None => default,
                Some(v) => match v.trim().parse::<f64>() {
                    Ok(n) if n.is_finite() && n > 0.0 => n,
                    _ => {
                        tracing::warn!(variable = name, value = %v, default, "ignored: not a positive number");
                        default
                    }
                },
            }
        };
        let max_cost_ms = number("ETHAOR_MAX_COST_MS", d.max_cost_ms);
        Config {
            engines: number("ETHAOR_ENGINES", d.engines as f64).max(1.0) as usize,
            wait: Duration::from_secs_f64(number("ETHAOR_WAIT_MS", d.wait.as_millis() as f64) / 1000.0),
            cache_bytes: (number("ETHAOR_CACHE_MB", (d.cache_bytes >> 20) as f64) * (1u64 << 20) as f64) as u64,
            max_cost_ms,
            // A burst below the heaviest admitted search would refuse it for
            // ever: it is raised to fit.
            ip_burst_ms: number("ETHAOR_IP_BURST_MS", d.ip_burst_ms).max(max_cost_ms + 1.0),
            ip_rate_ms: number("ETHAOR_IP_RATE_MS", d.ip_rate_ms),
            trust_proxy: var("ETHAOR_TRUST_PROXY").is_some_and(|v| matches!(v.trim(), "1" | "true" | "yes")),
            verbose: var("ETHAOR_VERBOSE").is_some_and(|v| matches!(v.trim(), "1" | "true" | "yes")),
            logs_token: var("ETHAOR_LOGS_TOKEN").filter(|t| t.trim().len() >= 16),
            open: var("ETHAOR_OPEN").is_some_and(|v| matches!(v.trim(), "1" | "true" | "yes")),
        }
    }
}
