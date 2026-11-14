//! Lyrics refresh commands.

use crate::session::Session;
use crate::session::tasks::lyrics::spawn_lyrics;

impl Session {
    /// Re-fetch lyrics for the current track, bypassing (and dropping) the
    /// disk cache. Frontends expose this when the shown lyrics look wrong.
    pub fn refresh_lyrics(&mut self) {
        let Some(item) = self.app.now_playing.clone() else {
            self.app.set_status("还没有播放内容", true);
            return;
        };
        if self.app.lyrics_key.is_empty() {
            self.app.set_status("还没有播放内容", true);
            return;
        }
        self.app.lyrics.clear();
        self.app.lyrics_status = "重新加载歌词…".to_string();
        let seq = self.next_lyrics_seq();
        spawn_lyrics(
            &self.api,
            self.tx.clone(),
            self.app.lyrics_key.clone(),
            item.aid,
            item.bvid,
            self.app.current_cid,
            seq,
            true,
        );
    }

    pub(in crate::session) fn next_lyrics_seq(&mut self) -> u64 {
        self.lyrics_seq = self.lyrics_seq.wrapping_add(1);
        self.lyrics_seq
    }
}
