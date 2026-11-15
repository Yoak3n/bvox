//! Search results table.

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Cell, Row, Table};

use crate::tui_app::TuiState;
use bvox_core::app::App;
pub(super) fn draw_results(frame: &mut Frame, app: &App, tui: &mut TuiState, area: Rect) {
    let rows = app
        .results
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let is_current = app.current == Some(index);
            let marker = if is_current { "▶" } else { " " };
            let style = if is_current {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            let cached = app.is_cached(&item.bvid);
            Row::new(vec![
                Cell::from(format!("{marker}{}", index + 1)),
                Cell::from(item.title.clone()),
                Cell::from(item.author.clone()),
                Cell::from(if cached { "✓" } else { "" }).style(Style::default().fg(Color::Green)),
                Cell::from(item.duration.clone()),
            ])
            .style(style)
        })
        .collect::<Vec<_>>();

    let title = if app.loading {
        format!(" {} · 加载中… ", app.results_title)
    } else if app.loading_more {
        format!(" {} ({}) · 加载中… ", app.results_title, app.results.len())
    } else if app.has_more_results() {
        format!(
            " {} ({}) · 下翻加载更多 ",
            app.results_title,
            app.results.len()
        )
    } else {
        format!(" {} ({}) ", app.results_title, app.results.len())
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Min(20),
            Constraint::Length(18),
            Constraint::Length(6),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(["#", "标题", "UP主", "缓存", "时长"]).style(
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(Block::bordered().title(title))
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol(" ");

    frame.render_stateful_widget(table, area, &mut tui.table_state);
}
