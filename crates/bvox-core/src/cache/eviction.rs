//! Size accounting, LRU eviction and stale-part cleanup.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::SystemTime;

use super::Cache;
use super::model::{CacheStats, STALE_PART_AGE};

impl Cache {
    /// Replace the size cap and evict right away when the new limit is lower.
    /// Returns the new cap in bytes. `keep` is the file of the track currently
    /// playing from cache, which must survive. A limit of 0 disables further
    /// caching without deleting the files already on disk (the UI offers a
    /// separate clear action for that).
    pub fn set_limit_mb(&self, limit_mb: u64, keep: Option<&Path>) -> u64 {
        let limit_bytes = limit_mb.saturating_mul(1024 * 1024);
        self.limit_bytes.store(limit_bytes, Ordering::Relaxed);
        self.evict(keep);
        limit_bytes
    }

    /// Current on-disk usage. Works even when caching is disabled, so stale
    /// files can still be inspected and removed from the UI.
    pub fn stats(&self) -> CacheStats {
        let mut files = Vec::new();
        collect_files(&self.root, &mut files);

        let mut stats = CacheStats {
            limit_bytes: self.limit_bytes(),
            ..CacheStats::default()
        };
        for file in files {
            stats.total_bytes = stats.total_bytes.saturating_add(file.size);
            stats.file_count += 1;
            match category_of(&self.root, &file.path) {
                Some("audio") => stats.audio_bytes += file.size,
                Some("covers") => stats.cover_bytes += file.size,
                Some("lyrics") => stats.lyrics_bytes += file.size,
                Some("view") => stats.view_bytes += file.size,
                _ => {}
            }
        }
        stats
    }

    /// Delete every completed cache file and return the number of bytes freed.
    /// `keep` is a file that must survive (the track currently playing from
    /// cache); `.part` downloads in flight are always skipped.
    pub fn clear(&self, keep: Option<&Path>) -> u64 {
        let mut files = Vec::new();
        collect_files(&self.root, &mut files);

        let mut freed = 0u64;
        for file in files {
            if file
                .path
                .extension()
                .and_then(|extension| extension.to_str())
                == Some("part")
            {
                continue;
            }
            if keep.is_some_and(|keep| keep == file.path) {
                continue;
            }
            if std::fs::remove_file(&file.path).is_ok() {
                freed = freed.saturating_add(file.size);
            }
        }
        freed
    }

    /// Drop least recently used files until the cache is back under 90% of the
    /// configured limit. `keep` is the file that was just written.
    pub(super) fn evict(&self, keep: Option<&Path>) {
        // Read once: a concurrent `set_limit_mb` then only means this pass
        // trims to the older cap; the setter runs its own eviction.
        let limit_bytes = self.limit_bytes();
        if limit_bytes == 0 {
            return;
        }

        let mut files = Vec::new();
        collect_files(&self.root, &mut files);
        let mut total = files.iter().map(|file| file.size).sum::<u64>();
        if total <= limit_bytes {
            return;
        }

        files.sort_by_key(|file| file.modified);
        let target = limit_bytes - limit_bytes / 10;
        for file in files {
            if total <= target {
                break;
            }
            // Parts belong to in-flight downloads; leave them alone.
            if file
                .path
                .extension()
                .and_then(|extension| extension.to_str())
                == Some("part")
            {
                continue;
            }
            if keep.is_some_and(|keep| keep == file.path) {
                continue;
            }
            if std::fs::remove_file(&file.path).is_ok() {
                total = total.saturating_sub(file.size);
            }
        }
    }

    pub(super) fn cleanup_stale_parts(&self) {
        // Housekeeping is safe in every mode; only a zero cap (legacy "off")
        // means the cache folder is not ours to touch.
        if self.limit_bytes() == 0 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(self.root.join("audio")) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("part") {
                continue;
            }
            let stale = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
                .is_some_and(|modified| modified.elapsed().is_ok_and(|age| age > STALE_PART_AGE));
            if stale {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}
pub(super) struct CacheFile {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

/// Top-level cache folder a file belongs to (`audio`, `covers`, ...).
pub(super) fn category_of<'a>(root: &Path, path: &'a Path) -> Option<&'a str> {
    path.strip_prefix(root)
        .ok()?
        .components()
        .next()?
        .as_os_str()
        .to_str()
}

pub(super) fn collect_files(dir: &Path, out: &mut Vec<CacheFile>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_files(&entry.path(), out);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        out.push(CacheFile {
            path: entry.path(),
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }
}
