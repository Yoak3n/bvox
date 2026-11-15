//! Playlist library state: folder list, selection and hidden entries.

use super::App;
use super::MainView;
use crate::api::{FavoriteFolder, VideoItem};

impl App {
    /// Install the last cached folder list (offline start / refresh fallback).
    /// No status change: the caller decides whether this is a fresh start or
    /// an offline fallback.
    pub fn load_cached_playlists(&mut self, playlists: Vec<FavoriteFolder>, hidden: Vec<i64>) {
        self.playlists = playlists;
        self.hidden_playlists = hidden.into_iter().collect();
        self.playlists_loading = false;
        self.fix_playlist_selection();
    }

    pub fn open_library(&mut self) {
        self.view = MainView::Library;
        self.picker = None;
        self.rename_target = None;
        self.fix_playlist_selection();
    }

    pub fn open_queue(&mut self) {
        self.view = MainView::Queue;
        self.picker = None;
        self.rename_target = None;
    }

    /// Start a folder-list request and return its sequence id. Results from
    /// older requests are ignored, which keeps account switches safe.
    pub fn begin_playlists_load(&mut self) -> u64 {
        self.playlists_loading = true;
        self.playlists_seq += 1;
        self.set_status("正在加载歌单…", false);
        self.playlists_seq
    }

    /// Start a playlist-open request and return its sequence id.
    pub fn begin_playlist_open(&mut self) -> u64 {
        self.playlist_open_seq += 1;
        self.playlist_open_seq
    }

    /// Drop in-flight results (logout / account switch).
    pub fn invalidate_playlist_requests(&mut self) {
        self.playlists_seq += 1;
        self.playlist_open_seq += 1;
        self.playlists_loading = false;
    }

    pub(super) fn fix_playlist_selection(&mut self) {
        let len = self.library_playlists().len();
        if len == 0 {
            self.playlist_selected = None;
        } else {
            let index = self.playlist_selected.unwrap_or(0).min(len - 1);
            self.playlist_selected = Some(index);
        }
    }

    /// The folders the library currently renders: hidden ones are omitted
    /// unless the user asked to reveal them.
    pub fn library_playlists(&self) -> Vec<FavoriteFolder> {
        self.playlists
            .iter()
            .filter(|folder| self.show_hidden || !self.is_hidden(folder.id))
            .cloned()
            .collect()
    }

    pub fn is_hidden(&self, folder_id: i64) -> bool {
        self.hidden_playlists.contains(&folder_id)
    }

    pub fn apply_playlist_hidden(&mut self, folder_id: i64, hidden: bool) {
        let selected = self.selected_playlist().map(|folder| folder.id);
        if hidden {
            self.hidden_playlists.insert(folder_id);
        } else {
            self.hidden_playlists.remove(&folder_id);
        }
        self.reselect_playlist(selected);
    }

    pub fn toggle_show_hidden(&mut self) {
        let selected = self.selected_playlist().map(|folder| folder.id);
        self.show_hidden = !self.show_hidden;
        self.reselect_playlist(selected);
    }

    /// Keep the cursor on the same folder across hide/unhide changes, falling
    /// back to the clamped index when that folder is no longer visible.
    fn reselect_playlist(&mut self, folder_id: Option<i64>) {
        if let Some(folder_id) = folder_id
            && let Some(index) = self
                .library_playlists()
                .iter()
                .position(|folder| folder.id == folder_id)
        {
            self.playlist_selected = Some(index);
            return;
        }
        self.fix_playlist_selection();
    }

    pub fn move_playlist_selection(&mut self, delta: isize) {
        let len = self.library_playlists().len();
        if len == 0 {
            return;
        }
        let current = self.playlist_selected.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, len as isize - 1) as usize;
        self.playlist_selected = Some(next);
    }

    pub fn selected_playlist(&self) -> Option<FavoriteFolder> {
        let playlists = self.library_playlists();
        playlists.get(self.playlist_selected?).cloned()
    }

    pub fn on_playlists_loaded(
        &mut self,
        seq: u64,
        result: Result<(Vec<FavoriteFolder>, Vec<i64>), String>,
    ) {
        if seq != self.playlists_seq {
            return;
        }
        self.playlists_loading = false;
        match result {
            Ok((playlists, hidden)) => {
                let selected = self.selected_playlist().map(|folder| folder.id);
                self.playlists = playlists;
                self.hidden_playlists = hidden.into_iter().collect();
                self.reselect_playlist(selected);
                // Keep the picker's own prompt while it is open; the list
                // renders the folders as soon as this response lands.
                if self.picker.is_none() {
                    let visible = self.library_playlists().len();
                    self.set_status(&format!("歌单 {visible} 个"), false);
                }
            }
            Err(error) => {
                if self.playlists.is_empty() {
                    self.set_status(&format!("歌单加载失败: {error}"), true);
                } else {
                    // Keep the cached list usable when the network is gone.
                    self.set_status("离线：显示缓存的歌单", false);
                }
            }
        }
    }

    pub fn on_playlist_hidden(&mut self, folder_id: i64, hidden: bool, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.apply_playlist_hidden(folder_id, hidden);
                let title = self
                    .playlists
                    .iter()
                    .find(|folder| folder.id == folder_id)
                    .map(|folder| folder.title.clone())
                    .unwrap_or_default();
                if hidden {
                    self.set_status(&format!("已隐藏「{title}」"), false);
                } else {
                    self.set_status(&format!("已恢复「{title}」"), false);
                }
            }
            Err(error) => self.set_status(&format!("保存隐藏状态失败: {error}"), true),
        }
    }

    pub fn on_playlist_opened(
        &mut self,
        seq: u64,
        folder: FavoriteFolder,
        result: Result<Vec<VideoItem>, String>,
        offline: bool,
    ) {
        if seq != self.playlist_open_seq {
            return;
        }
        match result {
            Ok(items) => {
                let count = items.len();
                let title = folder.title.clone();
                self.replace_results(items, &format!("歌单 · {title}"), Some(folder));
                self.view = MainView::Queue;
                let suffix = if offline { "（离线缓存）" } else { "" };
                self.set_status(&format!("已打开「{title}」{count} 首{suffix}"), false);
            }
            Err(error) => self.set_status(&format!("打开歌单失败: {error}"), true),
        }
    }
}
