//! On-disk cache for audio streams, covers, lyrics and video metadata.
//!
//! The frontend picks the root via `CorePaths` (XDG cache dir on desktop,
//! per-app cache on Android). Layout:
//!
//! ```text
//! audio/<bvid>_<cid>_<quality_id>_<bandwidth>.m4s
//! covers/<md5(thumbnail url)>.img
//! lyrics/<bvid>_<cid>.json
//! view/<bvid>.json
//! ```
//!
//! Audio is written to a `.part` file and renamed into place, so a truncated
//! download never looks like a cache hit. The whole cache is bounded by the
//! configured limit (default 1 GiB, changeable at runtime via
//! [`Cache::set_limit_mb`]); eviction drops the least recently used files.
//!
//! [`CacheMode`] picks what is stored at all: auto caches audio plus covers,
//! lyrics and metadata; small-files skips audio downloads and keeps the small
//! files; off bypasses the cache entirely. A size limit of 0 still disables
//! the cache as a whole for callers of the legacy limit knob.
//!
//! [`Cache::stats`] reports the space used per category and [`Cache::clear`]
//! empties the cache from the UI; in-flight `.part` downloads are left alone
//! so a running download cannot be corrupted by a cleanup.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::api::{LyricLine, VideoDetail};

pub const CACHE_DIR_NAME: &str = "bvox";
pub const DEFAULT_LIMIT_MB: u64 = 1024;
const VIEW_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const STALE_PART_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// Bump when lyric fetching/parsing changes so old files are discarded
/// instead of served forever. Version 2 drops lyrics poisoned by the
/// unsigned `/x/player/v2` endpoint returning another video's subtitle.
const LYRICS_VERSION: u32 = 2;

/// What the on-disk cache is allowed to store.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, uniffi::Enum)]
#[repr(u8)]
pub enum CacheMode {
    /// 自动缓存：音频、封面、歌词和视频信息都落盘。
    #[default]
    Auto = 0,
    /// 仅小文件：音频始终在线播放，只缓存封面、歌词和视频信息。
    SmallFiles = 1,
    /// 关闭：不读也不写缓存，已有文件保留到手动清理。
    Off = 2,
}

impl CacheMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "自动缓存",
            Self::SmallFiles => "仅小文件",
            Self::Off => "关闭",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Auto => "音频、封面、歌词和视频信息都缓存，下次播放直接用本地文件",
            Self::SmallFiles => "音频始终在线播放，只缓存封面、歌词和视频信息",
            Self::Off => "不读也不写缓存，已有文件保留到手动清理",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::SmallFiles,
            Self::SmallFiles => Self::Off,
            Self::Off => Self::Auto,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "small_files" | "small-files" | "small" => Some(Self::SmallFiles),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    fn as_u8(self) -> u8 {
        self as u8
    }

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::SmallFiles,
            2 => Self::Off,
            _ => Self::Auto,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CachedAudio {
    pub path: PathBuf,
    pub quality_id: i64,
    pub bandwidth: i64,
}

/// One playable entry for the offline "cached songs" list. Built by joining a
/// completed audio file with its cached `view` metadata; files without view
/// metadata are skipped because playback still needs the page/cid mapping.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct CachedTrack {
    pub bvid: String,
    pub cid: i64,
    /// Page index inside a multi-P video (0 for single-P).
    pub page_index: u32,
    pub aid: i64,
    pub title: String,
    pub author: String,
    pub duration: String,
    pub pic: Option<String>,
    pub quality: String,
    /// Empty for single-P videos.
    pub part: String,
}

#[derive(Serialize, Deserialize)]
struct CachedView {
    fetched_at: u64,
    detail: VideoDetail,
}

#[derive(Serialize, Deserialize)]
struct CachedLyrics {
    version: u32,
    lines: Vec<LyricLine>,
}

/// On-disk cache usage, grouped by what the files are used for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub total_bytes: u64,
    /// Configured cap in bytes; 0 when the cache is disabled.
    pub limit_bytes: u64,
    pub audio_bytes: u64,
    pub cover_bytes: u64,
    pub lyrics_bytes: u64,
    pub view_bytes: u64,
    pub file_count: u64,
}

/// Human-readable size for UI labels (`12.3 MB`).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

