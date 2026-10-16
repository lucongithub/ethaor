//! Engines for many callers at once.
//!
//! An `Engine` caches what it builds — planes, residue lists, its last
//! answers — so a search needs it alone: engines are not shared, they are
//! lent. One per core at most, built on first need, each lent to one search
//! at a time and taken back when it ends — even if its caller has gone.
//!
//! Everything a caller waits on is asynchronous: a free engine (a semaphore,
//! no thread held while queued) and its search (run on the blocking pool,
//! off the async threads). Past `wait` in the queue the caller is turned
//! away (`Failure::Busy`) rather than left to pile up: under a flood, a
//! quick 503 a client retries beats a queue that never drains.
//!
//! **A search that panics takes nothing down**: the panic is caught in its
//! task, its engine — perhaps left half-written — is dropped and built
//! again at the next need, the caller gets a 500, and the pool goes on.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ethaor::shared::els::{Engine, Refusal};

use crate::fault::{self, Fault};
use tokio::sync::Semaphore;

/// Why a search did not answer.
#[derive(Debug, Clone)]
pub enum Failure {
    /// The query itself: the caller's to fix.
    Refused(Refusal),
    /// No engine came free within `wait`.
    Busy,
    /// The client has spent its budget: how long until it may again, and who.
    RateLimited(Duration, std::net::IpAddr),
    /// Ours — a bug, never the caller's: the fault, numbered, with where it
    /// was raised and every step it went through.
    Fault(Box<Fault>),
}

impl Failure {
    /// Ours, raised here.
    #[track_caller]
    pub fn ours(code: &'static str, detail: impl Into<String>) -> Failure {
        Failure::Fault(Box::new(Fault::failure(code, detail)))
    }
}

pub struct Pool {
    idle: Mutex<Vec<Engine>>,
    permits: Arc<Semaphore>,
    size: usize,
    wait: Duration,
    built: AtomicUsize,
    runs: AtomicU64,
}

impl Pool {
    pub fn new(size: usize, wait: Duration) -> Pool {
        let size = size.max(1);
        Pool {
            idle: Mutex::new(Vec::with_capacity(size)),
            permits: Arc::new(Semaphore::new(size)),
            size,
            wait,
            built: AtomicUsize::new(0),
            runs: AtomicU64::new(0),
        }
    }

    /// Runs `work` on an engine of its own, off the async threads.
    pub async fn run<T, F>(self: &Arc<Self>, work: F) -> Result<T, Failure>
    where
        T: Send + 'static,
        F: FnOnce(&mut Engine) -> T + Send + 'static,
    {
        let permit = match tokio::time::timeout(self.wait, self.permits.clone().acquire_owned()).await {
            Ok(Ok(permit)) => permit,
            _ => return Err(Failure::Busy),
        };
        let pool = self.clone();
        // The request's span follows the work, so its logs say which request.
        let span = tracing::Span::current();
        // The engine goes back inside the blocking task, not after the
        // await: a caller dropped mid-search must not take it with it.
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _span = span.enter();
            let taken = pool.idle.lock().unwrap_or_else(|e| e.into_inner()).pop();
            let mut engine = match taken {
                Some(engine) => engine,
                None => match catch_unwind(Engine::new) {
                    Ok(engine) => {
                        pool.built.fetch_add(1, Ordering::Relaxed);
                        engine
                    }
                    Err(panic) => {
                        let fault = fault::from_panic("engine_unbuilt", &*panic, "building an engine for the pool");
                        return Err(Failure::Fault(Box::new(fault)));
                    }
                },
            };
            match catch_unwind(AssertUnwindSafe(|| work(&mut engine))) {
                Ok(out) => {
                    pool.runs.fetch_add(1, Ordering::Relaxed);
                    pool.idle.lock().unwrap_or_else(|e| e.into_inner()).push(engine);
                    Ok(out)
                }
                Err(panic) => {
                    // Its caches may be half-written: the engine is dropped.
                    pool.built.fetch_sub(1, Ordering::Relaxed);
                    let what = "a search on an engine of the pool; the engine is dropped";
                    Err(Failure::Fault(Box::new(fault::from_panic("search_panicked", &*panic, what))))
                }
            }
        })
        .await
        .unwrap_or_else(|e| Err(Failure::ours("task_lost", format!("the blocking task ended without an answer: {e}"))))
    }

    /// Engines at most, searches at once.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Engines built so far — never more than `size`.
    pub fn built(&self) -> usize {
        self.built.load(Ordering::Relaxed)
    }

    /// Engines back in the pool, free.
    pub fn idle(&self) -> usize {
        self.idle.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Searches run, answers from the cache aside.
    pub fn runs(&self) -> u64 {
        self.runs.load(Ordering::Relaxed)
    }

    /// Takes every engine, as a flood would: for the tests of the queue.
    #[cfg(test)]
    pub async fn occupy(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.permits.clone().acquire_many_owned(self.size as u32).await.unwrap()
    }
}
