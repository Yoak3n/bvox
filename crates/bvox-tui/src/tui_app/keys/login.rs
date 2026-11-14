//! Login modal keys.

//! TUI-side state: the keymap and render-only bits (table selection state,
//! decoded cover image). Session state lives in `bvox_core::app::App`.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::super::Action;
use bvox_core::app::App;

pub(in crate::tui_app) fn handle_login_key(app: &mut App, key: KeyEvent) -> Action {
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