pub struct Cache {
    root: PathBuf,
    /// Current cap in bytes, 0 when the cache is disabled. Atomic so the UI
    /// can change it while downloads run.
    limit_bytes: AtomicU64,
    /// What may be stored, changeable at runtime from the UI.
    mode: AtomicU8,
    /// `bvid:cid` keys with a background download in progress.
    inflight: Mutex<HashSet<String>>,
}

impl Cache {
    pub fn new(root: PathBuf, limit_mb: u64, mode: CacheMode) -> Self {
        let cache = Self {
            root,
            limit_bytes: AtomicU64::new(limit_mb.saturating_mul(1024 * 1024)),
            mode: AtomicU8::new(mode.as_u8()),
            inflight: Mutex::new(HashSet::new()),
        };
        cache.cleanup_stale_parts();
        cache
    }

    fn limit_bytes(&self) -> u64 {
        self.limit_bytes.load(Ordering::Relaxed)
    }

    pub fn mode(&self) -> CacheMode {
        CacheMode::from_u8(self.mode.load(Ordering::Relaxed))
    }

    /// Switch what the cache stores. Existing files are kept; the caller
    /// offers a separate clear action. A size limit of 0 still wins, but
    /// choosing a caching mode again restores the default cap so the choice
    /// is not silently inert.
    pub fn set_mode(&self, mode: CacheMode) {
        if mode != CacheMode::Off && self.limit_bytes() == 0 {
            self.limit_bytes.store(
                DEFAULT_LIMIT_MB.saturating_mul(1024 * 1024),
                Ordering::Relaxed,
            );
        }
        self.mode.store(mode.as_u8(), Ordering::Relaxed);
    }

    /// Audio is cached (and read back) only in auto mode.
    pub fn audio_enabled(&self) -> bool {
        self.mode() == CacheMode::Auto && self.limit_bytes() > 0
    }

    /// Covers, lyrics and metadata are cached in every mode except off.
    pub fn metadata_enabled(&self) -> bool {
        self.mode() != CacheMode::Off && self.limit_bytes() > 0
    }

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

