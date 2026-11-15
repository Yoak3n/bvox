//! Playlist library and queue table.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table};

use crate::tui_app::TuiState;
use bvox_core::app::App;
pub(super) fn draw_library(frame: &mut Frame, app: &App, tui: &mut TuiState, area: Rect) {
    let playlists = app.library_playlists();
    let title = if app.playlists_loading {
        " 我的歌单 · 加载中… ".to_string()
    } else {
        format!(" 我的歌单 ({}) ", playlists.len())
    };
    let block = Block::bordered().title(title);

    if playlists.is_empty() {
        let message = if app.playlists_loading {
            "正在加载歌单…"
        } else if app.user.is_none() && !app.has_cookie {
            "登录后查看 B 站收藏夹歌单（g 登录）"
        } else if app.user.is_none() {
            "离线：暂无缓存的歌单；联网后按 f 刷新"
        } else if !app.playlists.is_empty() && !app.show_hidden {
            "所有歌单都已隐藏；按 H 显示隐藏项"
        } else {
            "还没有歌单；按 n 新建，或到 B 站网页端创建收藏夹"
        };
        frame.render_widget(
            Paragraph::new(message)
                .block(block)
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let rows = playlists
        .iter()
        .enumerate()
        .map(|(index, folder)| {
            let hidden = app.is_hidden(folder.id);
            let mut label = folder.title.clone();
            if hidden {
                label.push_str("  [已隐藏]");
            }
            let row = Row::new(vec![
                Cell::from(format!("{}", index + 1)),
                Cell::from(label),
                Cell::from(folder.count.to_string()),
            ]);
            if hidden {
                row.style(Style::default().fg(Color::DarkGray))
            } else {
                row
            }
        })
        .collect::<Vec<_>>();

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Min(20),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(["#", "歌单", "歌曲"]).style(
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(block)
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol(" ");

    frame.render_stateful_widget(table, area, &mut tui.table_state);
}
