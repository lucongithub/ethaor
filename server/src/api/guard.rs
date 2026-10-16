//! What one client may spend.
//!
//! Each client — its IP address — has a budget in milliseconds of search,
//! as the cost model estimates them: it may spend `ip_burst_ms` at once and
//! earns back `ip_rate_ms` every second. Every request costs one; a search
//! the cache does not hold costs its estimate too. Past the budget: 429
//! `rate_limited`, with the seconds to wait. A cache hit stays cheap for
//! everyone, while one client asking heavy new searches in a loop can only
//! take its share of the engines.
//!
//! Budgets live in a bounded cache, forgotten after ten idle minutes: a
//! flood of addresses cannot grow the memory.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use moka::sync::Cache;

use crate::config::Config;
use crate::fault::{Fault, Kind};

/// Clients whose budget is remembered at once.
const CLIENTS: u64 = 1_000_000;

struct Budget {
    level: f64,
    at: Instant,
}

#[derive(Clone)]
pub struct Guard {
    budgets: Cache<IpAddr, Arc<Mutex<Budget>>>,
    burst: f64,
    rate: f64,
    trust_proxy: bool,
}

impl Guard {
    pub fn new(config: &Config) -> Guard {
        Guard {
            budgets: Cache::builder().max_capacity(CLIENTS).time_to_idle(Duration::from_secs(600)).build(),
            burst: config.ip_burst_ms,
            rate: config.ip_rate_ms,
            trust_proxy: config.trust_proxy,
        }
    }

    /// Spends `units` of a client's budget, or says how long until it can.
    pub fn charge(&self, ip: IpAddr, units: f64) -> Result<(), Duration> {
        let (burst, rate) = (self.burst, self.rate);
        let budget = self.budgets.get_with(ip, || Arc::new(Mutex::new(Budget { level: burst, at: Instant::now() })));
        let mut b = budget.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        b.level = (b.level + rate * now.duration_since(b.at).as_secs_f64()).min(burst);
        b.at = now;
        if b.level >= units {
            b.level -= units;
            Ok(())
        } else {
            Err(Duration::from_secs_f64(((units - b.level) / rate).max(0.001)))
        }
    }

    /// Who is asking: the socket's address, or — behind a proxy trusted to
    /// set them — the first address the proxy names.
    pub fn client(&self, headers: &HeaderMap, socket: Option<SocketAddr>) -> IpAddr {
        let named = || {
            let cf = headers.get("cf-connecting-ip").and_then(|v| v.to_str().ok());
            let forwarded = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()).and_then(|v| v.split(',').next());
            cf.or(forwarded)?.trim().parse::<IpAddr>().ok()
        };
        (if self.trust_proxy { named() } else { None })
            .or(socket.map(|s| s.ip()))
            .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
    }
}

/// Who a request comes from, for the handlers that charge more than one.
#[derive(Clone)]
pub struct Client {
    pub ip: IpAddr,
    pub guard: Guard,
}

/// Charges every request under `/api` one unit, and hands the handlers the
/// client to charge the rest to.
pub async fn limit(State(guard): State<Guard>, mut request: Request, next: Next) -> Response {
    let socket = request.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0);
    let ip = guard.client(request.headers(), socket);
    if let Err(wait) = guard.charge(ip, 1.0) {
        return rate_limited(wait, ip);
    }
    request.extensions_mut().insert(Client { ip, guard });
    next.run(request).await
}

/// The budget spent: answered with how long to wait. The client's address
/// is a step of the fault — in the verbose log, never in the answer.
#[track_caller]
pub fn rate_limited(wait: Duration, client: IpAddr) -> Response {
    let seconds = wait.as_secs_f64().ceil().max(1.0) as u64;
    let message = format!("this client has spent its budget; retry in {seconds} s");
    Fault::new(Kind::Unavailable, "rate_limited", StatusCode::TOO_MANY_REQUESTS, message)
        .escalate(format!("charging client {client}"))
        .retry_after(wait)
        .respond()
}
