//! Cache commands: strategy, limit, stats refresh and cleanup.

use std::path::PathBuf;

use crate::cache::CacheMode;
use crate::session::Session;
use crate::session::msg::Msg;

impl Session {
    /// Recompute the cache usage for the UI. Cheap enough to call whenever a
    /// cover/lyrics/audio write may have changed it.
    pub(in crate::session) fn update_cache_stats(&mut self) -> bool {
        let stats = self.api.cache().stats();
        if self.app.cache_stats == Some(stats) {
            return false;
        }
        self.app.cache_stats = Some(stats);
        true
    }

    /// Cached file of the track currently playing; cleanup must keep it so
    /// playback (and a pending seek) keeps working.
    fn playing_cached_audio(&self) -> Option<PathBuf> {
        if !self.app.cached_now {
            return None;
        }
        let item = self.app.now_playing.as_ref()?;
        self.api
            .cache()
            .find_audio(&item.bvid, self.app.current_cid)
            .map(|cached| cached.path)
    }

    /// Switch what the cache stores. Existing files are kept; turning audio
    /// caching back on makes them playable again without a re-download.
    pub fn set_cache_mode(&mut self, mode: CacheMode) {
        self.api.cache().set_mode(mode);
        self.app.cache_mode = mode;
        self.update_cache_stats();
        self.refresh_cached_tracks();
        self.app.set_status(
            &format!("缓存策略：{}（{}）", mode.label(), mode.description()),
            false,
        );
        let _ = self.tx.send(Msg::StateChanged);
    }

    /// Change the cache size cap at runtime. Lowering it evicts least
    /// recently used files immediately; 0 disables further caching.
    pub fn set_cache_limit_mb(&mut self, limit_mb: u64) {
        let keep = self.playing_cached_audio();
        let limit_bytes = self.api.cache().set_limit_mb(limit_mb, keep.as_deref());
        if limit_bytes == 0 {
            // Keep the exposed mode in sync with the effective behaviour.
            self.api.cache().set_mode(CacheMode::Off);
            self.app.cache_mode = CacheMode::Off;
        }
        self.update_cache_stats();
        self.refresh_cached_tracks();
        if limit_bytes == 0 {
            self.app
                .set_status("缓存已关闭，已有文件可以手动清理", false);
        } else {
            self.app.set_status(
                &format!("缓存上限已设为 {}", crate::cache::format_bytes(limit_bytes)),
                false,
            );
        }
        let _ = self.tx.send(Msg::StateChanged);
    }

    /// Recompute the cache usage and tell the frontend when it changed.
    pub fn refresh_cache_stats(&mut self) {
        let stats_changed = self.update_cache_stats();
        let markers_changed = self.refresh_cached_tracks();
        if stats_changed || markers_changed {
            let _ = self.tx.send(Msg::StateChanged);
        }
    }

    /// Re-scan completed audio files so list markers and the offline cached
    /// list follow cache changes.
    pub(in crate::session) fn refresh_cached_tracks(&mut self) -> bool {
        let bvids = self.api.cache().cached_bvids();
        let tracks = self.api.cache().cached_tracks();
        if self.app.cached_bvids == bvids && self.app.cached_tracks == tracks {
            return false;
        }
        self.app.cached_bvids = bvids;
        self.app.cached_tracks = tracks;
        true
    }

    /// Delete cached audio, covers, lyrics and video metadata. The file of
    /// the track currently playing from cache is kept so playback (and a
    /// pending seek) keeps working.
    pub fn clear_cache(&mut self) {
        let keep = self.playing_cached_audio();
        let freed = self.api.cache().clear(keep.as_deref());
        self.update_cache_stats();
        self.refresh_cached_tracks();
        if freed > 0 {
            self.app.set_status(
                &format!("已清理缓存，释放 {}", crate::cache::format_bytes(freed)),
                false,
            );
        } else {
            self.app.set_status("缓存已经是空的", false);
        }
        let _ = self.tx.send(Msg::StateChanged);
    }

    // ----- playlist (favorite folder) commands ----------------------------
}
