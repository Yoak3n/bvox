use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use crate::tui_app::TuiState;
use bvox_core::app::{App, MainView};
mod footer;
mod header;
mod help;
mod library;
mod lyrics;
mod modals;
mod player;
mod results;

use footer::draw_footer;
use header::draw_header;
use help::draw_help;
use library::draw_library;
use lyrics::draw_lyrics;
use modals::{draw_confirm, draw_login, draw_picker};
use player::draw_player;
use results::draw_results;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CoverLayout {
    Small,
    Large,
}

pub fn draw(frame: &mut Frame, app: &App, tui: &mut TuiState) {
    let area = frame.area();
    let has_cover = tui.cover_protocol.is_some() || tui.cover.is_some();
    let cover_layout = if has_cover && area.width >= 112 && area.height >= 32 {
        Some(CoverLayout::Large)
    } else if has_cover && area.width >= 96 && area.height >= 28 {
        Some(CoverLayout::Small)
    } else {
        None
    };
    let player_height = match cover_layout {
        Some(CoverLayout::Large) => 25,
        Some(CoverLayout::Small) => 18,
        None => 5,
    };
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(player_height),
        Constraint::Length(1),
    ])
    .split(area);

    draw_header(frame, app, chunks[0]);
    if app.view == MainView::Library {
        draw_library(frame, app, tui, chunks[1]);
    } else if app.show_lyrics {
        draw_lyrics(frame, app, chunks[1]);
    } else {
        draw_results(frame, app, tui, chunks[1]);
    }
    draw_player(frame, app, tui, chunks[2], cover_layout);
    draw_footer(frame, app, chunks[3]);

    if let Some(login) = &app.login {
        draw_login(frame, login);
    } else if tui.confirm.is_some() {
        draw_confirm(frame, tui);
    } else if app.picker.is_some() {
        draw_picker(frame, app);
    }
    if tui.show_help {
        draw_help(frame);
    }
}

pub(super) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
