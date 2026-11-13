//! UI-agnostic session state.
//!
//! `App` holds every piece of state a frontend renders: search results,
//! playback progress, login and favorites modals, lyrics. Frontends mutate it
//! through [`Session`](crate::session::Session) commands and read it back for
//! drawing; the field set is deliberately plain data so it maps onto Compose
//! state as easily as onto a ratatui frame.

use qrcode::types::Color as QrColor;
use qrcode::{EcLevel, QrCode};

use crate::api::{FavoriteFolder, LyricLine, UserInfo, VideoItem};
use crate::player::PlayerState;

pub struct PlayRequest {
    pub item: VideoItem,
    pub page_index: usize,
}

pub struct PlayingInfo {
    pub item: VideoItem,
    pub quality: String,
    pub page_index: usize,
    pub page_count: usize,
    pub part: String,
    pub page_duration: i64,
    pub cid: i64,
    pub pic: Option<String>,
    /// True when playback came from the on-disk audio cache.
    pub cached: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    None,
    Search,
    Bv,
}

pub struct BvImport {
    pub item: VideoItem,
    pub sync_message: String,
    pub sync_error: bool,
}

#[derive(Clone, uniffi::Record)]
pub struct LoginModal {
    pub qr: Vec<Vec<bool>>,
    pub status: String,
}

pub struct FavoritesModal {
    pub folders: Vec<FavoriteFolder>,
    pub selected: Option<usize>,
    pub loading: bool,
    pub status: String,
}

