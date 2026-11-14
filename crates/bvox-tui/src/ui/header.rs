//! Top status bar.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use bvox_core::app::App;
pub(super) fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
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
