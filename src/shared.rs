//! What both transports run: the WebAssembly exports (`wasm`) and the HTTP
//! API (`server/`). Each search or lookup is written once here, as plain
//! Rust values; a transport only moves them — records across the module's
//! memory, JSON over HTTP.
//!
//! The computation proper stays in its own module (`els`, `corpus`): this
//! layer validates what a caller asks, pages the answer and names the places
//! it falls on, which is what every caller would otherwise redo apart.

pub mod contents;
pub mod els;
pub mod passage;
pub mod reference;
pub mod words;

#[cfg(test)]
mod tests;
