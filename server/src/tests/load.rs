//! Load, measured rather than asserted: many callers at once, in memory —
//! no socket, so what is timed is the service, not the network. Run in
//! release:
//! `cargo test -p ethaor-server --release load -- --ignored --nocapture`.

use std::path::Path;
use std::time::{Duration, Instant};

use super::{get, query};
use crate::api::Api;
use crate::config::Config;

/// A query no cache has seen: each `k` its own skips.
fn fresh(k: usize) -> String {
    const WORDS: [&str; 6] = ["שלמה", "משה", "אדם", "תורה", "דוד", "ישראל"];
    let skip_min = k;
    let pairs = [
        ("word", WORDS[k % WORDS.len()]),
        ("skip_min", &skip_min.to_string()),
        ("skip_max", &(skip_min + 60).to_string()),
        ("limit", "20"),
    ];
    format!("/api/els?{}", query(&pairs))
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    sorted[((sorted.len() - 1) as f64 * p) as usize]
}

async fn burst(app: &axum::Router, uris: Vec<String>, callers: usize) -> (Duration, Vec<Duration>) {
    let start = Instant::now();
    let mut lanes = Vec::new();
    let per = uris.len().div_ceil(callers);
    for chunk in uris.chunks(per) {
        let (app, chunk) = (app.clone(), chunk.to_vec());
        lanes.push(tokio::spawn(async move {
            let mut times = Vec::new();
            for uri in chunk {
                let t = Instant::now();
                let answer = get(app.clone(), &uri, &[]).await;
                assert_eq!(answer.status, 200, "{uri}");
                times.push(t.elapsed());
            }
            times
        }));
    }
    let mut times = Vec::new();
    for lane in lanes {
        times.extend(lane.await.unwrap());
    }
    times.sort();
    (start.elapsed(), times)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn load() {
    // Logged as in production: every request writes its line.
    crate::logs::init();
    let cores = std::thread::available_parallelism().unwrap().get();
    let wait = Duration::from_secs(60);
    for (label, engines, cache_bytes) in
        [("1 engine, no cache", 1, 0), ("pool, no cache", cores, 0), ("pool and cache", cores, 256 << 20)]
    {
        println!("-- {label}");
        let config = Config { engines, wait, cache_bytes, ..super::open() };
        let app = crate::app_with(Path::new("no-such-build"), Api::new(&config));
        // Warm: postings built, first planes in.
        burst(&app, (0..64).map(fresh).collect(), 8).await;

        let n = 2048;
        let (total, times) = burst(&app, (1000..1000 + n).map(fresh).collect(), 64).await;
        println!(
            "fresh   {n} req, 64 callers: {:.0} req/s, p50 {:.2} ms, p99 {:.2} ms",
            n as f64 / total.as_secs_f64(),
            percentile(&times, 0.5).as_secs_f64() * 1e3,
            percentile(&times, 0.99).as_secs_f64() * 1e3
        );

        let (total, times) = burst(&app, (0..n).map(|k| fresh(10_000 + k % 16)).collect(), 64).await;
        println!(
            "repeat  {n} req, 64 callers: {:.0} req/s, p50 {:.2} ms, p99 {:.2} ms",
            n as f64 / total.as_secs_f64(),
            percentile(&times, 0.5).as_secs_f64() * 1e3,
            percentile(&times, 0.99).as_secs_f64() * 1e3
        );

        // Fresh light queries while heavy ones run: how long the light wait.
        let heavy = format!("/api/els?{}", query(&[("word", "ירושלים"), ("skip_max", "20000"), ("limit", "1")]));
        let mut heavies = Vec::new();
        for k in 0..4 {
            let (app, uri) = (app.clone(), format!("{heavy}&skip_min={k}"));
            heavies.push(tokio::spawn(async move { get(app, &uri, &[]).await }));
        }
        let (_, times) = burst(&app, (20_000..20_256).map(fresh).collect(), 16).await;
        for h in heavies {
            h.await.unwrap();
        }
        println!(
            "beside heavy, 256 fresh light: p50 {:.2} ms, p99 {:.2} ms",
            percentile(&times, 0.5).as_secs_f64() * 1e3,
            percentile(&times, 0.99).as_secs_f64() * 1e3
        );
    }
}
