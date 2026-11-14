//! Cache bookkeeping inside the UI state.

use super::App;
use crate::api::VideoItem;

impl App {
    /// True when a completed audio file for this video exists locally.
    pub fn is_cached(&self, bvid: &str) -> bool {
        self.cached_bvids.contains(bvid)
    }
}
impl App {
    pub fn on_cache_progress(&mut self, key: String, downloaded: u64, total: Option<u64>) {
        if key == self.playback_key {
            self.cache_progress = Some((downloaded, total));
        }
    }

    pub fn on_cache_done(&mut self, key: String, ok: bool) {
        if key != self.playback_key {
            return;
        }
        self.cache_progress = None;
        if ok {
            self.cached_now = true;
        }
    }
}
impl App {
    /// Replace the queue with the offline cached tracks. Playback uses the
    /// normal path so `view`/audio cache hits keep working without network.
    pub fn show_cached_tracks(&mut self, items: Vec<VideoItem>) {
        self.replace_results(items, "已缓存歌曲", None);
    }
}
