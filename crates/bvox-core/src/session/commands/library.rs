//! Library commands: favorite folders and the tracks inside them.

use crate::api::FavoriteFolder;
use crate::session::Session;
use crate::session::msg::Msg;
use crate::session::tasks::library::{
    spawn_create_playlist, spawn_delete_playlist, spawn_import_public_playlist,
    spawn_open_playlist, spawn_playlists, spawn_rename_playlist, spawn_set_playlist_hidden,
};
use crate::session::tasks::tracks::{spawn_add_track, spawn_remove_track};

impl Session {
    /// Show the playlist library, loading folders when possible.
    pub async fn show_library(&mut self) {
        self.app.open_library();
        if !self.api.has_cookie().await {
            self.app.invalidate_playlist_requests();
            self.app.playlists.clear();
            self.app.set_status("登录后管理歌单", false);
            self.save_playback();
            return;
        }
        self.refresh_playlists();
        self.save_playback();
    }

    pub fn show_queue(&mut self) {
        self.app.open_queue();
        self.save_playback();
    }

    pub fn refresh_playlists(&mut self) {
        if self.app.playlists_loading {
            return;
        }
        let seq = self.app.begin_playlists_load();
        spawn_playlists(&self.api, self.tx.clone(), seq);
    }

    pub fn open_playlist(&mut self, folder: FavoriteFolder) {
        self.app
            .set_status(&format!("正在打开「{}」…", folder.title), false);
        let seq = self.app.begin_playlist_open();
        spawn_open_playlist(&self.api, self.tx.clone(), seq, folder);
    }

    /// Load another user's public folder as a read-only queue. Accepts a
    /// media id, a full folder link, or a b23.tv short link.
    pub fn import_public_playlist(&mut self, input: String) {
        self.app.set_status("正在导入公开收藏夹…", false);
        let seq = self.app.begin_playlist_open();
        spawn_import_public_playlist(&self.api, self.tx.clone(), seq, input);
    }

    pub fn create_playlist(&mut self, title: String) {
        let pending = self.app.take_pending_import();
        spawn_create_playlist(&self.api, self.tx.clone(), title, pending);
    }

    pub fn rename_playlist(&self, folder_id: i64, title: String) {
        spawn_rename_playlist(&self.api, self.tx.clone(), folder_id, title);
    }

    pub fn delete_playlist(&self, folder_id: i64) {
        spawn_delete_playlist(&self.api, self.tx.clone(), folder_id);
    }

    /// Hide or unhide a favorite folder. The hidden list is persisted per
    /// account.
    pub fn set_playlist_hidden(&mut self, folder_id: i64, hidden: bool) {
        self.app.set_status(
            if hidden {
                "正在隐藏歌单…"
            } else {
                "正在恢复歌单…"
            },
            false,
        );
        let _ = self.tx.send(Msg::StateChanged);
        spawn_set_playlist_hidden(&self.api, self.tx.clone(), folder_id, hidden);
    }

    /// Temporarily reveal or collapse hidden folders in the library.
    pub fn toggle_show_hidden(&mut self) {
        self.app.toggle_show_hidden();
        let _ = self.tx.send(Msg::StateChanged);
    }

    /// Open the add-to-playlist picker, loading folders first if needed.
    pub fn begin_add_track(&mut self, bvid: String) {
        self.app.open_picker(bvid);
        if self.app.playlists.is_empty() && !self.app.playlists_loading {
            self.refresh_playlists();
        }
    }

    pub fn add_track_to_playlist(&mut self, folder_id: i64, bvid: String) {
        // Picking a folder from the import prompt is the user's choice; the
        // parked import has now been placed.
        self.app.cancel_pending_import();
        let item = self
            .app
            .results
            .iter()
            .find(|item| item.bvid == bvid)
            .cloned();
        spawn_add_track(&self.api, self.tx.clone(), folder_id, bvid, item);
    }

    pub fn remove_track_from_playlist(&self, folder_id: i64, bvid: String) {
        let item = self
            .app
            .results
            .iter()
            .find(|item| item.bvid == bvid)
            .cloned();
        spawn_remove_track(&self.api, self.tx.clone(), folder_id, bvid, item);
    }
}
