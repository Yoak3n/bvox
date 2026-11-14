//! Outcomes of playlist create/rename/delete and track add/remove.

use super::App;
use crate::api::FavoriteFolder;

impl App {
    pub fn on_playlist_created(&mut self, result: Result<FavoriteFolder, String>) {
        match result {
            Ok(folder) => {
                let id = folder.id;
                let title = folder.title.clone();
                if let Some(existing) = self.playlists.iter_mut().find(|entry| entry.id == id) {
                    *existing = folder;
                } else {
                    self.playlists.push(folder);
                }
                if let Some(index) = self
                    .library_playlists()
                    .iter()
                    .position(|entry| entry.id == id)
                {
                    self.playlist_selected = Some(index);
                }
                self.set_status(&format!("已新建歌单「{title}」"), false);
            }
            Err(error) => self.set_status(&format!("新建歌单失败: {error}"), true),
        }
    }

    pub fn on_playlist_renamed(
        &mut self,
        folder_id: i64,
        title: String,
        result: Result<(), String>,
    ) {
        match result {
            Ok(()) => {
                if let Some(folder) = self
                    .playlists
                    .iter_mut()
                    .find(|entry| entry.id == folder_id)
                {
                    folder.title = title.clone();
                }
                let current_renamed = self.current_playlist.as_mut().is_some_and(|folder| {
                    if folder.id == folder_id {
                        folder.title = title.clone();
                        true
                    } else {
                        false
                    }
                });
                if current_renamed {
                    self.results_title = format!("歌单 · {title}");
                }
                self.set_status(&format!("已重命名为「{title}」"), false);
            }
            Err(error) => self.set_status(&format!("重命名失败: {error}"), true),
        }
    }

    pub fn on_playlist_deleted(&mut self, folder_id: i64, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.playlists.retain(|folder| folder.id != folder_id);
                self.hidden_playlists.remove(&folder_id);
                self.fix_playlist_selection();
                if self
                    .current_playlist
                    .as_ref()
                    .is_some_and(|folder| folder.id == folder_id)
                {
                    // Keep the local queue (and playback) but it is no longer
                    // backed by a folder on Bilibili.
                    self.current_playlist = None;
                }
                self.set_status("歌单已删除", false);
            }
            Err(error) => self.set_status(&format!("删除歌单失败: {error}"), true),
        }
    }

    pub fn on_track_added(&mut self, folder_id: i64, changed: bool, result: Result<(), String>) {
        match result {
            Ok(()) => {
                let title = self
                    .playlists
                    .iter()
                    .find(|folder| folder.id == folder_id)
                    .map(|folder| folder.title.clone())
                    .unwrap_or_else(|| "歌单".to_string());
                if changed {
                    if let Some(folder) = self
                        .playlists
                        .iter_mut()
                        .find(|entry| entry.id == folder_id)
                    {
                        folder.count += 1;
                    }
                    if let Some(folder) = self
                        .current_playlist
                        .as_mut()
                        .filter(|folder| folder.id == folder_id)
                    {
                        folder.count += 1;
                    }
                }
                if changed {
                    self.set_status(&format!("已加入歌单「{title}」"), false);
                } else {
                    self.set_status(&format!("已在歌单「{title}」中"), false);
                }
            }
            Err(error) => self.set_status(&format!("加入歌单失败: {error}"), true),
        }
    }

    pub fn on_track_removed(
        &mut self,
        folder_id: i64,
        bvid: String,
        changed: bool,
        result: Result<(), String>,
    ) {
        match result {
            Ok(()) => {
                if changed {
                    if let Some(folder) = self
                        .playlists
                        .iter_mut()
                        .find(|entry| entry.id == folder_id)
                    {
                        folder.count = (folder.count - 1).max(0);
                    }
                    if let Some(folder) = self
                        .current_playlist
                        .as_mut()
                        .filter(|folder| folder.id == folder_id)
                    {
                        folder.count = (folder.count - 1).max(0);
                    }
                }
                if let Some(index) = self.results.iter().position(|item| item.bvid == bvid) {
                    self.remove_result(index);
                }
                if changed {
                    self.set_status("已从歌单移除", false);
                } else {
                    self.set_status("该歌曲不在歌单中", false);
                }
            }
            Err(error) => self.set_status(&format!("从歌单移除失败: {error}"), true),
        }
    }

    fn remove_result(&mut self, index: usize) {
        let selected = self.selected;
        let current = self.current;
        self.results.remove(index);

        self.selected = selected.and_then(|selected| {
            if self.results.is_empty() {
                None
            } else if selected > index {
                Some(selected - 1)
            } else {
                Some(selected.min(self.results.len() - 1))
            }
        });
        self.current = current.and_then(|current| {
            if current == index {
                // The playing track left the queue; audio keeps playing but
                // auto-advance can no longer use its index.
                None
            } else if current > index {
                Some(current - 1)
            } else {
                Some(current)
            }
        });
        self.reset_shuffle();
    }
}
