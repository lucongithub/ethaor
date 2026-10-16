//! Logs: structured, to standard output and to files on disk, followed live
//! from those files.
//!
//! - Every event is one JSON object — time, level, target, message, its
//!   fields and its request's id — so a log platform reads them without a
//!   parser of its own. Formatted once, written by threads of their own:
//!   logging costs a request no wait on the output or the disk.
//!   `ETHAOR_LOG` filters them (`info` by default, `RUST_LOG` syntax);
//!   `ETHAOR_LOG_FORMAT=pretty` writes standard output for a terminal
//!   instead — the files stay JSON, they are what `GET /api/logs` reads.
//! - **The files** (`ETHAOR_LOG_DIR`, `logs` by default, `off` for none) are
//!   `ethaor-<sequence>.log`: each process starts a new one, and one full
//!   rolls to the next. They are bounded on disk (`ETHAOR_LOG_MB`): `FILES`
//!   at most, the oldest deleted.
//! - **Logging never blocks and never fails the service**: past `LINES`
//!   waiting a line is dropped, a write that fails loses its line and the
//!   file is opened anew a little later, and a directory that cannot be
//!   written leaves standard output alone, the endpoint saying so.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::stream::{self, Stream};
use serde::Deserialize;
use serde_json::{Map, Value};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_appender::non_blocking::{NonBlocking, NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Lines `GET /api/logs` replays at most before following.
pub const HISTORY: usize = 2000;

/// Log files kept at most: the disk budget is shared between them.
pub const FILES: usize = 8;

/// Lines waiting for a writing thread before new ones are dropped.
const LINES: usize = 100_000;

/// After a failed write, how long before the file is opened anew.
const RETRY: Duration = Duration::from_secs(10);

/// How long a follower waits for word of new lines before it looks all
/// the same: the writer tells it at once (`WRITTEN`), this is the net under
/// a word missed.
const POLL: Duration = Duration::from_secs(1);

/// Bytes a file gathers before it is written out; the writing thread
/// flushes it whenever its queue is empty, so a line waits for no more than
/// the lines queued with it.
const BUFFER: usize = 64 << 10;

/// Said by the writer each time lines reach the disk: what wakes the
/// followers, at once, instead of every `POLL`.
fn written() -> &'static tokio::sync::Notify {
    static WRITTEN: OnceLock<tokio::sync::Notify> = OnceLock::new();
    WRITTEN.get_or_init(tokio::sync::Notify::new)
}

/// Bytes read at once, back for the history or on for a follower.
const CHUNK: u64 = 1 << 20;

/// The writing threads: dropped by `flush`, they write what waits, then stop.
static WRITERS: Mutex<Vec<WorkerGuard>> = Mutex::new(Vec::new());

/// The directory the files are written to, once they are.
static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Where the logs go on disk, from `ETHAOR_LOG_DIR` and `ETHAOR_LOG_MB`.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// `None`: standard output only.
    pub dir: Option<PathBuf>,
    /// Bytes all the files may take together.
    pub bytes: u64,
}

impl Settings {
    pub const DIR: &str = "logs";
    pub const MB: f64 = 256.0;

    /// The settings, and what was wrong with them if anything: said in the
    /// log once there is one.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> (Settings, Option<String>) {
        let dir = match var("ETHAOR_LOG_DIR").map(|d| d.trim().to_string()) {
            None => Some(PathBuf::from(Settings::DIR)),
            Some(d) if d.is_empty() || d.eq_ignore_ascii_case("off") => None,
            Some(d) => Some(PathBuf::from(d)),
        };
        let (mb, note) = match var("ETHAOR_LOG_MB") {
            None => (Settings::MB, None),
            Some(v) => match v.trim().parse::<f64>() {
                Ok(n) if n.is_finite() && n > 0.0 => (n, None),
                _ => (Settings::MB, Some(format!("ETHAOR_LOG_MB={v} ignored: not a positive number"))),
            },
        };
        (Settings { dir, bytes: (mb * (1u64 << 20) as f64) as u64 }, note)
    }
}

/// Installs the logging from the environment, once; later calls do nothing.
pub fn init() {
    init_with(|name| std::env::var(name).ok())
}

/// The same, reading the `ETHAOR_LOG*` settings from `var`.
pub fn init_with(var: impl Fn(&str) -> Option<String>) {
    // Once: two callers at once would each open a file, and the loser's
    // pruning could delete the winner's.
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| install(&var));
}

