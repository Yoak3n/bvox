//! Player strip: cover, title, progress gauge.

use image::imageops::FilterType;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Gauge, Paragraph};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

use super::CoverLayout;
use super::lyrics::lyric_line;
use crate::cover::Cover;
use crate::tui_app::TuiState;
use bvox_core::api::{AudioQuality, format_secs};
use bvox_core::app::App;
use bvox_core::cache::{CacheMode, format_bytes};

pub(super) fn draw_player(
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
            CacheMode::AudioOnly => format!(
                " 正在播放 · 仅音频 · 占用 {} ",
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
    // Multi-P videos already carry the 分P title as the track title (see
    // `Session::spawn_play`), so no separate part span is needed here.
    let title = app
        .now_playing
        .as_ref()
        .map(|item| format!("{}  —  {}", item.title, item.author))
        .unwrap_or_else(|| "（未播放）".to_string());
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
