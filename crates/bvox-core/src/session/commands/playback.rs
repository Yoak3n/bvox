//! Playback commands: queueing, transport control and quality preferences.

use crate::api::{AudioQuality, PlayMode, VideoItem};
use crate::app::PlayRequest;
use crate::session::Session;
use crate::session::msg::Msg;
use crate::session::tasks::playback::quality_status;
use crate::session::tasks::playback::spawn_play;

impl Session {
    pub fn play(&mut self, mut request: PlayRequest) {
        // A restored track resumes at its saved position when the user starts
        // it again (play button, retry, queue row). Any other track clears the
        // pending resume so it cannot affect later playback.
        if let Some(pending) = self.pending_resume.as_ref() {
            if pending.bvid == request.item.bvid && pending.page == request.page_index {
                if request.start_at <= 0.0 {
                    request.start_at = pending.position;
                }
            } else {
                self.pending_resume = None;
            }
        }
        spawn_play(
            &self.api,
            self.player.clone(),
            self.tx.clone(),
            request,
            self.app.quality_pref,
            self.app.vip,
        );
    }

    /// Play one entry from the offline cached list and make that list the
    /// current queue, so the Playing screen and next/previous have context.
    pub fn play_cached(&mut self, bvid: String) {
        let tracks = self.api.cache().cached_tracks();
        let Some(index) = tracks.iter().position(|track| track.bvid == bvid) else {
            self.app.set_status("缓存文件不存在，请重新在线播放", true);
            let _ = self.tx.send(Msg::StateChanged);
            return;
        };
        let target = tracks[index].clone();
        let items = tracks
            .iter()
            .map(|track| VideoItem {
                aid: track.aid,
                bvid: track.bvid.clone(),
                title: track.title.clone(),
                author: track.author.clone(),
                duration: track.duration.clone(),
                pic: track.pic.clone(),
            })
            .collect::<Vec<_>>();
        self.app.show_cached_tracks(items);
        let _ = self.tx.send(Msg::StateChanged);
        if let Some(mut request) = self.app.play_at(index) {
            request.page_index = target.page_index as usize;
            self.play(request);
        }
    }

    pub async fn pause(&mut self) {
        // Paused restores leave no media loaded, so the play button must
        // start the saved track rather than toggle an empty player.
        if let Some(request) = self.pending_resume_request() {
            self.app.pending_play = true;
            self.app.set_status("正在恢复上次播放…", false);
            self.play(request);
            return;
        }
        if !self.player.is_started().await {
            self.app.paused = !self.app.paused;
            return;
        }
        if let Err(error) = self.player.toggle_pause().await {
            self.app.paused = !self.app.paused;
            self.app.set_error(error);
        }
    }

    pub async fn seek(&mut self, delta: f64) {
        if self.player.is_started().await
            && let Err(error) = self.player.seek(delta).await
        {
            self.app.set_error(error);
        }
    }

    pub async fn seek_to(&mut self, seconds: f64) {
        if self.player.is_started().await
            && let Err(error) = self.player.seek_to(seconds).await
        {
            self.app.set_error(error);
        }
    }

    pub async fn volume(&mut self, value: f64) {
        if self.player.is_started().await
            && let Err(error) = self.player.set_volume(value).await
        {
            self.app.set_error(error);
        }
    }

    /// Change the audio track preference and persist it for the next start.
    pub fn set_audio_quality(&mut self, quality: AudioQuality) {
        self.app.quality_pref = quality;
        self.api.save_audio_quality(quality);
        self.app
            .set_status(&quality_status(quality, self.app.vip), false);
        let _ = self.tx.send(Msg::StateChanged);
    }

    /// Change the playback order and persist it for the next start.
    pub fn set_play_mode(&mut self, mode: PlayMode) {
        self.app.play_mode = mode;
        if mode == PlayMode::Shuffle {
            self.app.reset_shuffle();
        }
        self.api.save_play_mode(mode);
        self.app
            .set_status(&format!("播放模式：{}", mode.label()), false);
        let _ = self.tx.send(Msg::StateChanged);
    }

    /// Advance to the previous/next track (TUI's `n`/`p`).
    pub fn step_track(&mut self, delta: isize) {
        if let Some(request) = self.app.step(delta) {
            self.play(request);
        }
    }

    /// Re-resolve the stream for the current track (TUI's `r`).
    pub fn retry_current(&mut self) {
        if self.app.pending_play {
            return;
        }
        let item = self.app.now_playing.clone().or_else(|| {
            self.app
                .current
                .and_then(|index| self.app.results.get(index))
                .cloned()
        });
        if let Some(item) = item {
            self.app.pending_play = true;
            self.app.set_status("重新获取播放地址…", false);
            self.play(PlayRequest {
                item,
                page_index: self.app.page,
                start_at: 0.0,
            });
        }
    }

    /// Jump straight to one 分P of the currently playing video (page picker).
    pub fn play_page(&mut self, page: usize) {
        if self.app.pending_play {
            return;
        }
        let Some(item) = self.app.now_playing.clone() else {
            self.app.set_status("还没有播放内容", true);
            return;
        };
        if page >= self.app.playing_pages.len() {
            self.app.set_status("没有这个分P", true);
            return;
        }
        if page == self.app.page {
            return;
        }
        self.app.pending_play = true;
        self.app.page = page;
        self.app.set_status("准备播放…", false);
        self.play(PlayRequest {
            item,
            page_index: page,
            start_at: 0.0,
        });
    }
}
