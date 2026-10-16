//! A panic in the browser, as the page reports it.
//!
//! A WebAssembly module aborts on panic: the page only sees a trap,
//! `unreachable`, with neither message nor place. So before the abort, a
//! hook writes them here — `file:line:column`, a new line, the message —
//! and the page reads them back (`fault_ptr`, `fault_len`) to raise its
//! fault with the Rust place as origin (`web/src/lib/fault.ts`).
//!
//! Exported by both modules. The hook is set in the browser only: natively,
//! the test harness' own print must stay, or a failing test says nothing.

/// The last panic, as the page will read it.
static mut LAST: String = String::new();

/// Remembers a panic's place and message for the page.
#[allow(static_mut_refs)]
pub fn remember(at: &str, what: &str) {
    unsafe {
        LAST.clear();
        LAST.push_str(at);
        LAST.push('\n');
        LAST.push_str(what);
    }
}

/// Sets the hook, once; called by every export a page calls first.
pub fn guard() {
    #[cfg(target_arch = "wasm32")]
    {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            std::panic::set_hook(Box::new(|info| {
                // Forward slashes whatever the machine built it: an
                // incident reads the same everywhere.
                let at = info
                    .location()
                    .map_or(String::new(), |l| format!("{}:{}:{}", l.file().replace('\\', "/"), l.line(), l.column()));
                let what = info
                    .payload()
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| info.payload().downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "panic".into());
                remember(&at, &what);
            }))
        });
    }
}

/// Where the last panic's text lies: place, new line, message. Empty if none.
#[unsafe(no_mangle)]
#[allow(static_mut_refs)]
pub extern "C" fn fault_ptr() -> *const u8 {
    unsafe { LAST.as_ptr() }
}

#[unsafe(no_mangle)]
#[allow(static_mut_refs)]
pub extern "C" fn fault_len() -> u32 {
    unsafe { LAST.len() as u32 }
}

/// Probe: panics on purpose, so the page's tests see a real trap and what
/// the hook left of it. Nothing in the page calls it.
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_panic() {
    guard();
    panic!("probe: a panic on purpose");
}
