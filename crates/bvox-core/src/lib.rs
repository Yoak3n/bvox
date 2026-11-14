//! Platform-agnostic Bilibili music library.
//!
//! Everything a frontend needs lives here: the HTTP API client (`api`), the
//! disk cache (`cache`), the UI-agnostic session state (`app`, `session`) and
//! the player seam (`player`). The desktop TUI and the Android app are both
//! thin frontends over this crate.

use std::path::PathBuf;

pub mod api;
pub mod app;
pub mod cache;
pub mod ffi;
pub mod player;
pub mod playlists;
pub mod resume;
pub mod session;

// Proc-macro UniFFI workflow: scaffolding + metadata for the exported surface
// in `ffi` (see uniffi.toml at the workspace root for Kotlin package settings).
uniffi::setup_scaffolding!("bvox_core");

/// Filesystem locations the library reads and writes. Frontends inject these
/// so the core never touches `HOME` itself: the desktop passes XDG paths,
/// Android passes per-app directories.
#[derive(Clone, Debug)]
pub struct CorePaths {
    /// Cookie and small per-user preference files.
    pub data_dir: PathBuf,
    /// Audio/cover/lyrics/metadata cache root.
    pub cache_dir: PathBuf,
    /// Cache size cap in MiB; 0 disables caching.
    pub cache_mb: u64,
    /// What the cache may store (audio included or small files only).
    pub cache_mode: cache::CacheMode,
}

impl CorePaths {
    /// Desktop defaults: XDG base dirs with the env overrides
    /// (`BVOX_CACHE_DIR`, `BVOX_CACHE_MB`, `BVOX_CACHE_MODE`) applied.
    pub fn desktop_default() -> Self {
        let data_dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join(api::CONFIG_DIR_NAME);

        let cache_dir = std::env::var_os("BVOX_CACHE_DIR")
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())
            .or_else(|| {
                std::env::var_os("XDG_CACHE_HOME")
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .or_else(|| {
                        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache"))
                    })
                    .or_else(|| Some(std::env::temp_dir()))
            })
            .unwrap_or_else(std::env::temp_dir)
            .join(cache::CACHE_DIR_NAME);

        let cache_mb = std::env::var("BVOX_CACHE_MB")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(cache::DEFAULT_LIMIT_MB);

        // `BVOX_CACHE_MB=0` keeps meaning "off" when no mode is given; an
        // explicit `BVOX_CACHE_MODE` wins over that fallback.
        let cache_mode = std::env::var("BVOX_CACHE_MODE")
            .ok()
            .and_then(|value| cache::CacheMode::parse(value.trim()))
            .unwrap_or(if cache_mb == 0 {
                cache::CacheMode::Off
            } else {
                cache::CacheMode::Auto
            });

        Self {
            data_dir,
            cache_dir,
            cache_mb,
            cache_mode,
        }
    }
}
