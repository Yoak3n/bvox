//! Modal overlays: login, add-to-playlist picker, confirmations.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};

use super::centered;
use crate::tui_app::TuiState;
use bvox_core::app::{App, LoginModal};

pub(super) fn draw_login(frame: &mut Frame, modal: &LoginModal) {
    let modules = modal.qr.len();
    let rows = modules.div_ceil(2);
    let width = modules.max(28) as u16 + 4;
    let height = rows.max(3) as u16 + 6;
    let area = centered(frame.area(), width, height);

    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .title(" 扫码登录 ")
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if modal.qr.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(modal.status.clone()),
                Line::from("Esc 关闭"),
            ])
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray)),
            inner,
        );
        return;
    }

    let parts =
        Layout::vertical([Constraint::Length(rows as u16), Constraint::Min(3)]).split(inner);
    frame.render_widget(
        Paragraph::new(qr_lines(&modal.qr)).alignment(Alignment::Center),
        parts[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(modal.status.clone()),
            Line::from("r 重新生成 · Esc 关闭"),
        ])
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray)),
        parts[1],
    );
}

fn qr_lines(matrix: &[Vec<bool>]) -> Vec<Line<'static>> {
    let size = matrix.len();
    let mut lines = Vec::with_capacity(size.div_ceil(2));

    for row in (0..size).step_by(2) {
        let top = &matrix[row];
        let bottom = matrix.get(row + 1);
        let mut spans = Vec::with_capacity(size);
        for x in 0..size {
            let top_dark = top.get(x).copied().unwrap_or(false);
            let bottom_dark = bottom
                .and_then(|line| line.get(x))
                .copied()
                .unwrap_or(false);
            let (character, foreground) = match (top_dark, bottom_dark) {
                (true, true) => ("█", Color::Black),
                (true, false) => ("▀", Color::Black),
                (false, true) => ("▄", Color::Black),
                (false, false) => (" ", Color::White),
            };
            spans.push(Span::styled(
                character,
                Style::default().fg(foreground).bg(Color::White),
            ));
        }
        lines.push(Line::from(spans));
    }

    lines
}

pub(super) fn draw_picker(frame: &mut Frame, app: &App) {
    let playlists = app.library_playlists();
    let show_new = app.pending_import.is_some();
    let item_count = playlists.len() + usize::from(show_new);
    let height = (item_count.max(1) as u16 + 6).clamp(8, 24);
    let area = centered(frame.area(), 64, height);

    frame.render_widget(Clear, area);
    let target_title = app
        .picker
        .as_ref()
        .and_then(|picker| {
            app.results
                .iter()
                .find(|item| item.bvid == picker.target_bvid)
        })
        .map(|item| item.title.as_str())
        .unwrap_or("所选歌曲");
    let block = Block::bordered()
        .title(format!(" 加入歌单 · {target_title} "))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let parts = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(inner);

    let mut items = playlists
        .iter()
        .map(|folder| ListItem::new(Line::from(format!("{}  ({})", folder.title, folder.count))))
        .collect::<Vec<_>>();
    if show_new {
        items.push(ListItem::new(Line::from("＋ 新建歌单")));
    }
    if items.is_empty() {
        items.push(ListItem::new("（没有歌单）"));
    }

    let selected = if item_count == 0 {
        None
    } else {
        app.picker.as_ref().and_then(|picker| picker.selected)
    };

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    let mut state = ListState::default();
    state.select(selected);
    frame.render_stateful_widget(list, parts[0], &mut state);

    let footer = if show_new {
        "Enter 加入/新建 · Esc 取消"
    } else {
        "Enter 加入 · Esc 取消"
    };
    frame.render_widget(
        Paragraph::new(vec![Line::from(footer)]).style(Style::default().fg(Color::DarkGray)),
        parts[1],
    );
}

pub(super) fn draw_confirm(frame: &mut Frame, tui: &TuiState) {
    let Some(confirm) = &tui.confirm else {
        return;
    };
    let area = centered(frame.area(), 56, 7);

    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .title(" 请确认 ")
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(
        Paragraph::new(confirm.message())
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::White)),
        lines[0],
    );
    frame.render_widget(
        Paragraph::new("y / Enter 确认 · n / Esc 取消")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray)),
        lines[1],
    );
}
