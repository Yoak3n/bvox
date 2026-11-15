//! Add-to-playlist picker keys.

//! TUI-side state: the keymap and render-only bits (table selection state,
//! decoded cover image). Session state lives in `bvox_core::app::App`.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::super::{Action, start_input};
use bvox_core::app::{App, InputMode};

pub(in crate::tui_app) fn handle_picker_key(app: &mut App, key: KeyEvent) -> Action {
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
