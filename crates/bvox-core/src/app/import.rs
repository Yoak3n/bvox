//! BV import state: parked imports and result handling.

use super::App;
use super::{BvImport, PendingImport};

impl App {
    /// Take the parked import, if any. Used when a playlist was just created.
    pub fn take_pending_import(&mut self) -> Option<PendingImport> {
        self.pending_import.take()
    }

    /// Drop the parked import without adding it anywhere.
    pub fn cancel_pending_import(&mut self) {
        self.pending_import = None;
    }

    pub fn on_bv_imported(&mut self, result: Result<BvImport, String>) {
        match result {
            Ok(import) => {
                let bvid = import.item.bvid.clone();
                let title = import.item.title.clone();
                // Park the track when there is nowhere to put it yet. The
                // frontends immediately prompt for a new playlist; logged-out
                // imports cannot create one, so they stay local silently.
                self.pending_import =
                    (import.playlist.is_none() && import.logged_in).then(|| PendingImport {
                        bvid: bvid.clone(),
                        title: title.clone(),
                    });
                if let Some(folder) = import.playlist {
                    if let Some(entry) = self
                        .playlists
                        .iter_mut()
                        .find(|entry| entry.id == folder.id)
                    {
                        entry.count += 1;
                    }
                    if let Some(entry) = self
                        .current_playlist
                        .as_mut()
                        .filter(|entry| entry.id == folder.id)
                    {
                        entry.count += 1;
                    }
                }
                if let Some(index) = self.results.iter().position(|item| item.bvid == bvid) {
                    self.select(index);
                } else {
                    self.results.insert(0, import.item);
                    if let Some(current) = self.current {
                        self.current = Some(current + 1);
                    }
                    self.select(0);
                    self.reset_shuffle();
                }
                self.set_status(
                    &format!("已导入「{title}」 · {}", import.message),
                    import.error,
                );
            }
            Err(error) => {
                self.set_status(&format!("BV 导入失败: {error}"), true);
            }
        }
    }
}
