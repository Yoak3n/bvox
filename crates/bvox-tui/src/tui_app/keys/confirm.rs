//! Confirmation dialog keys.

//! TUI-side state: the keymap and render-only bits (table selection state,
//! decoded cover image). Session state lives in `bvox_core::app::App`.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::super::{Action, ConfirmAction, TuiState};
use bvox_core::app::App;

pub(in crate::tui_app) fn handle_confirm_key(
    app: &mut App,
    tui: &mut TuiState,
    key: KeyEvent,
) -> Action {
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