fn install(var: &dyn Fn(&str) -> Option<String>) {
    let directives = var("ETHAOR_LOG");
    let filter = || directives.as_deref().and_then(|d| EnvFilter::try_new(d).ok()).unwrap_or_else(|| EnvFilter::new("info"));
    let pretty = var("ETHAOR_LOG_FORMAT").is_some_and(|f| f == "pretty");
    let (mut settings, note) = Settings::from_vars(var);
    if cfg!(test) {
        // Under test, a directory of this run's own, read back by the tests.
        settings.dir = Some(test_dir());
    }
    let mut problem = None;
    let mut dir = None;
    let mut json_outs = Vec::new();
    if let Some(path) = &settings.dir {
        match Rolling::open(path, settings.bytes) {
            Ok(rolling) => {
                json_outs.push(spawn(rolling));
                dir = Some(path.clone());
            }
            Err(e) => problem = Some(format!("{}: {e}", path.display())),
        }
    }
    // Under test, nowhere on standard output — the tests read the files.
    let mut pretty_out = None;
    if !cfg!(test) {
        let out = spawn(io::stdout());
        if pretty {
            pretty_out = Some(out);
        } else {
            json_outs.push(out);
        }
    }
    let readable = pretty_out.map(|w| tracing_subscriber::fmt::layer().with_target(false).with_writer(w).with_filter(filter()));
    let _ = DIR.set(dir);
    // Already installed by someone else: theirs is kept.
    let installed = tracing_subscriber::registry()
        .with(readable)
        .with(JsonLayer { outs: json_outs }.with_filter(filter()))
        .try_init()
        .is_ok();
    if installed {
        if let Some(note) = note {
            tracing::warn!(note, "a log setting fell back");
        }
        if let Some(problem) = problem {
            tracing::warn!(problem, "no log file: standard output only, GET /api/logs unavailable");
        }
    }
}

/// Written by a thread of its own, never by the thread answering: a slow
/// console or disk must not slow a request. Past `LINES` waiting, lines are
/// dropped rather than waited for.
fn spawn<W: Write + Send + 'static>(writer: W) -> NonBlocking {
    let (writer, guard) = NonBlockingBuilder::default().lossy(true).buffered_lines_limit(LINES).finish(writer);
    WRITERS.lock().unwrap_or_else(|e| e.into_inner()).push(guard);
    writer
}

/// Writes what waits, then stops the writing threads: called last, at
/// shutdown, so the last lines are not lost with the process.
pub fn flush() {
    let guards = std::mem::take(&mut *WRITERS.lock().unwrap_or_else(|e| e.into_inner()));
    drop(guards);
}

/// The directory the files are written to: `None` before `init`, or when
/// there is none.
pub fn dir() -> Option<&'static Path> {
    DIR.get().and_then(|d| d.as_deref())
}

/// This test run's own directory, the other runs' cleared.
fn test_dir() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/test-logs");
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            // A run still going holds its files: left alone.
            let _ = fs::remove_dir_all(entry.path());
        }
    }
    root.join(std::process::id().to_string())
}

// --------------------------------------------------------------------- files

/// The log files in `dir`, by sequence, oldest first.
pub fn files(dir: &Path) -> Vec<(u64, PathBuf)> {
    let mut found: Vec<(u64, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let seq = name.to_str()?.strip_prefix("ethaor-")?.strip_suffix(".log")?.parse().ok()?;
            Some((seq, entry.path()))
        })
        .collect();
    found.sort();
    found
}

fn file_name(seq: u64) -> String {
    format!("ethaor-{seq:08}.log")
}

/// The files, written in turn: a new one per process and per `bytes /
/// FILES` written, the oldest deleted past `FILES`. Lines are gathered and
/// written out at each `flush` — the writing thread's, once its queue is
/// empty — not one system call each. Never fails a write: a line it cannot
/// write is lost, and the file opened anew after `RETRY`.
pub struct Rolling {
    dir: PathBuf,
    file_bytes: u64,
    seq: u64,
    file: Option<BufWriter<File>>,
    written: u64,
    retry_at: Instant,
}

impl Rolling {
    /// A new file after the last one in `dir`, created if need be.
    pub fn open(dir: &Path, bytes: u64) -> io::Result<Rolling> {
        fs::create_dir_all(dir)?;
        let seq = files(dir).last().map_or(1, |(seq, _)| seq + 1);
        let file_bytes = (bytes / FILES as u64).max(1);
        let mut rolling = Rolling { dir: dir.to_path_buf(), file_bytes, seq, file: None, written: 0, retry_at: Instant::now() };
        rolling.file = Some(rolling.create()?);
        rolling.prune();
        Ok(rolling)
    }

    fn create(&self) -> io::Result<BufWriter<File>> {
        let file = OpenOptions::new().create_new(true).append(true).open(self.dir.join(file_name(self.seq)))?;
        Ok(BufWriter::with_capacity(BUFFER, file))
    }

