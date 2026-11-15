//! Audio cache: lookup, commit and per-track bookkeeping.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};

use super::Cache;
use super::model::{CachedAudio, CachedTrack};
use super::paths::touch;

impl Cache {
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
                        title: crate::title::clean_title(&detail.title),
                        author: detail.author.clone(),
                        duration: crate::format_secs(duration),
                        pic: detail.pic.clone(),
                        quality: format!(
                            "{} · {} kbps",
                            crate::format::quality_label(quality_id),
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
}
pub(super) fn audio_file_name(bvid: &str, cid: i64, quality_id: i64, bandwidth: i64) -> String {
    format!("{bvid}_{cid}_{quality_id}_{bandwidth}.m4s")
}

/// Parse `{bvid}_{cid}_{quality}_{bandwidth}.m4s` into its components.
pub(super) fn parse_audio_stem(name: &str) -> Option<(&str, i64, i64, i64)> {
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

pub(super) fn parse_audio_file_name(name: &str, bvid: &str, cid: i64) -> Option<(i64, i64)> {
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
