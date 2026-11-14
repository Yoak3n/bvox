//! Add-to-playlist picker state.

use super::App;
use super::PlaylistPicker;
use crate::api::FavoriteFolder;

impl App {
    /// Open the add-to-playlist picker. Returns false when the folder list is
    /// still empty, in which case the caller should load it first.
    /// Open the add-to-playlist picker. The folder list may still be empty;
    /// the session refreshes it and the picker fills in when the response
    /// lands. A parked import adds a final "new playlist" row.
    pub fn open_picker(&mut self, bvid: String) {
        self.picker = Some(PlaylistPicker {
            target_bvid: bvid,
            selected: Some(0),
        });
        self.set_status("选择要加入的歌单，或新建一个", false);
    }

    pub fn move_picker(&mut self, delta: isize) {
        let extra = usize::from(self.pending_import.is_some());
        let len = self.library_playlists().len() + extra;
        let Some(picker) = &mut self.picker else {
            return;
        };
        if len == 0 {
            picker.selected = None;
            return;
        }
        let current = picker.selected.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, len as isize - 1) as usize;
        picker.selected = Some(next);
    }

    pub fn picker_folder(&self) -> Option<FavoriteFolder> {
        let picker = self.picker.as_ref()?;
        self.library_playlists().get(picker.selected?).cloned()
    }

    pub fn close_picker(&mut self) {
        self.picker = None;
        self.set_status("已取消加入歌单", false);
    }
}
