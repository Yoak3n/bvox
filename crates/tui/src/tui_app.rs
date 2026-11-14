//! TUI-side state: the keymap and render-only bits (table selection state,
//! decoded cover image). Session state lives in `bvox_core::app::App`.

use std::sync::Arc;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

use crate::cover::{CoverSet, from_dynamic};
use bvox_core::api::{AudioQuality, FavoriteFolder, PlayMode};
use bvox_core::app::{App, InputMode, MainView, PlayRequest};
use bvox_core::cache::CacheMode;

pub enum Action {
    None,
    Quit,
    Search(String),
    LoadMoreSearch,
    ImportBv(String),
    Play(PlayRequest),
    Pause,
    Seek(f64),
    Volume(f64),
    SetQuality(AudioQuality),
    SetPlayMode(PlayMode),
    Login(u64),
    OpenLibrary,
    OpenQueue,
    OpenPlaylist(FavoriteFolder),
    CreatePlaylist(String),
    CancelPendingImport,
    RenamePlaylist { folder_id: i64, title: String },
    DeletePlaylist(FavoriteFolder),
    SetPlaylistHidden { folder_id: i64, hidden: bool },
    ToggleShowHidden,
    RefreshLyrics,
    SetCacheMode(CacheMode),
    ClearCache,
    BeginAddTrack(String),
    AddToPlaylist { folder_id: i64, bvid: String },
    RemoveFromPlaylist { folder_id: i64, bvid: String },
}

/// Destructive action waiting for `y`/Enter confirmation.
pub enum ConfirmAction {
    DeletePlaylist(FavoriteFolder),
    RemoveTrack {
        folder_id: i64,
        bvid: String,
        title: String,
    },
    ClearCache,
}

impl ConfirmAction {
    pub fn message(&self) -> String {
        match self {
            Self::DeletePlaylist(folder) => {
                format!("删除歌单「{}」？视频不会被删除。", folder.title)
            }
            Self::RemoveTrack { title, .. } => {
                format!("从当前歌单移除「{title}」？")
            }
            Self::ClearCache => "清理全部缓存？正在播放的缓存文件会保留。".to_string(),
        }
    }
}

pub struct TuiState {
    pub picker: Option<Arc<Picker>>,
    pub table_state: TableState,
    /// View the table state belongs to, so switching lists resets scrolling.
    view: MainView,
    /// `bvid#cid#page` the stored cover data belongs to.
    cover_key: String,
    pub cover: Option<CoverSet>,
    pub cover_protocol: Option<StatefulProtocol>,
    pub confirm: Option<ConfirmAction>,
    pub show_help: bool,
}

impl TuiState {
    pub fn new(picker: Option<Arc<Picker>>) -> Self {
        Self {
            picker,
            table_state: TableState::default(),
            view: MainView::Queue,
            cover_key: String::new(),
            cover: None,
            cover_protocol: None,
            confirm: None,
            show_help: false,
        }
    }

    /// Mirror the core selection into the ratatui table state and drop stale
    /// cover data when the playing track changed.
    pub fn sync_selection(&mut self, app: &App) {
        if app.view != self.view {
            self.table_state = TableState::default();
            self.view = app.view;
        }
        let selection = match app.view {
            MainView::Library => app.playlist_selected,
            MainView::Queue => app.selected,
        };
        self.table_state.select(selection);
        if self.cover_key != app.cover_key {
            self.clear_cover();
            self.cover_key = app.cover_key.clone();
        }
    }

    /// Decode cover bytes and prepare whichever protocol this terminal
    /// supports (native graphics or half-block fallback).
    pub fn apply_cover(&mut self, key: &str, bytes: &[u8]) {
        self.cover_key = key.to_string();
        self.cover = None;
        self.cover_protocol = None;

        let Ok(image) = image::load_from_memory(bytes) else {
            return;
        };

        if let Some(picker) = &self.picker
            && picker.protocol_type() != ProtocolType::Halfblocks
        {
            self.cover_protocol = Some(picker.new_resize_protocol(image));
            return;
        }

        let small = from_dynamic(&image, 56, 32);
        let large = from_dynamic(&image, 80, 46);
        if let (Ok(small), Ok(large)) = (small, large) {
            self.cover = Some(CoverSet { small, large });
        }
    }

    /// Drop cover data (new track started).
    pub fn clear_cover(&mut self) {
        self.cover = None;
        self.cover_protocol = None;
    }
}

