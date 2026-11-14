//! Platform-agnostic Bilibili music library.
//!
//! Everything a frontend needs lives here: the HTTP API client (`api`), the
//! disk cache (`cache`), the UI-agnostic session state (`app`, `session`) and
//! the player seam (`player`). The desktop TUI and the Android app are both
//! thin frontends over this crate.

mod duration;
mod paths;
mod playlist_store;

pub mod api;
pub mod app;
pub mod cache;
pub mod ffi;
pub mod player;
pub mod resume;
pub mod session;

// Proc-macro UniFFI workflow: scaffolding + metadata for the exported surface
// in `ffi` (see uniffi.toml at the workspace root for Kotlin package settings).
uniffi::setup_scaffolding!("bvox_core");

pub use duration::format_secs;
pub use paths::CorePaths;
