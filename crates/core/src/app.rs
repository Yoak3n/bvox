//! UI-agnostic session state.
//!
//! `App` holds every piece of state a frontend renders: the playlist library,
//! the song queue, playback progress, login, lyrics. Frontends mutate it
//! through [`Session`](crate::session::Session) commands and read it back for
//! drawing; the field set is deliberately plain data so it maps onto Compose
//! state as easily as onto a ratatui frame.

use std::collections::HashSet;

use qrcode::types::Color as QrColor;
use qrcode::{EcLevel, QrCode};
use serde::{Deserialize, Serialize};

use crate::api::{
    AudioQuality, FavoriteFolder, LyricLine, PlayMode, SearchPage, UserInfo, VideoItem,
};
use crate::cache::{CacheMode, CacheStats, CachedTrack};
use crate::player::PlayerState;

pub struct PlayRequest {
    pub item: VideoItem,
    pub page_index: usize,
    /// Seconds to start from; non-zero when resuming a restored track.
    pub start_at: f64,
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
    /// Every 分P of the loaded video, for the frontend page picker.
    pub pages: Vec<PageSummary>,
    /// Seconds the player was asked to start from (resume), zero otherwise.
    pub start_position: f64,
    /// True when playback came from the on-disk audio cache.
    pub cached: bool,
}

/// One 分P of the video currently loaded in the player.
#[derive(Clone, Debug, Serialize, Deserialize, uniffi::Record)]
pub struct PageSummary {
    pub cid: i64,
    /// Part title, falling back to `P{n}` when Bilibili omits it.
    pub title: String,
    /// Formatted `m:ss` length of this page.
    pub duration: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    None,
    Search,
    Bv,
    NewPlaylist,
    RenamePlaylist,
}

/// Which list the main panel shows: the favorite folders (playlists) or the
/// current song queue (playlist contents / search results).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainView {
    Library,
    Queue,
}

pub struct PlaylistPicker {
    /// Track waiting to be added to a folder, identified by BV id so the
    /// selection cannot go stale while folders load.
    pub target_bvid: String,
    pub selected: Option<usize>,
}

/// Small xorshift PRNG for shuffle order. Avoids a dependency for the one
/// thing the core needs randomness for.
struct ShuffleRng(u64);

impl ShuffleRng {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self((nanos ^ ((std::process::id() as u64) << 32)).max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.0 = value;
        value.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, upper: usize) -> usize {
        if upper == 0 {
            0
        } else {
            (self.next_u64() % upper as u64) as usize
        }
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            let other = self.below(index + 1);
            items.swap(index, other);
        }
    }
}

pub struct BvImport {
    pub item: VideoItem,
    pub message: String,
    pub error: bool,
    /// Folder the video was added to, when a playlist was open.
    pub playlist: Option<FavoriteFolder>,
    /// Whether the account can create a playlist for a parked import.
    pub logged_in: bool,
}

/// A BV import that has no playlist to land in yet. Both frontends prompt for
/// a new playlist and then move this track into it.
#[derive(Clone, Debug)]
pub struct PendingImport {
    pub bvid: String,
    pub title: String,
}

#[derive(Clone, uniffi::Record)]
pub struct LoginModal {
    pub qr: Vec<Vec<bool>>,
    pub status: String,
}