pub struct App {
    pub input: String,
    pub input_mode: InputMode,
    pub results: Vec<VideoItem>,
    pub results_title: String,
    /// True when the list is a playlist that should roll on to the next
    /// entry at track end (favorites, cloud sync). Search results are
    /// pick-one: they stop after the track instead of auto-playing on.
    pub queue_auto_advance: bool,
    pub selected: Option<usize>,
    pub current: Option<usize>,
    pub status: String,
    pub error: bool,
    pub loading: bool,
    pub search_seq: u64,
    pub now_playing: Option<VideoItem>,
    pub quality: String,
    pub page: usize,
    pub page_count: usize,
    pub part: String,
    pub current_cid: i64,
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    pub volume: f64,
    pub has_media: bool,
    pub pending_play: bool,
    pub playback_started: bool,
    pub eof: bool,
    pub user: Option<String>,
    pub login: Option<LoginModal>,
    pub login_seq: u64,
    pub favorites: Option<FavoritesModal>,
    pub show_lyrics: bool,
    pub lyrics: Vec<LyricLine>,
    pub lyrics_key: String,
    pub lyrics_status: String,
    pub cover_key: String,
    /// `bvid#cid#page` of the current track, used to route cache progress.
    pub playback_key: String,
    pub cache_progress: Option<(u64, Option<u64>)>,
    pub cached_now: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            input_mode: InputMode::None,
            results: Vec::new(),
            results_title: "搜索结果".to_string(),
            queue_auto_advance: false,
            selected: None,
            current: None,
            status: "搜索 B 站音乐；登录后可导入收藏夹与云同步".to_string(),
            error: false,
            loading: false,
            search_seq: 0,
            now_playing: None,
            quality: String::new(),
            page: 0,
            page_count: 0,
            part: String::new(),
            current_cid: 0,
            position: 0.0,
            duration: 0.0,
            paused: false,
            volume: 100.0,
            has_media: false,
            pending_play: false,
            playback_started: false,
            eof: false,
            user: None,
            login: None,
            login_seq: 0,
            favorites: None,
            show_lyrics: false,
            lyrics: Vec::new(),
            lyrics_key: String::new(),
            lyrics_status: String::new(),
            cover_key: String::new(),
            playback_key: String::new(),
            cache_progress: None,
            cached_now: false,
        }
    }

    pub fn open_login(&mut self) -> u64 {
        self.login_seq += 1;
        self.login = Some(LoginModal {
            qr: Vec::new(),
            status: "正在生成二维码…".to_string(),
        });
        self.set_status("正在生成登录二维码…", false);
        self.login_seq
    }

    pub fn open_favorites(&mut self) {
        self.favorites = Some(FavoritesModal {
            folders: Vec::new(),
            selected: None,
            loading: true,
            status: "正在加载收藏夹…".to_string(),
        });
        self.set_status("正在加载收藏夹…", false);
    }

    pub fn on_login_qr(&mut self, seq: u64, matrix: Vec<Vec<bool>>) {
        if seq != self.login_seq {
            return;
        }
        if let Some(modal) = &mut self.login {
            modal.qr = matrix;
            modal.status = "用 B 站手机客户端扫码".to_string();
        }
        self.set_status("等待扫码…", false);
    }

    pub fn on_login_phase(&mut self, seq: u64, status: String) {
        if seq != self.login_seq {
            return;
        }
        if let Some(modal) = &mut self.login {
            modal.status = status;
        }
    }

    pub fn on_login_success(&mut self, seq: u64, user: Option<UserInfo>) -> bool {
        if seq != self.login_seq {
            return false;
        }
        self.login = None;
        let name = user
            .map(|user| user.name)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "已登录".to_string());
        self.user = Some(name.clone());
        self.set_status(&format!("已登录：{name}"), false);
        true
    }

    pub fn on_login_failed(&mut self, seq: u64, message: String) {
        if seq != self.login_seq {
            return;
        }
        self.login = None;
        self.set_status(&format!("登录失败: {message}"), true);
    }

    pub fn on_folders_loaded(&mut self, result: Result<Vec<FavoriteFolder>, String>) {
        let (modal_status, global_status, is_error) = match result {
            Ok(folders) if folders.is_empty() => {
                ("没有收藏夹".to_string(), "没有收藏夹".to_string(), true)
            }
            Ok(folders) => {
                let count = folders.len();
                if let Some(modal) = &mut self.favorites {
                    modal.folders = folders;
                    modal.selected = Some(0);
                }
                (String::new(), format!("收藏夹 {count} 个"), false)
            }
            Err(error) => (
                format!("加载失败: {error}"),
                format!("收藏夹加载失败: {error}"),
                true,
            ),
        };

        if let Some(modal) = &mut self.favorites {
            modal.loading = false;
            modal.status = modal_status;
        }
        self.set_status(&global_status, is_error);
    }

    pub fn on_favorites_imported(
        &mut self,
        folder_title: String,
        result: Result<Vec<VideoItem>, String>,
    ) {
        match result {
            Ok(items) if items.is_empty() => {
                if let Some(modal) = &mut self.favorites {
                    modal.loading = false;
                    modal.status = "该收藏夹没有可播放视频".to_string();
                }
                self.set_status("该收藏夹没有可播放视频", true);
            }
            Ok(items) => {
                let count = items.len();
                self.favorites = None;
                self.replace_results(items, &format!("收藏夹 · {folder_title}"), true);
                self.set_status(&format!("已导入「{folder_title}」{count} 首"), false);
            }
            Err(error) => {
                if let Some(modal) = &mut self.favorites {
                    modal.loading = false;
                    modal.status = format!("导入失败: {error}");
                }
                self.set_status(&format!("收藏夹导入失败: {error}"), true);
            }
        }
    }

    pub fn on_search_done(&mut self, seq: u64, result: Result<Vec<VideoItem>, String>) {
        if seq != self.search_seq {
            return;
        }
        self.loading = false;
        match result {
            Ok(items) if items.is_empty() => {
                self.replace_results(items, "搜索结果", false);
                self.set_status("没有搜索到结果", true);
            }
            Ok(items) => {
                let count = items.len();
                self.replace_results(items, "搜索结果", false);
                self.set_status(&format!("找到 {count} 条结果，Enter 播放"), false);
            }
            Err(error) => {
                self.set_status(&error, true);
            }
        }
    }

    pub fn on_bv_imported(&mut self, result: Result<BvImport, String>) {
        match result {
            Ok(import) => {
                let bvid = import.item.bvid.clone();
                let title = import.item.title.clone();
                if let Some(index) = self.results.iter().position(|item| item.bvid == bvid) {
                    self.select(index);
                } else {
                    self.results.insert(0, import.item);
                    if let Some(current) = self.current {
                        self.current = Some(current + 1);
                    }
                    self.select(0);
                }
                self.set_status(
                    &format!("已导入「{title}」 · {}", import.sync_message),
                    import.sync_error,
                );
            }
            Err(error) => {
                self.set_status(&format!("BV 导入失败: {error}"), true);
            }
        }
    }

    pub fn on_sync_pulled(
        &mut self,
        auto: bool,
        result: Result<Option<(String, Vec<VideoItem>)>, String>,
    ) {
        match result {
            Ok(Some((title, items))) => {
                let count = items.len();
                if count == 0 {
                    if auto {
                        return;
                    }
                    self.replace_results(items, &format!("云同步 · {title}"), true);
                    self.set_status("云同步收藏夹还是空的，按 b 导入 BV 会自动加入", false);
                    return;
                }
                if auto
                    && (!self.results.is_empty()
                        || self.loading
                        || self.input_mode != InputMode::None)
                {
                    self.set_status(&format!("云同步「{title}」{count} 首，按 y 载入"), false);
                } else {
                    self.replace_results(items, &format!("云同步 · {title}"), true);
                    self.set_status(&format!("已从云同步载入 {count} 首"), false);
                }
            }
            Ok(None) => {
                if !auto {
                    self.set_status("还没有云同步收藏夹，导入一个 BV 会自动创建", false);
                }
            }
            Err(error) => {
                if !auto {
                    self.set_status(&format!("云同步失败: {error}"), true);
                }
            }
        }
    }

    pub fn on_playing(&mut self, info: PlayingInfo) {
        let key = format!("{}#{}#{}", info.item.bvid, info.cid, info.page_index);
        self.pending_play = false;
        self.now_playing = Some(info.item.clone());
        self.quality = info.quality.clone();
        self.page = info.page_index;
        self.page_count = info.page_count;
        self.part = info.part;
        self.current_cid = info.cid;
        self.position = 0.0;
        self.duration = if info.page_duration > 0 {
            info.page_duration as f64
        } else {
            parse_duration(&info.item.duration)
        };
        self.paused = false;
        self.has_media = true;
        self.playback_started = false;
        self.eof = false;
        self.lyrics_key = key.clone();
        self.lyrics.clear();
        self.lyrics_status = "加载歌词…".to_string();
        self.cover_key = key.clone();
        self.playback_key = key;
        self.cache_progress = None;
        self.cached_now = info.cached;

        let page_info = if info.page_count > 1 {
            format!("P{}/{} · ", info.page_index + 1, info.page_count)
        } else {
            String::new()
        };
        self.set_status(&format!("正在播放 · {page_info}{}", info.quality), false);
    }

    pub fn on_play_failed(&mut self, error: String) {
        self.pending_play = false;
        self.has_media = false;
        self.set_status(&format!("播放失败: {error}"), true);
    }

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

    pub fn on_cache_progress(&mut self, key: String, downloaded: u64, total: Option<u64>) {
        if key == self.playback_key {
            self.cache_progress = Some((downloaded, total));
        }
    }

    pub fn on_cache_done(&mut self, key: String, ok: bool) {
        if key != self.playback_key {
            return;
        }
        self.cache_progress = None;
        if ok {
            self.cached_now = true;
        }
    }

    pub fn on_player_state(&mut self, state: PlayerState) {
        self.volume = state.volume;
        if !self.has_media {
            return;
        }
        self.position = state.position;
        if state.duration > 0.0 {
            self.duration = state.duration;
        }
        self.paused = state.paused;

        if state.position > 0.5 {
            self.playback_started = true;
        }
        // At EOF mpv unloads the file and `eof-reached` may already be reset,
        // so treat "was playing, now idle" as the end of the track too.
        if state.eof || (state.idle && self.playback_started && !state.paused) {
            self.eof = true;
            self.playback_started = false;
        }
    }

    pub fn set_status(&mut self, message: &str, error: bool) {
        self.status = message.to_string();
        self.error = error;
    }

    pub fn set_error(&mut self, error: impl std::fmt::Display) {
        self.set_status(&format!("{error:#}"), true);
    }

    /// Consume the EOF flag so auto-advance runs exactly once per track end.
    pub fn take_eof(&mut self) -> bool {
        let eof = self.eof;
        self.eof = false;
        eof
    }

    /// Auto-advance at the end of a track. Pages of a multi-page video always
    /// roll on; crossing into the next list entry only happens for real
    /// playlists (favorites, cloud sync), so search results stop after the
    /// selected track.
    pub fn auto_advance(&mut self) -> Option<PlayRequest> {
        if self.page + 1 < self.page_count || self.queue_auto_advance {
            self.step(1)
        } else {
            None
        }
    }

    /// Move forward/backward by one track. A multi-page video is treated as
    /// several consecutive tracks: pages first, then the next queue entry.
    pub fn step(&mut self, delta: isize) -> Option<PlayRequest> {
        if self.results.is_empty() {
            return None;
        }

        if delta > 0
            && self.page + 1 < self.page_count
            && let Some(item) = self.now_playing.clone()
        {
            self.page += 1;
            self.pending_play = true;
            self.set_status("准备播放…", false);
            return Some(PlayRequest {
                item,
                page_index: self.page,
            });
        }

        if delta < 0
            && self.page > 0
            && let Some(item) = self.now_playing.clone()
        {
            self.page -= 1;
            self.pending_play = true;
            self.set_status("准备播放…", false);
            return Some(PlayRequest {
                item,
                page_index: self.page,
            });
        }

        let base = self.current.or(self.selected).unwrap_or(0) as isize;
        let next = base + delta;
        if next < 0 {
            self.set_status("已经是第一首", false);
            return None;
        }
        if next as usize >= self.results.len() {
            self.set_status("已经是最后一首", false);
            return None;
        }

        let index = next as usize;
        let item = self.results.get(index)?.clone();
        self.current = Some(index);
        self.select(index);
        self.page = 0;
        self.page_count = 0;
        self.part.clear();
        self.pending_play = true;
        self.set_status("准备播放…", false);
        Some(PlayRequest {
            item,
            page_index: 0,
        })
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

    pub fn play_selected(&mut self) -> Option<PlayRequest> {
        let index = self.selected?;
        self.play_at(index)
    }

    /// Start playing the queue entry at `index` from its first page.
    pub fn play_at(&mut self, index: usize) -> Option<PlayRequest> {
        let item = self.results.get(index)?.clone();
        self.selected = Some(index);
        self.current = Some(index);
        self.page = 0;
        self.page_count = 0;
        self.part.clear();
        self.pending_play = true;
        self.set_status("准备播放…", false);
        Some(PlayRequest {
            item,
            page_index: 0,
        })
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.results.is_empty() {
            return;
        }
        let current = self.selected.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.results.len() as isize - 1) as usize;
        self.select(next);
    }

    pub fn move_favorite(&mut self, delta: isize) {
        let Some(modal) = &mut self.favorites else {
            return;
        };
        if modal.folders.is_empty() {
            return;
        }
        let current = modal.selected.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, modal.folders.len() as isize - 1) as usize;
        modal.selected = Some(next);
    }

    fn replace_results(&mut self, items: Vec<VideoItem>, title: &str, auto_advance: bool) {
        let empty = items.is_empty();
        self.results = items;
        self.results_title = title.to_string();
        self.queue_auto_advance = auto_advance;
        self.current = None;
        self.page = 0;
        self.page_count = 0;
        self.part.clear();
        if empty {
            self.selected = None;
        } else {
            self.select(0);
        }
    }

    fn select(&mut self, index: usize) {
        self.selected = Some(index);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

pub fn qr_matrix(url: &str) -> Option<Vec<Vec<bool>>> {
    let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::L).ok()?;
    let width = code.width();
    let modules = code.to_colors();
    let quiet = 4;
    let size = width + quiet * 2;
    let mut matrix = vec![vec![false; size]; size];

    for y in 0..width {
        for x in 0..width {
            matrix[y + quiet][x + quiet] = modules[y * width + x] == QrColor::Dark;
        }
    }

    Some(matrix)
}

