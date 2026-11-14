//! BV import commands.

use crate::session::Session;
use crate::session::msg::Msg;

impl Session {
    /// Drop the import that was waiting for a new playlist.
    pub fn cancel_pending_import(&mut self) {
        if self.app.pending_import.take().is_some() {
            self.app.set_status("已保留在本地列表", false);
            let _ = self.tx.send(Msg::StateChanged);
        }
    }
}
