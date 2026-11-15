//! Text input keys (search / rename / import).

//! TUI-side state: the keymap and render-only bits (table selection state,
//! decoded cover image). Session state lives in `bvox_core::app::App`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::super::Action;
use bvox_core::app::{App, InputMode};

pub(in crate::tui_app) fn handle_input_key(app: &mut App, key: KeyEvent) -> Action {
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
