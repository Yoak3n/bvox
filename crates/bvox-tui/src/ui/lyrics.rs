//! Lyrics pane.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use bvox_core::app::App;
pub(super) fn draw_lyrics(frame: &mut Frame, app: &App, area: Rect) {
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

pub(super) fn lyric_line(app: &App) -> Line<'static> {
    let text = if let Some(line) = app.current_lyric() {
        format!("♪ {line}")
    } else if app.lyrics.is_empty() {
        app.lyrics_status.clone()
    } else {
        String::new()
    };
    Line::from(Span::styled(text, Style::default().fg(Color::DarkGray)))
}
