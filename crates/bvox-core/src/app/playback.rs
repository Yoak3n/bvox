//! Playback state: now-playing info, player state, auto-advance.

use super::App;
use super::{PlayRequest, PlayingInfo};
use crate::api::PlayMode;
use crate::duration::parse_duration;
use crate::player::PlayerState;

impl App {
    pub fn on_playing(&mut self, info: PlayingInfo) {
        let key = format!("{}#{}#{}", info.item.bvid, info.cid, info.page_index);
        self.pending_play = false;
        self.play_failures = 0;
        self.play_error = None;
        self.now_playing = Some(info.item.clone());
        self.quality = info.quality.clone();
        self.page = info.page_index;
        self.page_count = info.page_count;
        self.part = info.part;
        self.playing_pages = info.pages;
        self.current_cid = info.cid;
        self.position = info.start_position.max(0.0);
        self.duration = if info.page_duration > 0 {
            info.page_duration as f64
        } else {
            parse_duration(&info.item.duration)
        };
        self.paused = false;
        self.has_media = true;
        self.playback_started = false;
        self.eof = false;
        self.lyrics_key = key.clone();
        self.lyrics.clear();
        self.lyrics_status = "加载歌词…".to_string();
        self.cover_key = key.clone();
        self.playback_key = key;
        self.cache_progress = None;
        self.cached_now = info.cached;

        let page_info = if info.page_count > 1 {
            format!("P{}/{} · ", info.page_index + 1, info.page_count)
        } else {
            String::new()
        };
        self.set_status(&format!("正在播放 · {page_info}{}", info.quality), false);
    }

    pub fn on_play_failed(&mut self, error: String) {
        self.pending_play = false;
        self.has_media = false;
        self.play_error = Some(error.clone());
        self.set_status(&format!("播放失败: {error}"), true);
    }
}
impl App {
    pub fn on_player_state(&mut self, state: PlayerState) {
        self.volume = state.volume;
        if !self.has_media {
            return;
        }
        self.position = state.position;
        if state.duration > 0.0 {
            self.duration = state.duration;
        }
        self.paused = state.paused;

        if state.position > 0.5 {
            self.playback_started = true;
        }
        // At EOF mpv unloads the file and `eof-reached` may already be reset,
        // so treat "was playing, now idle" as the end of the track too.
        if state.eof || (state.idle && self.playback_started && !state.paused) {
            self.eof = true;
            self.playback_started = false;
        }
    }
}
impl App {
    /// Consume the EOF flag so auto-advance runs exactly once per track end.
    pub fn take_eof(&mut self) -> bool {
        let eof = self.eof;
        self.eof = false;
        eof
    }

    /// Auto-advance at the end of a track. Repeat-one replays the current
    /// page; a multi-page video continues with its next page; otherwise the
    /// play mode decides which queue entry comes next.
    pub fn auto_advance(&mut self) -> Option<PlayRequest> {
        if self.play_mode == PlayMode::RepeatOne {
            return self.replay_current();
        }
        if self.page + 1 < self.page_count && self.now_playing.is_some() {
            return self.step_page(1);
        }
        match self.play_mode {
            PlayMode::Shuffle => self.step_shuffle(1),
            _ => self.step_queue(1),
        }
    }

    /// Move past a track that failed to load. Bounded by the queue length so
    /// an offline queue full of uncached tracks cannot retry forever.
    pub fn skip_failed(&mut self) -> Option<PlayRequest> {
        let queue_len = self.results.len();
        if queue_len == 0 {
            return None;
        }
        self.play_failures += 1;
        if self.play_failures >= queue_len {
            self.play_failures = 0;
            let reason = self.play_error.as_deref().unwrap_or("可能未缓存");
            self.set_status(&format!("列表中没有可播放的歌曲（{reason}）"), true);
            return None;
        }
        let request = match self.play_mode {
            PlayMode::Shuffle => self.step_shuffle(1),
            _ => self.step_queue(1),
        };
        if request.is_none() {
            self.play_failures = 0;
            self.set_status("列表已播完，没有可播放的歌曲", true);
        }
        request
    }
}
