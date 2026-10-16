//! The settings file: read when there, the environment winning over it,
//! ignored whole when broken — never a reason not to start.

use std::path::PathBuf;

use crate::config::{Config, EnvFile, Vars};

fn file(name: &str, text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("ethaor-{name}-{}.env", std::process::id()));
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_settings_file_is_read() {
    let path = file("read", "# a comment\nETHAOR_MAX_COST_MS=800\nETHAOR_LOGS_TOKEN=\"a-token-long-enough-1234\"\n");
    let (vars, loaded) = Vars::load(&path);
    assert_eq!(loaded, EnvFile::Loaded { path: path.clone(), settings: 2 });
    let config = Config::from_vars(|name| vars.get(name));
    assert_eq!(config.max_cost_ms, 800.0);
    assert_eq!(config.logs_token.as_deref(), Some("a-token-long-enough-1234"));
    let _ = std::fs::remove_file(path);
}

/// What the process' environment holds wins over the file: a host's secret
/// overrides a local file.
#[test]
fn the_environment_wins_over_the_file() {
    let real = std::env::var("PATH").expect("PATH is set everywhere");
    let path = file("wins", "PATH=from-the-file\nETHAOR_UNSET_IN_ANY_ENV=from-the-file\n");
    let (vars, _) = Vars::load(&path);
    assert_eq!(vars.get("PATH"), Some(real));
    assert_eq!(vars.get("ETHAOR_UNSET_IN_ANY_ENV").as_deref(), Some("from-the-file"));
    assert_eq!(std::env::var("ETHAOR_UNSET_IN_ANY_ENV").ok(), None, "the file wrote into the environment");
    let _ = std::fs::remove_file(path);
}

#[test]
fn no_file_is_the_environment_alone() {
    let path = std::env::temp_dir().join("ethaor-no-such-settings.env");
    let (vars, loaded) = Vars::load(&path);
    assert_eq!(loaded, EnvFile::Missing { path });
    assert_eq!(Config::from_vars(|name| vars.get(name)), Config::from_env());
}

/// A broken file is ignored whole — its good lines too — and says why.
#[test]
fn a_broken_file_is_ignored_whole() {
    let path = file("broken", "ETHAOR_MAX_COST_MS=800\nETHAOR_LOGS_TOKEN=\"never closed\n");
    let (vars, loaded) = Vars::load(&path);
    assert!(matches!(loaded, EnvFile::Broken { .. }), "{loaded:?}");
    assert_eq!(vars.get("ETHAOR_MAX_COST_MS"), std::env::var("ETHAOR_MAX_COST_MS").ok());
    let _ = std::fs::remove_file(path);
}

/// The link printed at start opens in a browser: the address bound, and an
/// unspecified one (every interface) named as this machine.
#[test]
fn the_start_link_opens_in_a_browser() {
    let url = |a: &str| crate::browse_url(a.parse().unwrap());
    assert_eq!(url("127.0.0.1:8080"), "http://127.0.0.1:8080/");
    assert_eq!(url("192.168.1.20:80"), "http://192.168.1.20:80/");
    assert_eq!(url("0.0.0.0:8080"), "http://127.0.0.1:8080/");
    assert_eq!(url("[::]:9000"), "http://127.0.0.1:9000/");
    assert_eq!(url("[::1]:9000"), "http://[::1]:9000/");
}
