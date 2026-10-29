//! Faults: every error the service raises, numbered, escalated, logged once
//! and answered in words a person reads.
//!
//! - **An incident number** (`ETH-7K3F92QD`) is given where the error is
//!   raised and never changes: each layer that escalates it adds a step, the
//!   log line carries it, the caller reads it. Quoted by a user, it finds the
//!   one log line that says everything.
//! - **A code and a place**: `code` is stable and raised at one place only
//!   (`every_code_is_raised_at_one_place`), so it names that place by
//!   itself; `at` is the `file:line` it was raised at — the caller's, through
//!   `#[track_caller]`, or a panic's own.
//! - **The escalation**: each layer adds what it was doing (`escalate`). In
//!   normal mode the log has the incident, the code, where, and the message
//!   the user got; **verbose** (`ETHAOR_VERBOSE=1`) adds every step and the
//!   backtrace.
//! - **One log line per fault**, written where it is answered (`respond`) —
//!   never once per layer.
//! - **The caller reads** `{"error": {"code", "message", "incident"}}`: for a
//!   refusal the code is the API's own; for a failure on our side it is
//!   `internal`, its precise code staying in the log.

use std::backtrace::Backtrace;
use std::cell::RefCell;
use std::fmt;
use std::panic::Location;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Map, Value, json};

/// Every fault code the service raises, each at one place: `raise` sites are
/// checked against this list.
pub const CODES: &[(&str, &str)] = &[
    ("handler_panicked", "a request handler panicked: its own location is in `at`"),
    ("search_panicked", "a search panicked on its engine (api/pool.rs)"),
    ("engine_unbuilt", "an engine could not be built (api/pool.rs)"),
    ("task_lost", "a blocking task ended without an answer (api/pool.rs)"),
    ("answer_unwritten", "a search answer could not be serialised (api/els.rs)"),
    ("words_answer_unwritten", "a words answer could not be serialised (api/words.rs)"),
    ("gematria_answer_unwritten", "a gematria search answer could not be serialised (api/gematria.rs)"),
];

/// Whether faults log their whole escalation and backtrace.
static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(on: bool) {
    VERBOSE.store(on, Ordering::Relaxed);
}

pub fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

/// An incident number: `ETH-` and eight characters of Crockford's base 32 —
/// no I, L, O, U, so it reads aloud without doubt. Forty random bits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Incident(String);

impl Incident {
    pub fn new() -> Incident {
        const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        let bits = u64::from_le_bytes(uuid::Uuid::new_v4().as_bytes()[..8].try_into().unwrap());
        let code: String = (0..8).map(|k| ALPHABET[((bits >> (5 * k)) & 31) as usize] as char).collect();
        Incident(format!("ETH-{code}"))
    }

    /// One a page made, if it has the shape of one.
    pub fn parse(text: &str) -> Option<Incident> {
        let rest = text.strip_prefix("ETH-")?;
        let ok = rest.len() == 8 && rest.bytes().all(|b| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&b));
        ok.then(|| Incident(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Incident {
    fn default() -> Incident {
        Incident::new()
    }
}

impl fmt::Display for Incident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whose the fault is, which sets its level and what the caller is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The caller's request: logged `info`, its code is the API's.
    Refused,
    /// The service is full or the caller over budget: `warn`, retry later.
    Unavailable,
    /// Ours: `error`, the caller reads `internal`.
    Failure,
}

/// One step of the escalation: where, and what was being done.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    pub at: String,
    pub what: String,
}

#[derive(Debug, Clone)]
pub struct Fault {
    pub incident: Incident,
    pub kind: Kind,
    pub code: &'static str,
    pub status: StatusCode,
    /// What the caller reads.
    pub message: String,
    /// Origin first.
    pub chain: Vec<Step>,
    pub backtrace: Option<String>,
    pub retry_after: Option<Duration>,
}

fn place(location: &Location<'_>) -> String {
    format!("{}:{}", location.file().replace('\\', "/"), location.line())
}

impl Fault {
    /// Raised here — the caller's place is its origin.
    #[track_caller]
    pub fn new(kind: Kind, code: &'static str, status: StatusCode, message: impl Into<String>) -> Fault {
        let message = message.into();
        let at = place(Location::caller());
        Fault::at(kind, code, status, message.clone(), Step { at, what: message })
    }

    /// Raised with an origin of its own — a panic's.
    pub fn at(kind: Kind, code: &'static str, status: StatusCode, message: String, origin: Step) -> Fault {
        let backtrace = (kind == Kind::Failure && verbose()).then(|| Backtrace::force_capture().to_string());
        Fault { incident: Incident::new(), kind, code, status, message, chain: vec![origin], backtrace, retry_after: None }
    }

