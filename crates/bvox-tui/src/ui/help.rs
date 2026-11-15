//! Full-screen key help overlay.

use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::centered;

pub(super) fn draw_help(frame: &mut Frame) {
    let mut lines: Vec<Line> = Vec::new();
    for (label, pairs) in [
        (
            "播放",
            &[
                ("space", "暂停/继续"),
                ("←/→", "快退/快进 5s"),
                ("n/p", "下一首/上一首"),
                ("+/-", "音量 ±5"),
                ("Enter", "播放选中"),
                ("l", "歌词开关"),
                ("L", "重拉歌词"),
                ("Esc", "队列/返回"),
            ][..],
        ),
        (
            "视图",
            &[
                ("/", "搜索"),
                ("b", "导入BV"),
                ("F", "导入公开收藏夹"),
                ("f", "歌单库/刷新"),
                ("g", "登录"),
            ][..],
        ),
        (
            "歌单",
            &[
                ("n", "新建"),
                ("r", "改名"),
                ("d", "删除/移除"),
                ("h", "隐藏/恢复"),
                ("H", "显示/收起隐藏"),
                ("a", "加入歌单"),
            ][..],
        ),
        (
            "设置",
            &[
                ("o", "音质"),
                ("m", "播放模式"),
                ("c", "缓存模式"),
                ("C", "清缓存"),
            ][..],
        ),
    ] {
        lines.extend(help_group_lines(label, pairs));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Esc 关闭",
        Style::default().fg(Color::DarkGray),
    )));

    let area = centered(frame.area(), 46, lines.len() as u16 + 2);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .title(" 键位帮助 ")
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines), inner);
}

fn help_group_lines(label: &str, pairs: &[(&str, &str)]) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        label.to_string(),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))];
    for chunk in pairs.chunks(2) {
        let mut spans = vec![Span::raw("  ")];
        for (index, (key, desc)) in chunk.iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled("   ", Style::default().fg(Color::DarkGray)));
            }
            spans.push(Span::styled(
                format!("{key:<7}"),
                Style::default().fg(Color::White),
            ));
            spans.push(Span::styled(
                (*desc).to_string(),
                Style::default().fg(Color::Gray),
            ));
        }
        lines.push(Line::from(spans));
    }
    lines
}
