//! Lyrics display state.

use super::App;
use crate::api::LyricLine;

impl App {
    pub fn on_lyrics(&mut self, key: String, result: Result<Vec<LyricLine>, String>) {
        if key != self.lyrics_key {
            return;
        }
        match result {
            Ok(lines) if lines.is_empty() => {
                self.lyrics.clear();
                self.lyrics_status = "该视频没有字幕/歌词".to_string();
            }
            Ok(lines) => {
                self.lyrics = lines;
                self.lyrics_status.clear();
            }
            Err(error) => {
                self.lyrics.clear();
                self.lyrics_status = format!("歌词加载失败: {error}");
            }
        }
    }

    pub fn lyric_cursor(&self) -> (usize, bool) {
        let time = self.position + 0.2;
        let mut cursor = 0;
        let mut found = false;
        for (index, line) in self.lyrics.iter().enumerate() {
            if line.from <= time {
                cursor = index;
                found = true;
            } else {
                break;
            }
        }
        (cursor, found)
    }

    pub fn current_lyric(&self) -> Option<&str> {
        let (index, found) = self.lyric_cursor();
        if !found {
            return None;
        }
        self.lyrics.get(index).map(|line| line.text.as_str())
    }
}
