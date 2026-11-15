//! BV import commands.

use crate::session::Session;
use crate::session::msg::Msg;
use crate::session::tasks::tracks::spawn_import_bv;

impl Session {
    /// Import a BV id / link. With `target_open_playlist` the currently open
    /// playlist is the destination (TUI keyboard flow); otherwise the track
    /// only parks in the queue and the frontend asks which folder to use.
    pub fn import_bv(&mut self, raw: String, target_open_playlist: bool) {
        // A public folder belongs to someone else; never write the imported
        // track into it. Leave the import parked so the frontend offers the
        // user's own playlists instead.
        let playlist = if target_open_playlist && !self.app.current_playlist_public {
            self.app.current_playlist.clone()
        } else {
            None
        };
        spawn_import_bv(&self.api, self.tx.clone(), raw, playlist);
    }

    /// Drop the import that was waiting for a new playlist.
    pub fn cancel_pending_import(&mut self) {
        if self.app.pending_import.take().is_some() {
            self.app.set_status("已保留在本地列表", false);
            let _ = self.tx.send(Msg::StateChanged);
        }
    }
}
