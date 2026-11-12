//! TUI-side state: the keymap and render-only bits (table selection state,
//! decoded cover image). Session state lives in `bili_core::app::App`.

use std::sync::Arc;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

use bili_core::api::FavoriteFolder;
use bili_core::app::{App, InputMode, PlayRequest};
use crate::cover::{CoverSet, from_dynamic};

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

pub struct TuiState {
    pub picker: Option<Arc<Picker>>,
    pub table_state: TableState,
    /// `bvid#cid#page` the stored cover data belongs to.
    cover_key: String,
    pub cover: Option<CoverSet>,
    pub cover_protocol: Option<StatefulProtocol>,
}

impl TuiState {
    pub fn new(picker: Option<Arc<Picker>>) -> Self {
        Self {
            picker,
            table_state: TableState::default(),
            cover_key: String::new(),
            cover: None,
            cover_protocol: None,
        }
    }

    /// Mirror the core selection into the ratatui table state and drop stale
    /// cover data when the playing track changed.
    pub fn sync_selection(&mut self, app: &App) {
        self.table_state.select(app.selected);
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
    let _ = tui; // selection state mirrors `app.selected` on draw
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }

    if app.login.is_some() {
        return handle_login_key(app, key);
    }
    if app.favorites.is_some() {
        return handle_favorites_key(app, key);
    }
    if app.input_mode != InputMode::None {
        return handle_input_key(app, key);
    }

    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('/') | KeyCode::Char('s') => {
            app.input_mode = InputMode::Search;
            app.input.clear();
            app.set_status("输入关键词，Enter 搜索，Esc 取消", false);
            Action::None
        }
        KeyCode::Char('b') => {
            app.input_mode = InputMode::Bv;
            app.input.clear();
            app.set_status("输入 BV 号或视频链接，Enter 导入", false);
            Action::None
        }
        KeyCode::Char('y') => Action::SyncPull,
        KeyCode::Char('g') => Action::Login(app.open_login()),
        KeyCode::Char('f') => Action::Favorites,
        KeyCode::Char('l') => {
            if app.now_playing.is_none() {
                app.set_status("还没有播放内容", true);
            } else {
                app.show_lyrics = !app.show_lyrics;
            }
            Action::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_selection(-1);
            Action::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_selection(1);
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
        KeyCode::Char(' ') => {
            if !app.has_media {
                return Action::None;
            }
            app.paused = !app.paused;
            Action::Pause
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
        KeyCode::Left => {
            if !app.has_media {
                return Action::None;
            }
            app.position = (app.position - 5.0).max(0.0);
            Action::Seek(-5.0)
        }
        KeyCode::Right => {
            if !app.has_media {
                return Action::None;
            }
            app.position += 5.0;
            Action::Seek(5.0)
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.volume = (app.volume + 5.0).min(150.0);
            Action::Volume(app.volume)
        }
        KeyCode::Char('-') => {
            app.volume = (app.volume - 5.0).max(0.0);
            Action::Volume(app.volume)
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
                })
            } else {
                Action::None
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

fn handle_favorites_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            app.favorites = None;
            app.set_status("已关闭收藏夹", false);
            Action::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_favorite(-1);
            Action::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_favorite(1);
            Action::None
        }
        KeyCode::Enter => {
            let Some(modal) = &app.favorites else {
                return Action::None;
            };
            if modal.loading {
                return Action::None;
            }
            let Some(folder) = modal.folders.get(modal.selected.unwrap_or(0)).cloned() else {
                return Action::None;
            };
            if let Some(modal) = &mut app.favorites {
                modal.loading = true;
                modal.status = format!("正在导入「{}」…", folder.title);
            }
            app.set_status("正在导入收藏夹…", false);
            Action::ImportFolder(folder)
        }
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
                InputMode::Search => {
                    app.loading = true;
                    app.search_seq += 1;
                    app.set_status("搜索中…", false);
                    Action::Search(text)
                }
                InputMode::Bv => {
                    app.set_status("正在导入 BV…", false);
                    Action::ImportBv(text)
                }
                InputMode::None => Action::None,
            }
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::None;
            app.set_status("已取消", false);
            Action::None
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
