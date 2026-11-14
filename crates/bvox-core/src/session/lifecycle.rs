//! Session teardown and persistence of playback / playlist state.

use crate::resume::SavedPlayback;
use crate::session::Session;

impl Session {
    /// Stop playback entirely.
    pub async fn stop(&mut self) {
        self.player.quit().await;
        self.app.has_media = false;
        self.app.paused = false;
        self.app.set_status("已停止播放", false);
        self.save_playback();
    }

    /// Persist the queue, current track and position for the next start.
    /// Errors are deliberately ignored: a read-only config directory must not
    /// break playback.
    pub fn save_playback(&mut self) {
        let saved = SavedPlayback::capture(&self.app);
        if crate::resume::save(self.api.data_dir(), &saved).is_ok() {
            self.saved_position = self.app.position;
            self.saved_paused = self.app.paused;
            self.saved_once = true;
        }
    }

    /// Write the current folder list to the per-account offline cache.
    pub(in crate::session) fn persist_playlists(&self) {
        let api = self.api.clone();
        let folders = self.app.playlists.clone();
        tokio::spawn(async move {
            api.save_playlist_cache(&folders).await;
        });
    }
}
