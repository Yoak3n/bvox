//! Library view keys: folders, playlists, add/remove.

//! TUI-side state: the keymap and render-only bits (table selection state,
//! decoded cover image). Session state lives in `bvox_core::app::App`.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::super::{Action, ConfirmAction, TuiState, request_confirm, start_input};
use bvox_core::app::{App, InputMode};

pub(in crate::tui_app) fn handle_library_key(
    app: &mut App,
    tui: &mut TuiState,
    key: KeyEvent,
) -> Action {
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