    /// The caller's mistake, with the API's code.
    #[track_caller]
    pub fn refused(code: &'static str, status: StatusCode, message: impl Into<String>) -> Fault {
        Fault::new(Kind::Refused, code, status, message)
    }

    /// Ours. The caller reads `message`; the log has `detail` too.
    #[track_caller]
    pub fn failure(code: &'static str, detail: impl Into<String>) -> Fault {
        let at = place(Location::caller());
        Fault::at(Kind::Failure, code, StatusCode::INTERNAL_SERVER_ERROR, FAILED.into(), Step { at, what: detail.into() })
    }

    /// A layer passes it on, saying what it was doing.
    #[track_caller]
    pub fn escalate(mut self, what: impl Into<String>) -> Fault {
        self.chain.push(Step { at: place(Location::caller()), what: what.into() });
        self
    }

    pub fn retry_after(mut self, wait: Duration) -> Fault {
        self.retry_after = Some(wait);
        self
    }

    /// What the caller reads as its code.
    pub fn public_code(&self) -> &'static str {
        if self.kind == Kind::Failure { "internal" } else { self.code }
    }

    /// The log line's fields: always the incident, the code, where and what
    /// the caller read; verbose, every step and the backtrace.
    pub fn fields(&self, verbose: bool) -> Map<String, Value> {
        let mut f = Map::new();
        f.insert("incident".into(), self.incident.as_str().into());
        f.insert("code".into(), self.code.into());
        f.insert("at".into(), self.chain[0].at.clone().into());
        f.insert("status".into(), self.status.as_u16().into());
        f.insert("user_message".into(), self.message.clone().into());
        if verbose {
            f.insert("chain".into(), json!(self.chain));
            if let Some(bt) = &self.backtrace {
                f.insert("backtrace".into(), bt.clone().into());
            }
        }
        f
    }

    /// Writes its one log line.
    pub fn log(&self) {
        let fields = Value::Object(self.fields(verbose())).to_string();
        let (incident, code, at) = (self.incident.as_str(), self.code, self.chain[0].at.as_str());
        match self.kind {
            // A door tried without its key is worth a look.
            Kind::Refused if matches!(self.status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) => {
                tracing::warn!(incident, code, at, fault = %fields, "refused")
            }
            Kind::Refused => tracing::info!(incident, code, at, fault = %fields, "refused"),
            Kind::Unavailable => tracing::warn!(incident, code, at, fault = %fields, "unavailable"),
            Kind::Failure => tracing::error!(incident, code, at, fault = %fields, "failed"),
        }
    }

    /// Logged, then answered.
    pub fn respond(self) -> Response {
        self.log();
        let body = Json(json!({
            "error": { "code": self.public_code(), "message": self.message, "incident": self.incident }
        }));
        let mut response = (self.status, [(header::CACHE_CONTROL, "no-store")], body).into_response();
        if let Some(wait) = self.retry_after {
            let seconds = wait.as_secs_f64().ceil().max(1.0) as u64;
            response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
    }
}

impl IntoResponse for Fault {
    fn into_response(self) -> Response {
        self.respond()
    }
}

/// What a caller reads when the fault is ours.
pub const FAILED: &str = "Something went wrong on our side. Your request was not lost: quote the incident \
    number if you report it.";

// -------------------------------------------------------------------- panics

thread_local! {
    /// The last panic on this thread, as the hook saw it: where, what, and
    /// the stack — for whoever catches it to raise its fault from.
    static PANIC: RefCell<Option<(Step, String)>> = const { RefCell::new(None) };
}

/// Replaces the default panic print: a panic is a fault, logged once by
/// whoever catches it, with its place, message and stack. Once only; under
/// test the default print goes on too — a failing test must still say why.
pub fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            record(info);
            if cfg!(test) {
                previous(info);
            }
        }));
    });
}

fn record(info: &std::panic::PanicHookInfo<'_>) {
    {
        let at = info.location().map(place).unwrap_or_else(|| "unknown".into());
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        let stack = Backtrace::force_capture().to_string();
        PANIC.with(|p| *p.borrow_mut() = Some((Step { at, what }, stack)));
    }
}

/// A panic caught on this thread, as a fault: its own place and message
/// first, then `what` the catcher was doing.
#[track_caller]
pub fn from_panic(code: &'static str, payload: &(dyn std::any::Any + Send), what: &str) -> Fault {
    let caught = PANIC.with(|p| p.borrow_mut().take());
    let (origin, stack) = caught.unwrap_or_else(|| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        (Step { at: "unknown".into(), what: message }, String::new())
    });
    let mut fault = Fault::at(Kind::Failure, code, StatusCode::INTERNAL_SERVER_ERROR, FAILED.into(), origin);
    if !stack.is_empty() {
        fault.backtrace = Some(stack);
    }
    fault.escalate(what)
}
