//! ethaor — the Tanakh as a zoomable map, with no background.
//!
//! All the computation lives here, compiled to WebAssembly: corpus, layout,
//! camera, zoom folding, pruning, click detection. The page only paints the
//! command list produced for each frame. Nothing travels over the network
//! during interaction.
//!
//! One single thing stays with the browser: **measuring and drawing text**.
//! That is where the shaping of pointed Hebrew lives — bidi, combining marks —
//! and no in-house implementation would do it as well. Rust asks for the
//! widths it lacks, caches them, and never comes back to them.

// A WebAssembly build exports one feature's functions: the other's code is
// then unused, and link-time optimisation drops it.
#![cfg_attr(not(all(feature = "map", feature = "search")), allow(dead_code, unused_imports))]

mod corpus;
mod els;
mod gematria;
mod letter;
mod map;
mod pack;
pub mod shared;
#[cfg(test)]
mod tests;
mod treemap;
mod variant;
pub mod wasm;
mod words;
