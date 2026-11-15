//! TUI-side state: the keymap and render-only bits (table selection state,
//! decoded cover image). Session state lives in `bvox_core::app::App`.

use std::sync::Arc;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

use crate::cover::{CoverSet, from_dynamic};
use bvox_core::app::{App, InputMode, MainView};
mod action;
mod keys;

pub use action::{Action, ConfirmAction};
use keys::*;

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
        KeyCode::Char('F') => {
            start_input(
                app,
                InputMode::Favorite,
                "输入公开收藏夹链接或 media_id，Enter 导入",
            );
            return Action::None;
        }
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
