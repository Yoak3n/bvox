use std::sync::Arc;

use image::DynamicImage;
use qrcode::types::Color as QrColor;
use qrcode::{EcLevel, QrCode};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

use bili_core::api::{FavoriteFolder, LyricLine, UserInfo, VideoItem};
use crate::cover::CoverSet;
use crate::mpv::PlayerState;

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

pub enum Action {
    None,
    Quit,
    Search(String),
    ImportBv(String),
    SyncPull,
    Play(PlayRequest),
    Pause,
    Seek(f64),
    Volume(f64),
    Login(u64),
    Favorites,
    ImportFolder(FavoriteFolder),
}

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
    pub selected: Option<usize>,
    pub current: Option<usize>,
    pub table_state: TableState,
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
    pub cover: Option<CoverSet>,
    pub cover_protocol: Option<StatefulProtocol>,
    pub cover_key: String,
    /// `bvid#cid#page` of the current track, used to route cache progress.
    pub playback_key: String,
    pub cache_progress: Option<(u64, Option<u64>)>,
    pub cached_now: bool,
    pub picker: Option<Arc<Picker>>,
}

impl App {
    pub fn new(picker: Option<Arc<Picker>>) -> Self {
        Self {
            input: String::new(),
            input_mode: InputMode::None,
            results: Vec::new(),
            results_title: "搜索结果".to_string(),
            selected: None,
            current: None,
            table_state: TableState::default(),
            status: "按 / 搜索，b 导入 BV，g 登录，f 收藏夹，y 云同步".to_string(),
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
            cover: None,
            cover_protocol: None,
            cover_key: String::new(),
            playback_key: String::new(),
            cache_progress: None,
            cached_now: false,
            picker,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        if self.login.is_some() {
            return self.handle_login_key(key);
        }
        if self.favorites.is_some() {
            return self.handle_favorites_key(key);
        }
        if self.input_mode != InputMode::None {
            return self.handle_input_key(key);
        }

        match key.code {
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Char('/') | KeyCode::Char('s') => {
                self.input_mode = InputMode::Search;
                self.input.clear();
                self.set_status("输入关键词，Enter 搜索，Esc 取消", false);
                Action::None
            }
            KeyCode::Char('b') => {
                self.input_mode = InputMode::Bv;
                self.input.clear();
                self.set_status("输入 BV 号或视频链接，Enter 导入", false);
                Action::None
            }
            KeyCode::Char('y') => Action::SyncPull,
            KeyCode::Char('g') => Action::Login(self.open_login()),
            KeyCode::Char('f') => Action::Favorites,
            KeyCode::Char('l') => {
                if self.now_playing.is_none() {
                    self.set_status("还没有播放内容", true);
                } else {
                    self.show_lyrics = !self.show_lyrics;
                }
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection(-1);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection(1);
                Action::None
            }
            KeyCode::Enter => {
                if self.pending_play {
                    return Action::None;
                }
                self.play_selected()
                    .map(Action::Play)
                    .unwrap_or(Action::None)
            }
            KeyCode::Char(' ') => {
                if !self.has_media {
                    return Action::None;
                }
                self.paused = !self.paused;
                Action::Pause
            }
            KeyCode::Char('n') => {
                if self.pending_play {
                    return Action::None;
                }
                self.step(1).map(Action::Play).unwrap_or(Action::None)
            }
            KeyCode::Char('p') => {
                if self.pending_play {
                    return Action::None;
                }
                self.step(-1).map(Action::Play).unwrap_or(Action::None)
            }
            KeyCode::Left => {
                if !self.has_media {
                    return Action::None;
                }
                self.position = (self.position - 5.0).max(0.0);
                Action::Seek(-5.0)
            }
            KeyCode::Right => {
                if !self.has_media {
                    return Action::None;
                }
                self.position += 5.0;
                Action::Seek(5.0)
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.volume = (self.volume + 5.0).min(150.0);
                Action::Volume(self.volume)
            }
            KeyCode::Char('-') => {
                self.volume = (self.volume - 5.0).max(0.0);
                Action::Volume(self.volume)
            }
            KeyCode::Char('r') => {
                if self.pending_play {
                    return Action::None;
                }
                let item = self.now_playing.clone().or_else(|| {
                    self.current
                        .and_then(|index| self.results.get(index))
                        .cloned()
                });
                if let Some(item) = item {
                    self.pending_play = true;
                    self.set_status("重新获取播放地址…", false);
                    Action::Play(PlayRequest {
                        item,
                        page_index: self.page,
                    })
                } else {
                    Action::None
                }
            }
            _ => Action::None,
        }
    }

    fn handle_login_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.login_seq += 1; // invalidate the polling task
                self.login = None;
                self.set_status("已取消登录", false);
                Action::None
            }
            KeyCode::Char('r') => Action::Login(self.open_login()),
            _ => Action::None,
        }
    }

    fn handle_favorites_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.favorites = None;
                self.set_status("已关闭收藏夹", false);
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_favorite(-1);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_favorite(1);
                Action::None
            }
            KeyCode::Enter => {
                let Some(modal) = &self.favorites else {
                    return Action::None;
                };
                if modal.loading {
                    return Action::None;
                }
                let Some(folder) = modal.folders.get(modal.selected.unwrap_or(0)).cloned() else {
                    return Action::None;
                };
                if let Some(modal) = &mut self.favorites {
                    modal.loading = true;
                    modal.status = format!("正在导入「{}」…", folder.title);
                }
                self.set_status("正在导入收藏夹…", false);
                Action::ImportFolder(folder)
            }
            _ => Action::None,
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) -> Action {
        let mode = self.input_mode;
        match key.code {
            KeyCode::Enter => {
                let text = self.input.trim().to_string();
                if text.is_empty() {
                    self.set_status("请输入内容", true);
                    return Action::None;
                }
                self.input_mode = InputMode::None;
                match mode {
                    InputMode::Search => {
                        self.loading = true;
                        self.search_seq += 1;
                        self.set_status("搜索中…", false);
                        Action::Search(text)
                    }
                    InputMode::Bv => {
                        self.set_status("正在导入 BV…", false);
                        Action::ImportBv(text)
                    }
                    InputMode::None => Action::None,
                }
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.set_status("已取消", false);
                Action::None
            }
            KeyCode::Backspace => {
                self.input.pop();
                Action::None
            }
            KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.push(character);
                Action::None
            }
            _ => Action::None,
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
                self.replace_results(items, &format!("收藏夹 · {folder_title}"));
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
                self.replace_results(items, "搜索结果");
                self.set_status("没有搜索到结果", true);
            }
            Ok(items) => {
                let count = items.len();
                self.replace_results(items, "搜索结果");
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
                    self.replace_results(items, &format!("云同步 · {title}"));
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
                    self.replace_results(items, &format!("云同步 · {title}"));
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
        self.cover = None;
        self.cover_protocol = None;

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

    pub fn on_cover_image(&mut self, key: String, result: Result<DynamicImage, String>) {
        if key != self.cover_key {
            return;
        }
        self.cover = None;
        self.cover_protocol = None;

        let Ok(image) = result else {
            return;
        };

        if let Some(picker) = &self.picker
            && picker.protocol_type() != ProtocolType::Halfblocks
        {
            self.cover_protocol = Some(picker.new_resize_protocol(image));
            return;
        }

        let small = crate::cover::from_dynamic(&image, 56, 32);
        let large = crate::cover::from_dynamic(&image, 80, 46);
        if let (Ok(small), Ok(large)) = (small, large) {
            self.cover = Some(CoverSet { small, large });
        }
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

    /// Move forward/backward by one track. A multi-page video is treated as
    /// several consecutive tracks: pages first, then the next search result.
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

    fn play_selected(&mut self) -> Option<PlayRequest> {
        let index = self.selected?;
        let item = self.results.get(index)?.clone();
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

    fn replace_results(&mut self, items: Vec<VideoItem>, title: &str) {
        let empty = items.is_empty();
        self.results = items;
        self.results_title = title.to_string();
        self.current = None;
        self.page = 0;
        self.page_count = 0;
        self.part.clear();
        if empty {
            self.selected = None;
            self.table_state.select(None);
        } else {
            self.select(0);
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.results.is_empty() {
            return;
        }
        let current = self.selected.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.results.len() as isize - 1) as usize;
        self.select(next);
    }

    fn move_favorite(&mut self, delta: isize) {
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

    fn select(&mut self, index: usize) {
        self.selected = Some(index);
        self.table_state.select(Some(index));
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new(None)
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
