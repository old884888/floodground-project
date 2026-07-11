use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let inner_h = area.height.saturating_sub(2) as usize;
    let start = app.log.len().saturating_sub(inner_h);

    let lines: Vec<Line> = app.log[start..]
        .iter()
        .map(|s| {
            if s.contains("[警告]") {
                Line::from(Span::styled(s.clone(), Style::default().fg(Color::Red)))
            } else if s.starts_with("——") {
                Line::from(Span::styled(
                    s.clone(),
                    Style::default().fg(Color::DarkGray),
                ))
            } else {
                Line::from(format!("> {}", s))
            }
        })
        .collect();

    let widget = Paragraph::new(lines).block(
        Block::default()
            .title(" 事件日志 ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(widget, area);
}
