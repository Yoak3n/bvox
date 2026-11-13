use image::imageops::FilterType;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Cell, Clear, Gauge, List, ListItem, ListState, Paragraph, Row, Table,
};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use unicode_width::UnicodeWidthStr;

use crate::cover::Cover;
use crate::tui_app::TuiState;
use bvox_core::api::{AudioQuality, format_secs};
use bvox_core::app::{App, InputMode, LoginModal, MainView};
use bvox_core::cache::{CacheMode, format_bytes};

#[derive(Clone, Copy, PartialEq, Eq)]
enum CoverLayout {
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
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let status_style = if app.error {
        Style::default().fg(Color::Red)
    } else if app.loading {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let mut spans = vec![
        Span::styled(
            " ♪ Bvox ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
    ];
    if let Some(user) = &app.user {
        spans.push(Span::styled(
            format!("@{user}  "),
            Style::default().fg(Color::Green),
        ));
        if app.vip {
            spans.push(Span::styled(
                "大会员  ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }
    spans.push(Span::styled(&app.status, status_style));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_results(frame: &mut Frame, app: &App, tui: &mut TuiState, area: Rect) {
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

fn draw_library(frame: &mut Frame, app: &App, tui: &mut TuiState, area: Rect) {
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

fn draw_lyrics(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::bordered().title(format!(
        " 歌词 · {} ",
        app.now_playing
            .as_ref()
            .map(|item| item.title.as_str())
            .unwrap_or("未播放")
    ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.lyrics.is_empty() {
        let message = if app.lyrics_status.is_empty() {
            "暂无歌词"
        } else {
            &app.lyrics_status
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(message),
                Line::from(Span::styled(
                    "按 L 重新拉取歌词",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    // Keep one line at the bottom for the re-fetch hint so a wrong lyric is
    // recoverable without knowing the keymap by heart.
    let show_hint = inner.height > 2;
    let lyrics_area = if show_hint {
        Rect {
            height: inner.height - 1,
            ..inner
        }
    } else {
        inner
    };

    let (cursor, found) = app.lyric_cursor();
    let height = lyrics_area.height as usize;
    let start = cursor.saturating_sub(height / 2);
    let lines = app
        .lyrics
        .iter()
        .skip(start)
        .take(height)
        .enumerate()
        .map(|(offset, line)| {
            let is_current = found && start + offset == cursor;
            let style = if is_current {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            Line::from(Span::styled(line.text.clone(), style))
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        lyrics_area,
    );
    if show_hint {
        let hint_area = Rect {
            y: inner.y + inner.height - 1,
            height: 1,
            ..inner
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                "L 重新拉取歌词",
                Style::default().fg(Color::DarkGray),
            ))
            .alignment(Alignment::Right),
            hint_area,
        );
    }
}

fn draw_player(
    frame: &mut Frame,
    app: &App,
    tui: &mut TuiState,
    area: Rect,
    cover_layout: Option<CoverLayout>,
) {
    let title = match app.cache_stats {
        Some(stats) => match app.cache_mode {
            CacheMode::Off => format!(
                " 正在播放 · 缓存已关闭 · 占用 {} ",
                format_bytes(stats.total_bytes)
            ),
            CacheMode::SmallFiles => format!(
                " 正在播放 · 仅小文件 · 占用 {} ",
                format_bytes(stats.total_bytes)
            ),
            CacheMode::Auto => format!(
                " 正在播放 · 缓存 {} / {} ",
                format_bytes(stats.total_bytes),
                format_bytes(stats.limit_bytes)
            ),
        },
        None => " 正在播放 ".to_string(),
    };
    let block = Block::bordered().title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(layout) = cover_layout {
        let cover_width = match layout {
            CoverLayout::Large => 82,
            CoverLayout::Small => 58,
        };
        let columns =
            Layout::horizontal([Constraint::Length(cover_width), Constraint::Min(20)]).split(inner);
        if let Some(protocol) = &mut tui.cover_protocol {
            let image = StatefulImage::<StatefulProtocol>::new()
                .resize(Resize::Fit(Some(FilterType::Lanczos3)));
            frame.render_stateful_widget(image, columns[0], protocol);
        } else if let Some(set) = &tui.cover {
            let cover = match layout {
                CoverLayout::Large => &set.large,
                CoverLayout::Small => &set.small,
            };
            frame.render_widget(Paragraph::new(cover_lines(cover)), columns[0]);
        }
        draw_player_text(frame, app, columns[1]);
    } else {
        draw_player_text(frame, app, inner);
    }
}

fn draw_player_text(frame: &mut Frame, app: &App, area: Rect) {
    let top = area.height.saturating_sub(3) / 2;
    let rows = Layout::vertical([
        Constraint::Length(top),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(area);

    frame.render_widget(Paragraph::new(title_line(app)), rows[1]);
    frame.render_widget(gauge(app), rows[2]);
    frame.render_widget(Paragraph::new(lyric_line(app)), rows[3]);
}

fn title_line(app: &App) -> Line<'static> {
    let page_prefix = if app.page_count > 1 {
        format!("P{}/{}  ", app.page + 1, app.page_count)
    } else {
        String::new()
    };
    let title = app
        .now_playing
        .as_ref()
        .map(|item| format!("{}  —  {}", item.title, item.author))
        .unwrap_or_else(|| "（未播放）".to_string());
    let part = if app.page_count > 1 && !app.part.is_empty() {
        format!("  ·  {}", app.part)
    } else {
        String::new()
    };
    let quality = if app.quality.is_empty() {
        String::new()
    } else if app.quality_pref == AudioQuality::Auto {
        format!("  [{}]", app.quality)
    } else {
        format!("  [{} · {}]", app.quality, app.quality_pref.label())
    };
    let cache = match app.cache_progress {
        Some((downloaded, total)) => match total.filter(|total| *total > 0) {
            Some(total) => format!("  · 缓存 {:.0}%", downloaded as f64 * 100.0 / total as f64),
            None => format!("  · 缓存 {:.1} MB", downloaded as f64 / 1_048_576.0),
        },
        None if app.cached_now => "  · 缓存".to_string(),
        None => String::new(),
    };

    Line::from(vec![
        Span::styled(
            page_prefix,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(title, Style::default().fg(Color::White)),
        Span::styled(part, Style::default().fg(Color::Gray)),
        Span::styled(quality, Style::default().fg(Color::DarkGray)),
        Span::styled(cache, Style::default().fg(Color::DarkGray)),
    ])
}

fn gauge(app: &App) -> Gauge<'static> {
    let ratio = if app.duration > 0.0 {
        (app.position / app.duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let icon = if app.paused {
        "⏸ 已暂停"
    } else {
        "▶ 播放中"
    };
    let label = format!(
        "{} / {}   {} · {}   音量 {:.0}",
        format_secs(app.position as i64),
        format_secs(app.duration as i64),
        icon,
        app.play_mode.label(),
        app.volume
    );
    Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .ratio(ratio)
        .label(label)
}

fn lyric_line(app: &App) -> Line<'static> {
    let text = if let Some(line) = app.current_lyric() {
        format!("♪ {line}")
    } else if app.lyrics.is_empty() {
        app.lyrics_status.clone()
    } else {
        String::new()
    };
    Line::from(Span::styled(text, Style::default().fg(Color::DarkGray)))
}

fn cover_lines(cover: &Cover) -> Vec<Line<'static>> {
    let row_width = cover.width;
    let mut lines = Vec::with_capacity(cover.height / 2);

    for pair in cover.pixels.chunks(row_width * 2) {
        if pair.len() < row_width * 2 {
            break;
        }
        let mut spans = Vec::with_capacity(row_width);
        for x in 0..row_width {
            let top = pair[x];
            let bottom = pair[row_width + x];
            spans.push(Span::styled(
                "▀",
                Style::default()
                    .fg(Color::Rgb(top.0, top.1, top.2))
                    .bg(Color::Rgb(bottom.0, bottom.1, bottom.2)),
            ));
        }
        lines.push(Line::from(spans));
    }

    lines
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    if app.input_mode != InputMode::None {
        let (prefix, hint) = match app.input_mode {
            InputMode::Search => ("搜索: ", "  （Enter 搜索 / Esc 取消）"),
            InputMode::Bv => ("BV/URL: ", "  （Enter 导入 / Esc 取消）"),
            InputMode::NewPlaylist => ("新歌单名: ", "  （Enter 创建 / Esc 取消）"),
            InputMode::RenamePlaylist => ("新名称: ", "  （Enter 确认 / Esc 取消）"),
            InputMode::None => unreachable!(),
        };
        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::raw(app.input.clone()),
            Span::styled(hint, Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(line), area);

        let cursor_x = area.x
            + UnicodeWidthStr::width(prefix) as u16
            + UnicodeWidthStr::width(app.input.as_str()) as u16;
        if cursor_x < area.right() {
            frame.set_cursor_position((cursor_x, area.y));
        }
        return;
    }

    let help = if app.view == MainView::Library {
        let toggle = if app.show_hidden {
            "H 收起隐藏"
        } else {
            "H 显示隐藏"
        };
        format!(
            " ↑/↓ 选择  Enter 打开  n 新建  r 改名  d 删除  h 隐藏/恢复  {toggle}  f 刷新  / 搜索  g 登录  o 音质  m 模式  c 缓存  C 清缓存  Esc 返回  q 退出 "
        )
    } else if app.current_playlist.is_some() {
        " Enter 播放  space 暂停  n/p 切歌  a 加入歌单  d 移除  f 歌单  Esc 歌单  / 搜索  l 歌词  L 重拉歌词  b 导入BV  o 音质  m 模式  c 缓存  C 清缓存  q 退出 ".to_string()
    } else {
        " Enter 播放  space 暂停  n/p 切歌  a 加入歌单  f 歌单  Esc 歌单  / 搜索  l 歌词  L 重拉歌词  b 导入BV  o 音质  m 模式  c 缓存  C 清缓存  q 退出 ".to_string()
    };
    frame.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

fn draw_login(frame: &mut Frame, modal: &LoginModal) {
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

fn draw_picker(frame: &mut Frame, app: &App) {
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

fn draw_confirm(frame: &mut Frame, tui: &TuiState) {
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

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