pub struct App {
    pub input: String,
    pub input_mode: InputMode,
    pub results: Vec<VideoItem>,
    pub results_title: String,
    /// Playback order: sequential, repeat-all, repeat-one or shuffle.
    pub play_mode: PlayMode,
    pub selected: Option<usize>,
    pub current: Option<usize>,
    pub status: String,
    pub error: bool,
    pub loading: bool,
    pub search_seq: u64,
    /// Keyword of the currently displayed search results, when they came from
    /// a search. Empty for playlists and BV imports.
    pub search_keyword: String,
    /// Last search page loaded and how many pages the query has (1-based).
    pub search_page: u32,
    pub search_pages: u32,
    /// Total hits reported by Bilibili for the current search.
    pub search_total: u64,
    /// True while the next search page is in flight.
    pub loading_more: bool,
    pub now_playing: Option<VideoItem>,
    pub quality: String,
    pub page: usize,
    pub page_count: usize,
    pub part: String,
    /// 分P list of the loaded video, backing the page picker. Cleared
    /// whenever the queue moves to another track; refilled on `Playing`.
    pub playing_pages: Vec<PageSummary>,
    pub current_cid: i64,
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    pub volume: f64,
    pub has_media: bool,
    pub pending_play: bool,
    /// Consecutive load failures while skipping unavailable tracks. Reset on
    /// a successful start or when a new queue is installed.
    pub play_failures: usize,
    pub playback_started: bool,
    pub eof: bool,
    /// True when a cookie is stored, even if the nav request cannot confirm
    /// the account (offline starts). Distinguishes offline from logged out.
    pub has_cookie: bool,
    pub user: Option<String>,
    /// Avatar URL for the logged-in account, when the nav response has one.
    pub user_face: Option<String>,
    /// True when the logged-in account has an active 大会员 subscription.
    pub vip: bool,
    /// Audio track preference; `Auto` follows the account entitlement.
    pub quality_pref: AudioQuality,
    /// Folder ids hidden by the user; persisted per account by `ApiClient`.
    pub hidden_playlists: HashSet<i64>,
    /// Temporarily reveal hidden folders (e.g. to unhide one).
    pub show_hidden: bool,
    pub login: Option<LoginModal>,
    pub login_seq: u64,
    pub view: MainView,
    pub playlists: Vec<FavoriteFolder>,
    pub playlists_loading: bool,
    pub playlists_seq: u64,
    pub playlist_open_seq: u64,
    pub playlist_selected: Option<usize>,
    /// The folder the current queue was opened from, if any.
    pub current_playlist: Option<FavoriteFolder>,
    /// Add-to-playlist modal (TUI).
    pub picker: Option<PlaylistPicker>,
    /// Folder being renamed through the text input.
    pub rename_target: Option<FavoriteFolder>,
    /// Imported track waiting for the user to create a playlist for it.
    pub pending_import: Option<PendingImport>,
    /// Shuffle bookkeeping: indices already played (for previous) and the
    /// remaining bag for the current pass.
    shuffle_history: Vec<usize>,
    shuffle_remaining: Vec<usize>,
    shuffle_rng: ShuffleRng,
    pub show_lyrics: bool,
    pub lyrics: Vec<LyricLine>,
    pub lyrics_key: String,
    pub lyrics_status: String,
    pub cover_key: String,
    /// `bvid#cid#page` of the current track, used to route cache progress.
    pub playback_key: String,
    pub cache_progress: Option<(u64, Option<u64>)>,
    pub cached_now: bool,
    /// On-disk cache usage, refreshed by the session after writes/cleanup.
    pub cache_stats: Option<CacheStats>,
    /// What the cache stores (auto / small files only / off).
    pub cache_mode: CacheMode,
    /// BVIDs with completed audio files, refreshed by the session. List views
    /// use it to mark cached tracks.
    pub cached_bvids: HashSet<String>,
    /// Playable offline entries for the "cached songs" screen, newest first.
    pub cached_tracks: Vec<CachedTrack>,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            input_mode: InputMode::None,
            results: Vec::new(),
            results_title: "搜索结果".to_string(),
            play_mode: PlayMode::Sequential,
            selected: None,
            current: None,
            status: "搜索 B 站音乐；登录后管理收藏夹歌单".to_string(),
            error: false,
            loading: false,
            search_seq: 0,
            search_keyword: String::new(),
            search_page: 0,
            search_pages: 0,
            search_total: 0,
            loading_more: false,
            now_playing: None,
            quality: String::new(),
            page: 0,
            page_count: 0,
            part: String::new(),
            playing_pages: Vec::new(),
            current_cid: 0,
            position: 0.0,
            duration: 0.0,
            paused: false,
            volume: 100.0,
            has_media: false,
            pending_play: false,
            play_failures: 0,
            playback_started: false,
            eof: false,
            has_cookie: false,
            user: None,
            user_face: None,
            vip: false,
            quality_pref: AudioQuality::Auto,
            hidden_playlists: HashSet::new(),
            show_hidden: false,
            login: None,
            login_seq: 0,
            view: MainView::Queue,
            playlists: Vec::new(),
            playlists_loading: false,
            playlists_seq: 0,
            playlist_open_seq: 0,
            playlist_selected: None,
            current_playlist: None,
            picker: None,
            rename_target: None,
            pending_import: None,
            shuffle_history: Vec::new(),
            shuffle_remaining: Vec::new(),
            shuffle_rng: ShuffleRng::new(),
            show_lyrics: false,
            lyrics: Vec::new(),
            lyrics_key: String::new(),
            lyrics_status: String::new(),
            cover_key: String::new(),
            playback_key: String::new(),
            cache_progress: None,
            cached_now: false,
            cache_stats: None,
            cache_mode: CacheMode::Auto,
            cached_bvids: HashSet::new(),
            cached_tracks: Vec::new(),
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