pub fn handle_key(app: &mut App, tui: &mut TuiState, key: KeyEvent) -> Action {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }

    if tui.show_help {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
        ) {
            tui.show_help = false;
        }
        return Action::None;
    }

    if tui.confirm.is_some() {
        return handle_confirm_key(app, tui, key);
    }
    if app.login.is_some() {
        return handle_login_key(app, key);
    }
    if app.picker.is_some() {
        return handle_picker_key(app, key);
    }
    if app.input_mode != InputMode::None {
        return handle_input_key(app, key);
    }

    match key.code {
        KeyCode::Char('q') => return Action::Quit,
        KeyCode::Char('?') => {
            tui.show_help = true;
            return Action::None;
        }
        KeyCode::Char('/') | KeyCode::Char('s') => {
            start_input(app, InputMode::Search, "输入关键词，Enter 搜索，Esc 取消");
            return Action::None;
        }
        KeyCode::Char('b') => {
            start_input(app, InputMode::Bv, "输入 BV 号或视频链接，Enter 导入");
            return Action::None;
        }
        KeyCode::Char('g') => return Action::Login(app.open_login()),
        KeyCode::Char('f') => return Action::OpenLibrary,
        KeyCode::Char('l') => {
            if app.now_playing.is_none() {
                app.set_status("还没有播放内容", true);
            } else {
                app.show_lyrics = !app.show_lyrics;
            }
            return Action::None;
        }
        KeyCode::Char('L') => return Action::RefreshLyrics,
        KeyCode::Char(' ') => {
            if !app.has_media {
                return Action::None;
            }
            app.paused = !app.paused;
            return Action::Pause;
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.volume = (app.volume + 5.0).min(150.0);
            return Action::Volume(app.volume);
        }
        KeyCode::Char('-') => {
            app.volume = (app.volume - 5.0).max(0.0);
            return Action::Volume(app.volume);
        }
        KeyCode::Char('o') => return Action::SetQuality(app.quality_pref.next()),
        KeyCode::Char('m') => return Action::SetPlayMode(app.play_mode.next()),
        KeyCode::Char('c') => return Action::SetCacheMode(app.cache_mode.next()),
        KeyCode::Char('C') => {
            request_confirm(app, tui, ConfirmAction::ClearCache);
            return Action::None;
        }
        KeyCode::Left => {
            if !app.has_media {
                return Action::None;
            }
            app.position = (app.position - 5.0).max(0.0);
            return Action::Seek(-5.0);
        }
        KeyCode::Right => {
            if !app.has_media {
                return Action::None;
            }
            app.position += 5.0;
            return Action::Seek(5.0);
        }
        _ => {}
    }

    match app.view {
        MainView::Library => handle_library_key(app, tui, key),
        MainView::Queue => handle_queue_key(app, tui, key),
    }
}

fn handle_library_key(app: &mut App, tui: &mut TuiState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_playlist_selection(-1);
            Action::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_playlist_selection(1);
            Action::None
        }
        KeyCode::Enter => app
            .selected_playlist()
            .map(Action::OpenPlaylist)
            .unwrap_or(Action::None),
        KeyCode::Char('n') => {
            app.rename_target = None;
            start_input(
                app,
                InputMode::NewPlaylist,
                "输入新歌单名，Enter 创建，Esc 取消",
            );
            Action::None
        }
        KeyCode::Char('r') => {
            let Some(folder) = app.selected_playlist() else {
                app.set_status("请先选择一个歌单", true);
                return Action::None;
            };
            app.rename_target = Some(folder);
            start_input(
                app,
                InputMode::RenamePlaylist,
                "输入新名称，Enter 确认，Esc 取消",
            );
            Action::None
        }
        KeyCode::Char('d') => {
            let Some(folder) = app.selected_playlist() else {
                app.set_status("请先选择一个歌单", true);
                return Action::None;
            };
            request_confirm(app, tui, ConfirmAction::DeletePlaylist(folder));
            Action::None
        }
        KeyCode::Char('h') => {
            let Some(folder) = app.selected_playlist() else {
                app.set_status("请先选择一个歌单", true);
                return Action::None;
            };
            let hidden = app.is_hidden(folder.id);
            Action::SetPlaylistHidden {
                folder_id: folder.id,
                hidden: !hidden,
            }
        }
        KeyCode::Char('H') => Action::ToggleShowHidden,
        KeyCode::Esc => Action::OpenQueue,
        _ => Action::None,
    }
}

fn handle_queue_key(app: &mut App, tui: &mut TuiState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_selection(-1);
            Action::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_selection(1);
            // Reaching the last loaded row pulls in the next search page.
            if app.has_more_results()
                && !app.loading_more
                && app.selected == app.results.len().checked_sub(1)
            {
                return Action::LoadMoreSearch;
            }
            Action::None
        }
        KeyCode::Enter => {
            if app.pending_play {
                return Action::None;
            }
            app.play_selected()
                .map(Action::Play)
                .unwrap_or(Action::None)
        }
        KeyCode::Char('n') => {
            if app.pending_play {
                return Action::None;
            }
            app.step(1).map(Action::Play).unwrap_or(Action::None)
        }
        KeyCode::Char('p') => {
            if app.pending_play {
                return Action::None;
            }
            app.step(-1).map(Action::Play).unwrap_or(Action::None)
        }
        KeyCode::Char('a') => {
            let Some(index) = app.selected else {
                app.set_status("请先选择一首歌", true);
                return Action::None;
            };
            let Some(item) = app.results.get(index) else {
                return Action::None;
            };
            Action::BeginAddTrack(item.bvid.clone())
        }
        KeyCode::Char('d') => {
            let Some(folder) = app.current_playlist.clone() else {
                app.set_status("当前列表不是歌单，无法移除", true);
                return Action::None;
            };
            let Some(index) = app.selected else {
                app.set_status("请先选择一首歌", true);
                return Action::None;
            };
            let Some(item) = app.results.get(index) else {
                return Action::None;
            };
            let confirm = ConfirmAction::RemoveTrack {
                folder_id: folder.id,
                bvid: item.bvid.clone(),
                title: item.title.clone(),
            };
            request_confirm(app, tui, confirm);
            Action::None
        }
        KeyCode::Char('r') => {
            if app.pending_play {
                return Action::None;
            }
            let item = app.now_playing.clone().or_else(|| {
                app.current
                    .and_then(|index| app.results.get(index))
                    .cloned()
            });
            if let Some(item) = item {
                app.pending_play = true;
                app.set_status("重新获取播放地址…", false);
                Action::Play(PlayRequest {
                    item,
                    page_index: app.page,
                    start_at: 0.0,
                })
            } else {
                Action::None
            }
        }
        KeyCode::Esc => Action::OpenLibrary,
        _ => Action::None,
    }
}