    /// The next file, the one before written out whole first; the oldest
    /// past `FILES` deleted.
    fn roll(&mut self) {
        if let Some(mut file) = self.file.take() {
            let _ = file.flush();
        }
        self.seq += 1;
        self.written = 0;
        self.file = self.create().ok();
        if self.file.is_none() {
            self.retry_at = Instant::now() + RETRY;
        }
        self.prune();
    }

    fn prune(&self) {
        let all = files(&self.dir);
        for (_, path) in &all[..all.len().saturating_sub(FILES)] {
            // A follower may hold it open: it is told it lagged.
            let _ = fs::remove_file(path);
        }
    }
}

impl Write for Rolling {
    fn write(&mut self, line: &[u8]) -> io::Result<usize> {
        let full = self.written > 0 && self.written + line.len() as u64 > self.file_bytes;
        if full || (self.file.is_none() && Instant::now() >= self.retry_at) {
            self.roll();
        }
        if let Some(file) = &mut self.file {
            match file.write_all(line) {
                Ok(()) => self.written += line.len() as u64,
                Err(_) => {
                    self.file = None;
                    self.retry_at = Instant::now() + RETRY;
                }
            }
        }
        // Lost or written, the service goes on.
        Ok(line.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = &mut self.file
            && file.flush().is_err()
        {
            // What it held is lost; the file is opened anew later.
            self.file = None;
            self.retry_at = Instant::now() + RETRY;
        }
        written().notify_waiters();
        Ok(())
    }
}

// ------------------------------------------------------------------ reading

/// A place in the files: which one, and how far into it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub seq: u64,
    pub at: u64,
}

/// Where the last whole line ends: what a follower starts after, and the
/// history stops at — a line half written is neither's.
pub fn end(dir: &Path) -> Position {
    let Some((seq, path)) = files(dir).pop() else { return Position { seq: 0, at: 0 } };
    let len = fs::metadata(&path).map_or(0, |m| m.len());
    let from = len.saturating_sub(CHUNK);
    let tail = read_range(&path, from, len).unwrap_or_default();
    let at = match tail.iter().rposition(|&b| b == b'\n') {
        Some(i) => from + i as u64 + 1,
        None => from,
    };
    Position { seq, at }
}

fn read_range(path: &Path, from: u64, to: u64) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(from))?;
    let mut bytes = Vec::with_capacity((to - from) as usize);
    file.take(to - from).read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[derive(Deserialize)]
struct Leveled<'a> {
    level: &'a str,
}

/// A line's level, if it is a log line.
fn level_of(line: &str) -> Option<Level> {
    serde_json::from_str::<Leveled>(line).ok().and_then(|l| level(l.level))
}

/// The last `n` lines at `level` or above before `until`, oldest first,
/// read back across the files.
pub fn recent(dir: &Path, until: Position, n: usize, level: Level) -> Vec<String> {
    let mut kept = Vec::new();
    for (seq, path) in files(dir).into_iter().rev().filter(|(seq, _)| *seq <= until.seq) {
        if kept.len() >= n {
            break;
        }
        let end = if seq == until.seq { until.at } else { fs::metadata(&path).map_or(0, |m| m.len()) };
        back(&path, end, n, level, &mut kept);
    }
    kept.reverse();
    kept
}

/// Lines of one file before `end`, newest first, until `kept` holds `n`.
fn back(path: &Path, mut end: u64, n: usize, level: Level, kept: &mut Vec<String>) {
    // The start of a line cut by the chunk before.
    let mut carry: Vec<u8> = Vec::new();
    while end > 0 && kept.len() < n {
        let from = end.saturating_sub(CHUNK);
        let Ok(mut bytes) = read_range(path, from, end) else { return };
        bytes.extend_from_slice(&carry);
        let mut lines: Vec<&[u8]> = bytes.split(|&b| b == b'\n').collect();
        // The first piece may be the end of a line that starts further back.
        let first = if from > 0 { lines.remove(0) } else { &[][..] };
        for line in lines.iter().rev() {
            if let Ok(line) = std::str::from_utf8(line)
                && level_of(line).is_some_and(|l| l <= level)
            {
                kept.push(line.to_string());
                if kept.len() == n {
                    return;
                }
            }
        }
        carry = first.to_vec();
        end = from;
    }
}

/// What a follower is handed.
#[derive(Debug, Clone, PartialEq)]
pub enum Followed {
    Line(String),
    /// Files deleted before it read them: it was too slow, or arrived on a
    /// file already gone.
    Lagged(u64),
}

struct Follower {
    dir: PathBuf,
    at: Position,
    level: Level,
    /// The start of a line not yet whole.
    partial: Vec<u8>,
    ready: std::collections::VecDeque<Followed>,
}