    fn fix_playlist_selection(&mut self) {
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
            .as_ref()
            .map(|user| user.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "已登录".to_string());
        self.vip = user.as_ref().is_some_and(|user| user.vip);
        self.user_face = user.as_ref().and_then(|user| user.face.clone());
        self.user = Some(name.clone());
        self.view = MainView::Library;
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

    pub fn on_search_done(
        &mut self,
        seq: u64,
        keyword: String,
        result: Result<SearchPage, String>,
    ) {
        if seq != self.search_seq {
            return;
        }
        self.loading = false;
        self.view = MainView::Queue;
        match result {
            Ok(page) if page.items.is_empty() => {
                let page_no = page.page.max(1);
                self.replace_results(Vec::new(), "搜索结果", None);
                self.set_search_paging(keyword, page_no, page_no, page.total);
                self.set_status("没有搜索到结果", true);
            }
            Ok(page) => {
                let count = page.items.len();
                self.replace_results(page.items, "搜索结果", None);
                self.set_search_paging(keyword, page.page, page.pages, page.total);
                // Platform-neutral: the TUI renders its own key hints.
                if self.has_more_results() {
                    self.set_status(&format!("找到 {count} 条结果，还有更多"), false);
                } else {
                    self.set_status(&format!("找到 {count} 条结果"), false);
                }
            }
            Err(error) => {
                self.set_status(&error, true);
            }
        }
    }

    /// Append the next search page. Stale pages (a newer search started) and
    /// duplicates (Bilibili sometimes repeats hits across pages) are dropped.
    pub fn on_search_more(&mut self, seq: u64, result: Result<SearchPage, String>) {
        if seq != self.search_seq {
            return;
        }
        self.loading_more = false;
        match result {
            Ok(page) => {
                let mut known = self
                    .results
                    .iter()
                    .map(|item| item.bvid.clone())
                    .collect::<HashSet<_>>();
                let empty = page.items.is_empty();
                let first_new = self.results.len();
                let mut added = 0;
                for item in page.items {
                    if known.insert(item.bvid.clone()) {
                        self.results.push(item);
                        added += 1;
                    }
                }
                // Keep shuffle mode covering the freshly loaded tracks without
                // restarting the current pass.
                if self.play_mode == PlayMode::Shuffle && first_new < self.results.len() {
                    let tail = self.shuffle_remaining.len();
                    self.shuffle_remaining.extend(first_new..self.results.len());
                    self.shuffle_rng
                        .shuffle(&mut self.shuffle_remaining[tail..]);
                }
                let page_no = page.page.max(1);
                self.search_page = page_no;
                // An empty page always ends the run, even if `numPages` lied.
                self.search_pages = if empty {
                    page_no
                } else {
                    page.pages.max(page_no)
                };
                self.search_total = page.total;
                if self.selected.is_none() && !self.results.is_empty() {
                    self.select(0);
                }
                if added == 0 && !self.has_more_results() {
                    self.set_status("没有更多结果了", false);
                } else {
                    self.set_status(&format!("已加载 {} 条结果", self.results.len()), false);
                }
            }
            Err(error) => self.set_status(&error, true),
        }
    }

    /// True when the current queue is a search with another page available.
    pub fn has_more_results(&self) -> bool {
        !self.search_keyword.is_empty()
            && self.current_playlist.is_none()
            && self.search_page < self.search_pages
    }

    /// True when a completed audio file for this video exists locally.
    pub fn is_cached(&self, bvid: &str) -> bool {
        self.cached_bvids.contains(bvid)
    }

    fn set_search_paging(&mut self, keyword: String, page: u32, pages: u32, total: u64) {
        let page_no = page.max(1);
        self.search_keyword = keyword;
        self.search_page = page_no;
        self.search_pages = pages.max(page_no);
        self.search_total = total;
        self.loading_more = false;
    }

    fn clear_search_paging(&mut self) {
        self.search_keyword.clear();
        self.search_page = 0;
        self.search_pages = 0;
        self.search_total = 0;
        self.loading_more = false;
    }

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
                self.pending_import = (import.playlist.is_none() && import.logged_in).then(|| {
                    PendingImport {
                        bvid: bvid.clone(),
                        title: title.clone(),
                    }
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

    pub fn on_playing(&mut self, info: PlayingInfo) {
        let key = format!("{}#{}#{}", info.item.bvid, info.cid, info.page_index);
        self.pending_play = false;
        self.play_failures = 0;
        self.now_playing = Some(info.item.clone());
        self.quality = info.quality.clone();
        self.page = info.page_index;
        self.page_count = info.page_count;
        self.part = info.part;
        self.playing_pages = info.pages;
        self.current_cid = info.cid;
        self.position = info.start_position.max(0.0);
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

    /// Auto-advance at the end of a track. Repeat-one replays the current
    /// page; a multi-page video continues with its next page; otherwise the
    /// play mode decides which queue entry comes next.
    pub fn auto_advance(&mut self) -> Option<PlayRequest> {
        if self.play_mode == PlayMode::RepeatOne {
            return self.replay_current();
        }
        if self.page + 1 < self.page_count && self.now_playing.is_some() {
            return self.step_page(1);
        }
        match self.play_mode {
            PlayMode::Shuffle => self.step_shuffle(1),
            _ => self.step_queue(1),
        }
    }

    /// Move past a track that failed to load. Bounded by the queue length so
    /// an offline queue full of uncached tracks cannot retry forever.
    pub fn skip_failed(&mut self) -> Option<PlayRequest> {
        let queue_len = self.results.len();
        if queue_len == 0 {
            return None;
        }
        self.play_failures += 1;
        if self.play_failures >= queue_len {
            self.play_failures = 0;
            self.set_status("列表中没有可播放的歌曲（可能未缓存）", true);
            return None;
        }
        let request = match self.play_mode {
            PlayMode::Shuffle => self.step_shuffle(1),
            _ => self.step_queue(1),
        };
        if request.is_none() {
            self.play_failures = 0;
            self.set_status("列表已播完，没有可播放的歌曲", true);
        }
        request
    }

    /// Move forward/backward by one track. A multi-page video is treated as
    /// several consecutive tracks: pages first, then the queue entry chosen
    /// by the current play mode.
    pub fn step(&mut self, delta: isize) -> Option<PlayRequest> {
        if self.results.is_empty() {
            return None;
        }

        if delta > 0 && self.page + 1 < self.page_count && self.now_playing.is_some() {
            return self.step_page(1);
        }
        if delta < 0 && self.page > 0 && self.now_playing.is_some() {
            return self.step_page(-1);
        }

        if self.play_mode == PlayMode::Shuffle {
            return self.step_shuffle(delta);
        }
        self.step_queue(delta)
    }

    fn step_page(&mut self, delta: isize) -> Option<PlayRequest> {
        let item = self.now_playing.clone()?;
        self.page = (self.page as isize + delta).max(0) as usize;
        self.pending_play = true;
        self.set_status("准备播放…", false);
        Some(PlayRequest {
            item,
            page_index: self.page,
            start_at: 0.0,
        })
    }

    fn replay_current(&mut self) -> Option<PlayRequest> {
        let item = self.now_playing.clone().or_else(|| {
            self.current
                .and_then(|index| self.results.get(index))
                .cloned()
        })?;
        self.pending_play = true;
        self.set_status("单曲循环…", false);
        Some(PlayRequest {
            item,
            page_index: self.page,
            start_at: 0.0,
        })
    }

    fn step_queue(&mut self, delta: isize) -> Option<PlayRequest> {
        let len = self.results.len();
        if len == 0 {
            return None;
        }
        let base = self.current.or(self.selected).unwrap_or(0) as isize;
        let next = base + delta;
        let index = if next < 0 || next as usize >= len {
            if self.play_mode == PlayMode::RepeatAll {
                next.rem_euclid(len as isize) as usize
            } else {
                self.set_status(
                    if delta > 0 {
                        "已经是最后一首"
                    } else {
                        "已经是第一首"
                    },
                    false,
                );
                return None;
            }
        } else {
            next as usize
        };
        self.start_queue_index(index, false)
    }

    fn step_shuffle(&mut self, delta: isize) -> Option<PlayRequest> {
        if delta > 0 {
            let next = self.next_shuffle_index()?;
            self.start_queue_index(next, true)
        } else {
            let Some(previous) = self.previous_shuffle_index() else {
                self.set_status("没有上一首了", false);
                return None;
            };
            self.start_queue_index(previous, false)
        }
    }

    fn start_queue_index(&mut self, index: usize, record_history: bool) -> Option<PlayRequest> {
        let item = self.results.get(index)?.clone();
        if record_history
            && self.play_mode == PlayMode::Shuffle
            && let Some(current) = self.current
            && current != index
        {
            self.shuffle_history.push(current);
        }
        self.current = Some(index);
        self.select(index);
        self.page = 0;
        self.page_count = 0;
        self.part.clear();
        self.playing_pages.clear();
        self.pending_play = true;
        self.set_status("准备播放…", false);
        Some(PlayRequest {
            item,
            page_index: 0,
            start_at: 0.0,
        })
    }

    /// Rebuild the shuffle bag around the current track. Called when the
    /// queue changes or the user switches to shuffle mode.
    pub fn reset_shuffle(&mut self) {
        self.shuffle_history.clear();
        self.shuffle_remaining.clear();
        if self.results.is_empty() {
            return;
        }
        self.shuffle_remaining = (0..self.results.len())
            .filter(|index| Some(*index) != self.current)
            .collect();
        self.shuffle_rng.shuffle(&mut self.shuffle_remaining);
    }

    /// Rebuild the current-pass bag, excluding the track that is playing.
    fn refill_shuffle(&mut self) {
        self.shuffle_remaining = (0..self.results.len())
            .filter(|index| Some(*index) != self.current)
            .collect();
        self.shuffle_rng.shuffle(&mut self.shuffle_remaining);
    }

    fn next_shuffle_index(&mut self) -> Option<usize> {
        let len = self.results.len();
        if len == 0 {
            return None;
        }
        if len == 1 {
            return Some(0);
        }
        loop {
            if self.shuffle_remaining.is_empty() {
                self.refill_shuffle();
            }
            let index = self.shuffle_remaining.pop()?;
            if index < len && Some(index) != self.current {
                return Some(index);
            }
        }
    }

    fn previous_shuffle_index(&mut self) -> Option<usize> {
        while let Some(index) = self.shuffle_history.pop() {
            if index < self.results.len() && Some(index) != self.current {
                if let Some(current) = self.current {
                    self.shuffle_remaining.push(current);
                }
                return Some(index);
            }
        }
        None
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

    /// Replace the queue with the offline cached tracks. Playback uses the
    /// normal path so `view`/audio cache hits keep working without network.
    pub fn show_cached_tracks(&mut self, items: Vec<VideoItem>) {
        self.replace_results(items, "已缓存歌曲", None);
    }

    /// Start playing the queue entry at `index` from its first page.
    pub fn play_at(&mut self, index: usize) -> Option<PlayRequest> {
        let item = self.results.get(index)?.clone();
        self.selected = Some(index);
        self.current = Some(index);
        self.play_failures = 0;
        self.page = 0;
        self.page_count = 0;
        self.part.clear();
        self.playing_pages.clear();
        self.pending_play = true;
        self.reset_shuffle();
        self.set_status("准备播放…", false);
        Some(PlayRequest {
            item,
            page_index: 0,
            start_at: 0.0,
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

    /// Move the main-panel cursor according to the visible list.
    pub fn move_main_selection(&mut self, delta: isize) {
        match self.view {
            MainView::Library => self.move_playlist_selection(delta),
            MainView::Queue => self.move_selection(delta),
        }
    }

    fn replace_results(
        &mut self,
        items: Vec<VideoItem>,
        title: &str,
        playlist: Option<FavoriteFolder>,
    ) {
        let empty = items.is_empty();
        self.results = items;
        self.results_title = title.to_string();
        self.current_playlist = playlist;
        self.play_failures = 0;
        self.current = None;
        self.page = 0;
        self.page_count = 0;
        self.part.clear();
        if empty {
            self.selected = None;
        } else {
            self.select(0);
        }
        self.clear_search_paging();
        self.reset_shuffle();
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
            aid: 0,
            bvid: bvid.to_string(),
            title: bvid.to_string(),
            author: String::new(),
            duration: "0:00".to_string(),
            pic: None,
        }
    }

    fn queue(mode: PlayMode) -> App {
        let mut app = App::new();
        app.play_mode = mode;
        app.replace_results(vec![item("BV1"), item("BV2")], "测试", None);
        app.play_at(0);
        app
    }

    fn folder(id: i64, title: &str, count: i64) -> FavoriteFolder {
        FavoriteFolder {
            id,
            title: title.to_string(),
            count,
        }
    }

    fn open_playlist(app: &mut App, folder: FavoriteFolder, items: Vec<VideoItem>) {
        let seq = app.begin_playlist_open();
        app.on_playlist_opened(seq, folder, Ok(items), false);
    }

    fn search_page(items: Vec<VideoItem>, page: u32, pages: u32, total: u64) -> SearchPage {
        SearchPage {
            items,
            page,
            pages,
            total,
        }
    }

    #[test]
    fn opening_a_playlist_makes_it_the_auto_advancing_queue() {
        let mut app = App::new();
        open_playlist(
            &mut app,
            folder(7, "歌单", 2),
            vec![item("BV1"), item("BV2")],
        );

        assert_eq!(app.view, MainView::Queue);
        assert_eq!(app.results_title, "歌单 · 歌单");
        assert_eq!(
            app.current_playlist.as_ref().map(|folder| folder.id),
            Some(7)
        );

        let request = app.auto_advance().expect("playlist should advance");
        assert_eq!(request.item.bvid, "BV2");
    }

    #[test]
    fn search_results_clear_the_current_playlist() {
        let mut app = App::new();
        open_playlist(&mut app, folder(7, "歌单", 1), vec![item("BV1")]);
        app.search_seq = 1;
        app.on_search_done(
            1,
            "晴天".to_string(),
            Ok(search_page(vec![item("BV10")], 1, 1, 1)),
        );

        assert_eq!(app.view, MainView::Queue);
        assert!(app.current_playlist.is_none());
        assert!(app.auto_advance().is_none());
    }

    #[test]
    fn search_reports_more_pages_and_appends_them() {
        let mut app = App::new();
        app.search_seq = 1;
        app.on_search_done(
            1,
            "晴天".to_string(),
            Ok(search_page(vec![item("BV1")], 1, 2, 2)),
        );

        assert!(app.has_more_results());
        assert_eq!(app.search_keyword, "晴天");
        assert_eq!(app.search_total, 2);

        app.loading_more = true;
        app.on_search_more(1, Ok(search_page(vec![item("BV2")], 2, 2, 2)));

        assert!(!app.loading_more);
        assert!(!app.has_more_results());
        assert_eq!(
            app.results
                .iter()
                .map(|item| item.bvid.as_str())
                .collect::<Vec<_>>(),
            ["BV1", "BV2"]
        );
    }

    #[test]
    fn search_more_dedupes_repeated_hits() {
        let mut app = App::new();
        app.search_seq = 1;
        app.on_search_done(
            1,
            "晴天".to_string(),
            Ok(search_page(vec![item("BV1"), item("BV2")], 1, 2, 3)),
        );

        app.on_search_more(1, Ok(search_page(vec![item("BV2"), item("BV3")], 2, 2, 3)));

        assert_eq!(
            app.results
                .iter()
                .map(|item| item.bvid.as_str())
                .collect::<Vec<_>>(),
            ["BV1", "BV2", "BV3"]
        );
    }

    #[test]
    fn loading_more_results_extends_the_shuffle_bag() {
        let mut app = App::new();
        app.play_mode = PlayMode::Shuffle;
        app.search_seq = 1;
        app.on_search_done(
            1,
            "晴天".to_string(),
            Ok(search_page(vec![item("BV1")], 1, 2, 2)),
        );
        assert_eq!(app.shuffle_remaining.len(), 1);

        app.on_search_more(1, Ok(search_page(vec![item("BV2")], 2, 2, 2)));

        assert_eq!(app.shuffle_remaining.len(), 2);
        assert!(app.shuffle_remaining.contains(&1));
    }

    #[test]
    fn stale_search_pages_are_ignored() {
        let mut app = App::new();
        app.search_seq = 2;
        app.on_search_done(
            2,
            "晴天".to_string(),
            Ok(search_page(vec![item("BV1")], 1, 2, 2)),
        );

        app.on_search_more(1, Ok(search_page(vec![item("BV9")], 2, 2, 2)));

        assert_eq!(app.results.len(), 1);
        assert_eq!(app.results[0].bvid, "BV1");
        assert!(app.has_more_results());
    }

    #[test]
    fn opening_a_playlist_clears_search_paging() {
        let mut app = App::new();
        app.search_seq = 1;
        app.on_search_done(
            1,
            "晴天".to_string(),
            Ok(search_page(vec![item("BV1")], 1, 2, 2)),
        );

        open_playlist(&mut app, folder(7, "歌单", 1), vec![item("BV2")]);

        assert!(!app.has_more_results());
        assert!(app.search_keyword.is_empty());
    }

    #[test]
    fn removing_a_track_keeps_indices_consistent() {
        let mut app = App::new();
        open_playlist(
            &mut app,
            folder(7, "歌单", 3),
            vec![item("BV1"), item("BV2"), item("BV3")],
        );
        app.current = Some(2);
        app.selected = Some(2);

        app.on_track_removed(7, "BV1".to_string(), true, Ok(()));

        assert_eq!(
            app.results
                .iter()
                .map(|item| item.bvid.as_str())
                .collect::<Vec<_>>(),
            ["BV2", "BV3"]
        );
        assert_eq!(app.current, Some(1));
        assert_eq!(app.selected, Some(1));
        assert_eq!(
            app.current_playlist.as_ref().map(|folder| folder.count),
            Some(2)
        );
    }

    #[test]
    fn removing_the_playing_track_clears_its_queue_index() {
        let mut app = App::new();
        open_playlist(
            &mut app,
            folder(7, "歌单", 2),
            vec![item("BV1"), item("BV2")],
        );
        app.current = Some(1);

        app.on_track_removed(7, "BV2".to_string(), true, Ok(()));

        assert_eq!(app.results.len(), 1);
        assert!(app.current.is_none());
        assert_eq!(app.selected, Some(0));
    }

    #[test]
    fn renaming_a_playlist_updates_the_open_queue_title() {
        let mut app = App::new();
        app.playlists = vec![folder(7, "旧名", 1)];
        open_playlist(&mut app, folder(7, "旧名", 1), vec![item("BV1")]);

        app.on_playlist_renamed(7, "新名".to_string(), Ok(()));

        assert_eq!(app.playlists[0].title, "新名");
        assert_eq!(
            app.current_playlist
                .as_ref()
                .map(|folder| folder.title.as_str()),
            Some("新名")
        );
        assert_eq!(app.results_title, "歌单 · 新名");
    }

    #[test]
    fn deleting_the_open_playlist_keeps_the_local_queue() {
        let mut app = App::new();
        app.playlists = vec![folder(7, "歌单", 1)];
        open_playlist(&mut app, folder(7, "歌单", 1), vec![item("BV1")]);

        app.on_playlist_deleted(7, Ok(()));

        assert!(app.playlists.is_empty());
        assert!(app.current_playlist.is_none());
        assert_eq!(app.results.len(), 1);
    }

    #[test]
    fn picker_opens_before_the_folder_list_arrives() {
        let mut app = App::new();
        app.open_picker("BV1".to_string());
        assert_eq!(
            app.picker
                .as_ref()
                .map(|picker| picker.target_bvid.as_str()),
            Some("BV1")
        );
        assert!(app.picker_folder().is_none());

        let seq = app.begin_playlists_load();
        app.on_playlists_loaded(seq, Ok((vec![folder(7, "歌单", 0)], Vec::new())));

        assert_eq!(app.picker_folder().map(|folder| folder.id), Some(7));
    }

    #[test]
    fn hidden_playlists_are_filtered_out_of_the_library() {
        let mut app = App::new();
        let seq = app.begin_playlists_load();
        app.on_playlists_loaded(
            seq,
            Ok((
                vec![
                    folder(7, "稍后再听", 3),
                    folder(8, "视频收藏", 20),
                    folder(9, "纯音乐", 5),
                ],
                vec![8],
            )),
        );

        assert_eq!(
            app.library_playlists()
                .iter()
                .map(|folder| folder.id)
                .collect::<Vec<_>>(),
            vec![7, 9]
        );
        assert_eq!(app.playlist_selected, Some(0));

        // Revealing hidden folders restores the entry with its hidden mark.
        app.toggle_show_hidden();
        assert_eq!(app.library_playlists().len(), 3);
        assert!(app.is_hidden(8));

        // There is no special-cased folder anymore: every folder can hide.
        app.toggle_show_hidden();
        app.apply_playlist_hidden(7, true);
        assert!(!app.library_playlists().iter().any(|folder| folder.id == 7));
    }

    #[test]
    fn importing_without_a_playlist_parks_the_track_for_a_new_playlist() {
        let mut app = App::new();
        app.on_bv_imported(Ok(BvImport {
            item: item("BV1"),
            message: "未打开歌单".to_string(),
            error: false,
            playlist: None,
            logged_in: true,
        }));

        assert_eq!(
            app.pending_import
                .as_ref()
                .map(|pending| pending.bvid.as_str()),
            Some("BV1")
        );
        app.cancel_pending_import();
        assert!(app.pending_import.is_none());
    }

    #[test]
    fn logged_out_or_playlist_imports_do_not_park_a_track() {
        let mut app = App::new();
        app.on_bv_imported(Ok(BvImport {
            item: item("BV1"),
            message: "未登录，仅加入本地列表".to_string(),
            error: false,
            playlist: None,
            logged_in: false,
        }));
        assert!(app.pending_import.is_none());

        app.on_bv_imported(Ok(BvImport {
            item: item("BV2"),
            message: "已加入歌单「歌单」".to_string(),
            error: false,
            playlist: Some(folder(7, "歌单", 0)),
            logged_in: true,
        }));
        assert!(app.pending_import.is_none());
    }

    #[test]
    fn offline_refresh_keeps_the_cached_folder_list() {
        let mut app = App::new();
        app.load_cached_playlists(vec![folder(7, "缓存歌单", 2)], Vec::new());
        assert_eq!(app.library_playlists().len(), 1);

        let seq = app.begin_playlists_load();
        app.on_playlists_loaded(seq, Err("网络不可用".to_string()));

        assert_eq!(app.library_playlists().len(), 1);
        assert_eq!(app.status, "离线：显示缓存的歌单");
        assert!(!app.error);
    }

    #[test]
    fn offline_playlist_open_is_marked_in_the_status() {
        let mut app = App::new();
        let seq = app.begin_playlist_open();
        app.on_playlist_opened(
            seq,
            folder(7, "缓存歌单", 1),
            Ok(vec![item("BV1")]),
            true,
        );

        assert_eq!(app.results.len(), 1);
        assert!(app.status.contains("离线缓存"));
    }

    #[test]
    fn stale_playlist_responses_are_ignored() {
        let mut app = App::new();
        let stale_open = app.begin_playlist_open();
        let _fresh_open = app.begin_playlist_open();
        app.on_playlist_opened(stale_open, folder(7, "旧歌单", 1), Ok(vec![item("BV1")]), false);
        assert!(app.current_playlist.is_none());
        assert!(app.results.is_empty());

        let stale_list = app.begin_playlists_load();
        let _fresh_list = app.begin_playlists_load();
        app.on_playlists_loaded(stale_list, Ok((vec![folder(7, "旧列表", 1)], Vec::new())));
        assert!(app.playlists.is_empty());
    }

    #[test]
    fn sequential_advances_then_stops_at_the_end() {
        let mut app = queue(PlayMode::Sequential);
        let request = app.auto_advance().expect("sequential should advance");
        assert_eq!(request.item.bvid, "BV2");
        assert!(app.auto_advance().is_none());
    }

    #[test]
    fn repeat_all_wraps_to_the_first_track() {
        let mut app = queue(PlayMode::RepeatAll);
        let request = app.auto_advance().expect("advance to BV2");
        assert_eq!(request.item.bvid, "BV2");
        let request = app.auto_advance().expect("wrap to BV1");
        assert_eq!(request.item.bvid, "BV1");
    }

    #[test]
    fn repeat_one_replays_the_current_page() {
        let mut app = queue(PlayMode::RepeatOne);
        let request = app.auto_advance().expect("repeat one");
        assert_eq!(request.item.bvid, "BV1");
        assert_eq!(request.page_index, 0);

        // Even a multi-page video repeats the current page.
        app.page_count = 2;
        app.now_playing = Some(item("BV1"));
        let request = app.auto_advance().expect("repeat the page");
        assert_eq!(request.item.bvid, "BV1");
        assert_eq!(request.page_index, 0);
    }

    #[test]
    fn failed_loads_skip_to_the_next_queue_entry() {
        let mut app = queue(PlayMode::Sequential);
        let request = app.skip_failed().expect("try BV2");
        assert_eq!(request.item.bvid, "BV2");

        // The second failure exhausts the two-track queue.
        assert!(app.skip_failed().is_none());
        assert!(app.status.contains("没有可播放"));
    }

    #[test]
    fn failed_loads_skip_even_in_repeat_one() {
        let mut app = queue(PlayMode::RepeatOne);
        let request = app
            .skip_failed()
            .expect("repeat-one should still skip a broken track");
        assert_eq!(request.item.bvid, "BV2");
    }

    #[test]
    fn shuffle_visits_every_track_before_repeating() {
        let mut app = App::new();
        app.play_mode = PlayMode::Shuffle;
        app.replace_results(
            (0..6).map(|index| item(&format!("BV{index}"))).collect(),
            "测试",
            None,
        );
        app.play_at(0);

        let mut seen = vec!["BV0".to_string()];
        for _ in 0..5 {
            let request = app.auto_advance().expect("shuffle should continue");
            seen.push(request.item.bvid);
        }
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 6, "shuffle should cover the queue once");

        // A new pass starts instead of stopping.
        assert!(app.auto_advance().is_some());
    }

    #[test]
    fn shuffle_previous_walks_the_history() {
        let mut app = App::new();
        app.play_mode = PlayMode::Shuffle;
        app.replace_results(vec![item("BV1"), item("BV2"), item("BV3")], "测试", None);
        app.play_at(0);

        let next = app.step(1).expect("shuffle next");
        assert_ne!(next.item.bvid, "BV1");
        let previous = app.step(-1).expect("shuffle previous");
        assert_eq!(previous.item.bvid, "BV1");
    }

    #[test]
    fn multi_page_videos_auto_continue_inside_any_mode() {
        let mut app = queue(PlayMode::Sequential);
        app.page = 0;
        app.page_count = 2;
        app.now_playing = Some(item("BV1"));

        let request = app.auto_advance().expect("pages should keep playing");
        assert_eq!(request.item.bvid, "BV1");
        assert_eq!(request.page_index, 1);
    }
}