fn handle_confirm_key(app: &mut App, tui: &mut TuiState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => {
            let Some(confirm) = tui.confirm.take() else {
                return Action::None;
            };
            match confirm {
                ConfirmAction::DeletePlaylist(folder) => {
                    app.set_status("正在删除歌单…", false);
                    Action::DeletePlaylist(folder)
                }
                ConfirmAction::RemoveTrack {
                    folder_id, bvid, ..
                } => {
                    app.set_status("正在从歌单移除…", false);
                    Action::RemoveFromPlaylist { folder_id, bvid }
                }
                ConfirmAction::ClearCache => {
                    app.set_status("正在清理缓存…", false);
                    Action::ClearCache
                }
            }
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            tui.confirm = None;
            app.set_status("已取消", false);
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_picker_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            let cancel_import = app.pending_import.is_some();
            app.close_picker();
            if cancel_import {
                Action::CancelPendingImport
            } else {
                Action::None
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_picker(-1);
            Action::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_picker(1);
            Action::None
        }
        KeyCode::Enter => {
            let Some(bvid) = app.picker.as_ref().map(|picker| picker.target_bvid.clone()) else {
                return Action::None;
            };
            let Some(folder) = app.picker_folder() else {
                if app.pending_import.is_some() {
                    // The final "new playlist" row: reuse the text input and
                    // let the create flow consume the parked import.
                    app.picker = None;
                    app.rename_target = None;
                    start_input(
                        app,
                        InputMode::NewPlaylist,
                        "输入新歌单名，Enter 创建并加入，Esc 仅本地",
                    );
                    return Action::None;
                }
                app.set_status("没有可用的歌单，先新建一个", true);
                return Action::None;
            };
            app.picker = None;
            Action::AddToPlaylist {
                folder_id: folder.id,
                bvid,
            }
        }
        _ => Action::None,
    }
}

fn handle_login_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            app.login_seq += 1; // invalidate the polling task
            app.login = None;
            app.set_status("已取消登录", false);
            Action::None
        }
        KeyCode::Char('r') => Action::Login(app.open_login()),
        _ => Action::None,
    }
}

fn handle_input_key(app: &mut App, key: KeyEvent) -> Action {
    let mode = app.input_mode;
    match key.code {
        KeyCode::Enter => {
            let text = app.input.trim().to_string();
            if text.is_empty() {
                app.set_status("请输入内容", true);
                return Action::None;
            }
            app.input_mode = InputMode::None;
            match mode {
                InputMode::Search => Action::Search(text),
                InputMode::Bv => {
                    app.set_status("正在导入 BV…", false);
                    Action::ImportBv(text)
                }
                InputMode::NewPlaylist => {
                    app.set_status("正在创建歌单…", false);
                    Action::CreatePlaylist(text)
                }
                InputMode::RenamePlaylist => {
                    let Some(folder) = app.rename_target.take() else {
                        app.set_status("没有待重命名的歌单", true);
                        return Action::None;
                    };
                    app.set_status("正在重命名…", false);
                    Action::RenamePlaylist {
                        folder_id: folder.id,
                        title: text,
                    }
                }
                InputMode::None => Action::None,
            }
        }
        KeyCode::Esc => {
            let cancel_import = app.pending_import.is_some();
            app.input_mode = InputMode::None;
            app.rename_target = None;
            if cancel_import {
                Action::CancelPendingImport
            } else {
                app.set_status("已取消", false);
                Action::None
            }
        }
        KeyCode::Backspace => {
            app.input.pop();
            Action::None
        }
        KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.push(character);
            Action::None
        }
        _ => Action::None,
    }
}

fn start_input(app: &mut App, mode: InputMode, status: &str) {
    app.input_mode = mode;
    app.input.clear();
    app.set_status(status, false);
}

/// Start a destructive confirmation. The message is shown by the UI; `y`
/// executes the returned action.
pub fn request_confirm(app: &mut App, tui: &mut TuiState, confirm: ConfirmAction) {
    app.set_status(&format!("{}  y 确认 / n 取消", confirm.message()), false);
    tui.confirm = Some(confirm);
}
