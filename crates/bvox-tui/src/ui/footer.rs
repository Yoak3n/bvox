//! Key hint footer.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use bvox_core::app::{App, InputMode, MainView};
pub(super) fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
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
        " ↑/↓ 选择  Enter 打开  n 新建  d 删除  / 搜索  ? 帮助  q 退出 "
    } else if app.current_playlist.is_some() {
        " space 暂停  n/p 切歌  a 加入歌单  d 移除  l 歌词  ? 帮助  q 退出 "
    } else {
        " Enter 播放  space 暂停  n/p 切歌  l 歌词  f 歌单  / 搜索  ? 帮助  q 退出 "
    };
    frame.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}