/// Every line at `level` or above written after `from`, as it is written,
/// across the files — never waited for by the service: it only reads.
pub fn follow(dir: PathBuf, from: Position, level: Level) -> impl Stream<Item = Followed> {
    let follower = Follower { dir, at: from, level, partial: Vec::new(), ready: Default::default() };
    stream::unfold(follower, |mut f| async move {
        loop {
            if let Some(next) = f.ready.pop_front() {
                return Some((next, f));
            }
            // Listening before looking: lines written while it looks still
            // wake it.
            let word = written().notified();
            tokio::pin!(word);
            word.as_mut().enable();
            let (dir, at) = (f.dir.clone(), f.at);
            let read = tokio::task::spawn_blocking(move || step(&dir, at)).await.unwrap_or(Step::Nothing);
            match read {
                Step::Bytes(bytes) => {
                    f.at.at += bytes.len() as u64;
                    f.take(&bytes);
                }
                Step::Next { seq, missed } => {
                    f.at = Position { seq, at: 0 };
                    f.partial.clear();
                    if missed > 0 {
                        f.ready.push_back(Followed::Lagged(missed));
                    }
                }
                Step::Nothing => {
                    let _ = tokio::time::timeout(POLL, word).await;
                }
            }
        }
    })
}

impl Follower {
    fn take(&mut self, bytes: &[u8]) {
        self.partial.extend_from_slice(bytes);
        let Some(last) = self.partial.iter().rposition(|&b| b == b'\n') else { return };
        let whole: Vec<u8> = self.partial.drain(..=last).collect();
        for line in whole.split(|&b| b == b'\n') {
            if let Ok(line) = std::str::from_utf8(line)
                && level_of(line).is_some_and(|l| l <= self.level)
            {
                self.ready.push_back(Followed::Line(line.to_string()));
            }
        }
    }
}

enum Step {
    Bytes(Vec<u8>),
    /// On to file `seq`, `missed` files gone before it.
    Next { seq: u64, missed: u64 },
    Nothing,
}

/// One look at the files from `at`. A newer file is looked for first: if
/// there is one, the current file is complete, and once read to its end
/// the follower moves on.
fn step(dir: &Path, at: Position) -> Step {
    let all = files(dir);
    let newer = all.iter().map(|(seq, _)| *seq).find(|seq| *seq > at.seq);
    let current = all.iter().find(|(seq, _)| *seq == at.seq).map(|(_, path)| path);
    let Some(path) = current else {
        // Gone, read or not: deleted under a slow follower. Sequence 0 is
        // no file at all, nothing missed.
        return match newer {
            Some(seq) => Step::Next { seq, missed: if at.seq == 0 { 0 } else { seq - at.seq } },
            None => Step::Nothing,
        };
    };
    let len = fs::metadata(path).map_or(at.at, |m| m.len());
    if len > at.at {
        return match read_range(path, at.at, len.min(at.at + CHUNK)) {
            Ok(bytes) if !bytes.is_empty() => Step::Bytes(bytes),
            _ => Step::Nothing,
        };
    }
    match newer {
        Some(seq) => Step::Next { seq, missed: seq - at.seq - 1 },
        None => Step::Nothing,
    }
}

// ------------------------------------------------------------------ writing

/// Formats each event once, as JSON, for every output.
struct JsonLayer {
    outs: Vec<NonBlocking>,
}

impl<S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>> Layer<S> for JsonLayer {
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut fields = Map::new();
        let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        fields.insert("ts_ms".into(), ms.into());
        fields.insert("level".into(), meta.level().as_str().into());
        fields.insert("target".into(), meta.target().into());
        // The request an event belongs to: its span's fields.
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope.from_root() {
                if let Some(Spanned(map)) = span.extensions().get::<Spanned>() {
                    for (k, v) in map {
                        fields.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        event.record(&mut Fields(&mut fields));
        let mut line = Value::Object(fields).to_string().into_bytes();
        line.push(b'\n');
        for out in &self.outs {
            // Lossy by design: a full queue drops the line, never waits.
            let _ = out.make_writer().write_all(&line);
        }
    }

    fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &tracing::span::Id, ctx: Context<'_, S>) {
        let mut map = Map::new();
        attrs.record(&mut Fields(&mut map));
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Spanned(map));
        }
    }
}

struct Spanned(Map<String, Value>);

struct Fields<'a>(&'a mut Map<String, Value>);

impl Visit for Fields<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.insert(field.name().into(), format!("{value:?}").into());
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.0.insert(field.name().into(), value.into());
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().into(), value.into());
    }
}

/// A level by its name, as `GET /api/logs?level=` takes it.
pub fn level(name: &str) -> Option<Level> {
    match name.trim().to_ascii_lowercase().as_str() {
        "error" => Some(Level::ERROR),
        "warn" | "warning" => Some(Level::WARN),
        "info" => Some(Level::INFO),
        "debug" => Some(Level::DEBUG),
        "trace" => Some(Level::TRACE),
        _ => None,
    }
}