    /// Return a completed audio file for this page, preferring the newest copy.
    pub fn find_audio(&self, bvid: &str, cid: i64) -> Option<CachedAudio> {
        if !self.audio_enabled() {
            return None;
        }

        let entries = std::fs::read_dir(self.root.join("audio")).ok()?;
        let mut best: Option<(SystemTime, CachedAudio)> = None;

        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                continue;
            };
            let Some((quality_id, bandwidth)) = parse_audio_file_name(&name, bvid, cid) else {
                continue;
            };
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let candidate = CachedAudio {
                path: entry.path(),
                quality_id,
                bandwidth,
            };
            if best
                .as_ref()
                .is_none_or(|(best_modified, _)| modified >= *best_modified)
            {
                best = Some((modified, candidate));
            }
        }

        let (_, cached) = best?;
        touch(&cached.path);
        Some(cached)
    }

    /// BVIDs with at least one completed audio file. Used to mark cached
    /// tracks in list views; scans the audio directory once per refresh.
    pub fn cached_bvids(&self) -> HashSet<String> {
        if !self.audio_enabled() {
            return HashSet::new();
        }
        let Ok(entries) = std::fs::read_dir(self.root.join("audio")) else {
            return HashSet::new();
        };
        let mut bvids = HashSet::new();
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                continue;
            };
            // Completed files are `{bvid}_{cid}_{quality}_{bandwidth}.m4s`;
            // in-flight `.part` files never match.
            let Some(stem) = name.strip_suffix(".m4s") else {
                continue;
            };
            let mut parts = stem.split('_');
            let (Some(bvid), Some(cid)) = (parts.next(), parts.next()) else {
                continue;
            };
            if bvid.is_empty() || cid.parse::<i64>().is_err() {
                continue;
            }
            bvids.insert(bvid.to_string());
        }
        bvids
    }

    /// Playable entries for the offline list: one per BVID, using the newest
    /// cached audio file. The list is newest-first and only contains tracks
    /// whose `view` metadata is still available, so every row is playable
    /// without a network request.
    pub fn cached_tracks(&self) -> Vec<CachedTrack> {
        if !self.audio_enabled() {
            return Vec::new();
        }
        let Ok(entries) = std::fs::read_dir(self.root.join("audio")) else {
            return Vec::new();
        };

        let mut newest: HashMap<String, (SystemTime, i64, i64, i64)> = HashMap::new();
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
                continue;
            };
            let Some((bvid, cid, quality_id, bandwidth)) = parse_audio_stem(&name) else {
                continue;
            };
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let replace = newest.get(bvid).is_none_or(|(best, ..)| modified >= *best);
            if replace {
                newest.insert(bvid.to_string(), (modified, cid, quality_id, bandwidth));
            }
        }

        let mut tracks = newest
            .into_iter()
            .filter_map(|(bvid, (modified, cid, quality_id, bandwidth))| {
                let detail = self
                    .get_view(&bvid)
                    .or_else(|| self.get_stale_view(&bvid))?;
                let page_index = detail
                    .pages
                    .iter()
                    .position(|page| page.cid == cid)
                    .unwrap_or(0);
                let page = detail.pages.get(page_index);
                let duration = page.map(|page| page.duration).unwrap_or(detail.duration);
                Some((
                    modified,
                    CachedTrack {
                        bvid,
                        cid,
                        page_index: page_index as u32,
                        aid: detail.aid,
                        title: crate::api::clean_title(&detail.title),
                        author: detail.author.clone(),
                        duration: crate::api::format_secs(duration),
                        pic: detail.pic.clone(),
                        quality: format!(
                            "{} · {} kbps",
                            crate::api::quality_label(quality_id),
                            bandwidth / 1000
                        ),
                        part: page.map(|page| page.part.clone()).unwrap_or_default(),
                    },
                ))
            })
            .collect::<Vec<_>>();
        tracks.sort_by(|(a, _), (b, _)| b.cmp(a));
        tracks.into_iter().map(|(_, track)| track).collect()
    }

    pub fn audio_target(&self, bvid: &str, cid: i64, quality_id: i64, bandwidth: i64) -> PathBuf {
        self.root
            .join("audio")
            .join(audio_file_name(bvid, cid, quality_id, bandwidth))
    }

    pub fn audio_part(&self, final_path: &Path) -> PathBuf {
        let mut name = final_path
            .file_name()
            .map(ToOwned::to_owned)
            .unwrap_or_default();
        // Include the pid so two running instances cannot write the same part.
        name.push(format!(".{}.part", std::process::id()));
        final_path.with_file_name(name)
    }

    /// Atomically move a finished download into place, drop older encodings of
    /// the same page and enforce the cache size limit.
    pub fn commit_audio(&self, part: &Path, bvid: &str, cid: i64, final_path: &Path) -> Result<()> {
        std::fs::rename(part, final_path).context("提交音频缓存失败")?;
        self.remove_other_audio(bvid, cid, final_path);
        self.evict(Some(final_path));
        Ok(())
    }

    pub fn get_cover(&self, url: &str) -> Option<Vec<u8>> {
        if !self.metadata_enabled() {
            return None;
        }
        let path = self.cover_path(url);
        let bytes = std::fs::read(&path).ok()?;
        if bytes.is_empty() {
            return None;
        }
        touch(&path);
        Some(bytes)
    }

    pub fn put_cover(&self, url: &str, bytes: &[u8]) {
        if !self.metadata_enabled() {
            return;
        }
        let path = self.cover_path(url);
        if write_atomic(&path, bytes).is_ok() {
            self.evict(Some(&path));
        }
    }

    pub fn get_lyrics(&self, bvid: &str, cid: i64) -> Option<Vec<LyricLine>> {
        if !self.metadata_enabled() {
            return None;
        }
        let path = self.lyrics_path(bvid, cid);
        let bytes = std::fs::read(&path).ok()?;
        let cached = serde_json::from_slice::<CachedLyrics>(&bytes).ok()?;
        if cached.version != LYRICS_VERSION || cached.lines.is_empty() {
            return None;
        }
        touch(&path);
        Some(cached.lines)
    }

    /// Empty lyric lists are intentionally not stored: subtitles often become
    /// available only after login, and a cached empty result would hide them.
    pub fn put_lyrics(&self, bvid: &str, cid: i64, lines: &[LyricLine]) {
        if !self.metadata_enabled() || lines.is_empty() {
            return;
        }
        let path = self.lyrics_path(bvid, cid);
        let cached = CachedLyrics {
            version: LYRICS_VERSION,
            lines: lines.to_vec(),
        };
        if let Ok(bytes) = serde_json::to_vec(&cached)
            && write_atomic(&path, &bytes).is_ok()
        {
            self.evict(Some(&path));
        }
    }

    /// Drop cached lyrics for a page. Used by the manual "re-fetch lyrics"
    /// action, which must bypass the cache.
    pub fn remove_lyrics(&self, bvid: &str, cid: i64) {
        if !self.metadata_enabled() {
            return;
        }
        let _ = std::fs::remove_file(self.lyrics_path(bvid, cid));
    }

    /// Metadata at most a day old. Returns `None` when the entry is missing or
    /// stale.
    pub fn get_view(&self, bvid: &str) -> Option<VideoDetail> {
        self.load_view(bvid, Some(VIEW_TTL))
    }

    /// Metadata of any age; used as an offline/risk-control fallback.
    pub fn get_stale_view(&self, bvid: &str) -> Option<VideoDetail> {
        self.load_view(bvid, None)
    }

    pub fn put_view(&self, bvid: &str, detail: &VideoDetail) {
        if !self.metadata_enabled() {
            return;
        }
        let path = self.view_path(bvid);
        let cached = CachedView {
            fetched_at: unix_secs(),
            detail: detail.clone(),
        };
        if let Ok(bytes) = serde_json::to_vec(&cached)
            && write_atomic(&path, &bytes).is_ok()
        {
            self.evict(Some(&path));
        }
    }

    /// Claim a background download for `bvid:cid`. Returns false when caching is
    /// disabled or a download for the same page is already running.
    pub fn begin_download(&self, key: &str) -> bool {
        if !self.audio_enabled() {
            return false;
        }
        self.inflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(key.to_string())
    }

    pub fn end_download(&self, key: &str) {
        self.inflight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(key);
    }

    fn load_view(&self, bvid: &str, max_age: Option<Duration>) -> Option<VideoDetail> {
        if !self.metadata_enabled() {
            return None;
        }
        let path = self.view_path(bvid);
        let bytes = std::fs::read(&path).ok()?;
        let cached = serde_json::from_slice::<CachedView>(&bytes).ok()?;
        if let Some(max_age) = max_age
            && unix_secs().saturating_sub(cached.fetched_at) > max_age.as_secs()
        {
            return None;
        }
        touch(&path);
        Some(cached.detail)
    }

    fn cover_path(&self, url: &str) -> PathBuf {
        let digest = format!("{:x}", md5::compute(url.as_bytes()));
        self.root.join("covers").join(format!("{digest}.img"))
    }

    fn lyrics_path(&self, bvid: &str, cid: i64) -> PathBuf {
        self.root.join("lyrics").join(format!("{bvid}_{cid}.json"))
    }

    fn view_path(&self, bvid: &str) -> PathBuf {
        self.root.join("view").join(format!("{bvid}.json"))
    }

    fn remove_other_audio(&self, bvid: &str, cid: i64, keep: &Path) {
        let Ok(entries) = std::fs::read_dir(self.root.join("audio")) else {
            return;
        };
        let prefix = format!("{bvid}_{cid}_");
        for entry in entries.flatten() {
            let path = entry.path();
            if path == keep {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name.starts_with(&prefix) && name.ends_with(".m4s") {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    /// Drop least recently used files until the cache is back under 90% of the
    /// configured limit. `keep` is the file that was just written.
    fn evict(&self, keep: Option<&Path>) {
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

    fn cleanup_stale_parts(&self) {
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

struct CacheFile {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

/// Top-level cache folder a file belongs to (`audio`, `covers`, ...).
fn category_of<'a>(root: &Path, path: &'a Path) -> Option<&'a str> {
    path.strip_prefix(root)
        .ok()?
        .components()
        .next()?
        .as_os_str()
        .to_str()
}

fn collect_files(dir: &Path, out: &mut Vec<CacheFile>) {
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

fn audio_file_name(bvid: &str, cid: i64, quality_id: i64, bandwidth: i64) -> String {
    format!("{bvid}_{cid}_{quality_id}_{bandwidth}.m4s")
}

/// Parse `{bvid}_{cid}_{quality}_{bandwidth}.m4s` into its components.
fn parse_audio_stem(name: &str) -> Option<(&str, i64, i64, i64)> {
    let stem = name.strip_suffix(".m4s")?;
    let mut parts = stem.split('_');
    let bvid = parts.next()?;
    let cid = parts.next()?.parse().ok()?;
    let quality_id = parts.next()?.parse().ok()?;
    let bandwidth = parts.next()?.parse().ok()?;
    if bvid.is_empty() || parts.next().is_some() {
        return None;
    }
    Some((bvid, cid, quality_id, bandwidth))
}

fn parse_audio_file_name(name: &str, bvid: &str, cid: i64) -> Option<(i64, i64)> {
    let rest = name
        .strip_prefix(&format!("{bvid}_{cid}_"))?
        .strip_suffix(".m4s")?;
    let mut parts = rest.split('_');
    let quality_id = parts.next()?.parse().ok()?;
    let bandwidth = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((quality_id, bandwidth))
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn touch(path: &Path) {
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("缓存路径缺少父目录")?;
    std::fs::create_dir_all(parent).context("创建缓存目录失败")?;
    let suffix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("tmp.{}.{suffix}", std::process::id()));
    std::fs::write(&tmp, bytes).context("写入缓存临时文件失败")?;
    std::fs::rename(&tmp, path).context("提交缓存文件失败")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_file_names_roundtrip() {
        let name = audio_file_name("BV1xx411c7mD", 12345, 30280, 204800);
        assert_eq!(
            parse_audio_file_name(&name, "BV1xx411c7mD", 12345),
            Some((30280, 204800))
        );
    }

    #[test]
    fn malformed_audio_file_names_are_rejected() {
        assert_eq!(
            parse_audio_file_name("BV1xx411c7mD_1_30280_204800.m4s.part", "BV1xx411c7mD", 1),
            None
        );
        assert_eq!(
            parse_audio_file_name("BV1xx411c7mD_2_30280_204800.m4s", "BV1xx411c7mD", 1),
            None
        );
        assert_eq!(
            parse_audio_file_name("BV1xx411c7mD_1_30280_x.m4s", "BV1xx411c7mD", 1),
            None
        );
    }

    fn test_cache() -> (Cache, PathBuf) {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "bvox-cache-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let cache = Cache::new(root.clone(), 1, CacheMode::Auto);
        (cache, root)
    }

    #[test]
    fn stats_group_files_and_clear_keeps_active_audio() {
        let (cache, root) = test_cache();
        let audio = root.join("audio/BV1_1_30280_204800.m4s");
        let part = root.join("audio/BV2_2_30280_204800.m4s.42.part");
        let cover = root.join("covers/cover.img");
        let lyrics = root.join("lyrics/BV1_1.json");
        let view = root.join("view/BV1.json");
        for (path, size) in [
            (&audio, 10),
            (&part, 20),
            (&cover, 30),
            (&lyrics, 40),
            (&view, 50),
        ] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, vec![0u8; size]).unwrap();
        }

        let stats = cache.stats();
        assert_eq!(stats.total_bytes, 150);
        assert_eq!(stats.audio_bytes, 30, "completed file + in-flight part");
        assert_eq!(stats.cover_bytes, 30);
        assert_eq!(stats.lyrics_bytes, 40);
        assert_eq!(stats.view_bytes, 50);
        assert_eq!(stats.file_count, 5);

        // The playing track and a running download must survive a cleanup.
        let freed = cache.clear(Some(&audio));
        assert_eq!(freed, 120);
        assert!(audio.exists());
        assert!(part.exists());
        assert!(!cover.exists() && !lyrics.exists() && !view.exists());

        let stats = cache.stats();
        assert_eq!(stats.total_bytes, 30);
        assert_eq!(stats.audio_bytes, 30);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cached_bvids_lists_completed_audio_only() {
        let (cache, root) = test_cache();
        let audio = root.join("audio/BV1_1_30280_204800.m4s");
        let other_page = root.join("audio/BV2_9_30280_204800.m4s");
        let part = root.join("audio/BV3_3_30280_204800.m4s.42.part");
        let malformed = root.join("audio/not-a-track.m4s");
        for path in [&audio, &other_page, &part, &malformed] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"m4s").unwrap();
        }

        let bvids = cache.cached_bvids();
        assert!(bvids.contains("BV1"));
        assert!(bvids.contains("BV2"));
        assert!(!bvids.contains("BV3"));
        assert!(!bvids.contains("not-a-track"));
        assert_eq!(bvids.len(), 2);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cached_tracks_join_audio_with_view_metadata() {
        let (cache, root) = test_cache();
        let audio = root.join("audio/BV1_1_30280_204800.m4s");
        std::fs::create_dir_all(audio.parent().unwrap()).unwrap();
        std::fs::write(&audio, b"m4s").unwrap();

        // Without view metadata the file cannot be mapped to a page.
        assert!(cache.cached_tracks().is_empty());

        cache.put_view(
            "BV1",
            &VideoDetail {
                bvid: "BV1".to_string(),
                aid: 7,
                cid: 1,
                title: "<em>歌</em>".to_string(),
                author: "UP".to_string(),
                duration: 120,
                pages: vec![crate::api::VideoPage {
                    cid: 1,
                    duration: 120,
                    part: "P1".to_string(),
                }],
                pic: Some("https://example.com/a.jpg".to_string()),
            },
        );

        let tracks = cache.cached_tracks();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].bvid, "BV1");
        assert_eq!(tracks[0].page_index, 0);
        assert_eq!(tracks[0].title, "歌");
        assert_eq!(tracks[0].author, "UP");
        assert_eq!(tracks[0].duration, "2:00");
        assert_eq!(tracks[0].part, "P1");
        assert_eq!(tracks[0].quality, "192K · 204 kbps");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn lowering_the_limit_evicts_files_immediately() {
        let (cache, root) = test_cache();
        for index in 0..3 {
            let path = root.join(format!("covers/{index}.img"));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, vec![0u8; 500 * 1024]).unwrap();
        }
        assert_eq!(cache.stats().total_bytes, 3 * 500 * 1024);

        // Files written behind the cache's back still count once the user
        // lowers the cap through the UI; 1 MiB trims to 90% (921.6 KiB).
        let limit = cache.set_limit_mb(1, None);
        assert_eq!(limit, 1024 * 1024);
        let stats = cache.stats();
        assert_eq!(stats.limit_bytes, 1024 * 1024);
        assert_eq!(stats.total_bytes, 500 * 1024);
        assert_eq!(stats.file_count, 1);

        // Disabling keeps existing files so the user can clear them manually.
        cache.set_limit_mb(0, None);
        assert!(!cache.metadata_enabled());
        assert_eq!(cache.stats().total_bytes, 500 * 1024);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cache_mode_gates_audio_and_metadata_separately() {
        let (cache, root) = test_cache();
        let audio = root.join("audio/BV1_1_30280_204800.m4s");
        let cover = root.join("covers/cover.img");
        for path in [&audio, &cover] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"bytes").unwrap();
        }

        // Auto reads both categories.
        assert!(cache.audio_enabled() && cache.metadata_enabled());
        assert!(cache.find_audio("BV1", 1).is_some());
        assert!(cache.get_cover("https://i0.hdslb.com/a.jpg").is_none());
        cache.put_cover("https://i0.hdslb.com/a.jpg", b"image-bytes");
        assert!(cache.get_cover("https://i0.hdslb.com/a.jpg").is_some());

        // Small files: audio is neither read nor written, metadata still is.
        cache.set_mode(CacheMode::SmallFiles);
        assert!(!cache.audio_enabled() && cache.metadata_enabled());
        assert!(cache.find_audio("BV1", 1).is_none());
        assert!(cache.cached_bvids().is_empty());
        assert!(!cache.begin_download("BV1:1"));
        assert!(cache.get_cover("https://i0.hdslb.com/a.jpg").is_some());
        cache.put_lyrics(
            "BV1",
            1,
            &[LyricLine {
                from: 0.0,
                to: 1.0,
                text: "ok".to_string(),
            }],
        );
        assert!(cache.get_lyrics("BV1", 1).is_some());

        // Off: nothing is read or written, existing files stay on disk.
        cache.set_mode(CacheMode::Off);
        assert!(!cache.audio_enabled() && !cache.metadata_enabled());
        assert!(cache.get_cover("https://i0.hdslb.com/a.jpg").is_none());
        assert!(cache.get_lyrics("BV1", 1).is_none());
        cache.put_view(
            "BV1",
            &VideoDetail {
                bvid: "BV1".to_string(),
                aid: 1,
                cid: 1,
                title: "标题".to_string(),
                author: "UP".to_string(),
                duration: 60,
                pages: Vec::new(),
                pic: None,
            },
        );
        assert!(cache.get_view("BV1").is_none());
        assert!(audio.exists() && cover.exists());

        // Cycling is what the frontends use for their one-key switch.
        assert_eq!(CacheMode::Auto.next(), CacheMode::SmallFiles);
        assert_eq!(CacheMode::SmallFiles.next(), CacheMode::Off);
        assert_eq!(CacheMode::Off.next(), CacheMode::Auto);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn byte_sizes_are_human_readable() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.0 MB");
        assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn audio_commit_and_lookup() {
        let (cache, root) = test_cache();
        let target = cache.audio_target("BV1xx411c7mD", 7, 30280, 204800);
        let part = cache.audio_part(&target);
        std::fs::create_dir_all(part.parent().unwrap()).unwrap();
        std::fs::write(&part, b"m4s-bytes").unwrap();

        cache
            .commit_audio(&part, "BV1xx411c7mD", 7, &target)
            .unwrap();

        let cached = cache.find_audio("BV1xx411c7mD", 7).unwrap();
        assert_eq!(cached.quality_id, 30280);
        assert_eq!(cached.bandwidth, 204800);
        assert_eq!(std::fs::read(&cached.path).unwrap(), b"m4s-bytes");
        assert!(!part.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn lyrics_and_cover_roundtrip() {
        let (cache, root) = test_cache();
        let lines = vec![LyricLine {
            from: 1.5,
            to: 3.0,
            text: "第一句".to_string(),
        }];
        cache.put_lyrics("BV1xx411c7mD", 42, &lines);
        let loaded = cache.get_lyrics("BV1xx411c7mD", 42).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, "第一句");

        // Empty lyrics are deliberately not cached.
        cache.put_lyrics("BV1xx411c7mD", 43, &[]);
        assert!(cache.get_lyrics("BV1xx411c7mD", 43).is_none());

        // A manual refresh drops the cached copy before re-fetching.
        cache.remove_lyrics("BV1xx411c7mD", 42);
        assert!(cache.get_lyrics("BV1xx411c7mD", 42).is_none());

        let cover_url = "https://i0.hdslb.com/a.jpg@640w_360h.jpg";
        cache.put_cover(cover_url, b"image-bytes");
        assert_eq!(
            cache.get_cover(cover_url).as_deref(),
            Some(b"image-bytes".as_slice())
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_lyrics_cache_is_ignored() {
        let (cache, root) = test_cache();
        // Version 1 stored a bare array; those files may hold another video's
        // subtitle and must not be served after the version bump.
        let path = root.join("lyrics").join("BV1xx411c7mD_42.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, br#"[{"from":0.0,"to":1.0,"text":"wrong song"}]"#).unwrap();
        assert!(cache.get_lyrics("BV1xx411c7mD", 42).is_none());

        cache.put_lyrics(
            "BV1xx411c7mD",
            42,
            &[LyricLine {
                from: 0.0,
                to: 1.0,
                text: "ok".to_string(),
            }],
        );
        assert_eq!(cache.get_lyrics("BV1xx411c7mD", 42).unwrap()[0].text, "ok");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn view_cache_respects_ttl_with_stale_fallback() {
        let (cache, root) = test_cache();
        let detail = VideoDetail {
            bvid: "BV1xx411c7mD".to_string(),
            aid: 1,
            cid: 42,
            title: "标题".to_string(),
            author: "UP".to_string(),
            duration: 60,
            pages: vec![crate::api::VideoPage {
                cid: 42,
                duration: 60,
                part: "P1".to_string(),
            }],
            pic: None,
        };
        cache.put_view("BV1xx411c7mD", &detail);

        let fresh = cache.get_view("BV1xx411c7mD").unwrap();
        assert_eq!(fresh.cid, 42);
        assert_eq!(fresh.title, "标题");

        // Age the entry past the 24h TTL.
        let path = root.join("view/BV1xx411c7mD.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["fetched_at"] = serde_json::json!(0);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        assert!(cache.get_view("BV1xx411c7mD").is_none());
        assert!(cache.get_stale_view("BV1xx411c7mD").is_some());
        let _ = std::fs::remove_dir_all(root);
    }
}