fn parse_duration(text: &str) -> f64 {
    text.split(':')
        .filter_map(|part| part.parse::<f64>().ok())
        .fold(0.0, |total, part| total * 60.0 + part)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(bvid: &str) -> VideoItem {
        VideoItem {
            bvid: bvid.to_string(),
            title: bvid.to_string(),
            author: String::new(),
            duration: "0:00".to_string(),
            pic: None,
        }
    }

    fn queue(auto_advance: bool) -> App {
        let mut app = App::new();
        app.replace_results(vec![item("BV1"), item("BV2")], "测试", auto_advance);
        app.play_at(0);
        app
    }

    #[test]
    fn search_queues_stop_at_track_end() {
        let mut app = queue(false);
        assert!(app.auto_advance().is_none());

        // Manual next still walks the list.
        let request = app.step(1).expect("manual next should work");
        assert_eq!(request.item.bvid, "BV2");
    }

    #[test]
    fn playlists_roll_on_to_the_next_entry() {
        let mut app = queue(true);
        let request = app.auto_advance().expect("playlist should advance");
        assert_eq!(request.item.bvid, "BV2");
        assert_eq!(request.page_index, 0);
    }

    #[test]
    fn multi_page_videos_auto_continue_inside_search_results() {
        let mut app = queue(false);
        app.page = 0;
        app.page_count = 2;
        app.now_playing = Some(item("BV1"));

        let request = app.auto_advance().expect("pages should keep playing");
        assert_eq!(request.item.bvid, "BV1");
        assert_eq!(request.page_index, 1);
    }
}
