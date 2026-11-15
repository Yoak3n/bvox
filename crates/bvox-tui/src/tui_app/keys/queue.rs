//! Song queue keys: play, seek, paging, lyrics.

//! TUI-side state: the keymap and render-only bits (table selection state,
//! decoded cover image). Session state lives in `bvox_core::app::App`.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::super::{Action, ConfirmAction, TuiState, request_confirm};
use bvox_core::app::{App, PlayRequest};

pub(in crate::tui_app) fn handle_queue_key(
    app: &mut App,
    tui: &mut TuiState,
    key: KeyEvent,
) -> Action {
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
            if app.current_playlist_public {
                app.set_status("公开收藏夹只读，不能移除歌曲", true);
                return Action::None;
            }
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
