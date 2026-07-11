use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, ExamineMenu};
use crate::components::Pile;
use crate::items::pile_at;
use crate::systems::examine;

pub fn draw(frame: &mut Frame, app: &App, map_area: Rect) {
    let Some(state) = &app.examine else {
        return;
    };

    let (tx, ty) = (state.x, state.y);
    let mut lines: Vec<Line> = Vec::new();

    match &state.menu {
        ExamineMenu::Pile => {
            if let Some(e) = pile_at(app, tx, ty) {
                if let Ok(pile) = app.world.get::<&Pile>(e) {
                    if pile.is_empty() {
                        lines.push(Line::from("（空）"));
                    } else {
                        for (i, slot) in pile.slots.iter().enumerate() {
                            let prefix = if i == state.cursor { "> " } else { "  " };
                            let style = if i == state.cursor {
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::White)
                            };
                            lines.push(Line::from(Span::styled(
                                format!(
                                    "{}{}. {}  ×{}",
                                    prefix,
                                    i + 1,
                                    slot.item.label(),
                                    slot.count
                                ),
                                style,
                            )));
                        }
                    }
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑↓选  G/Enter捡  D丢  Esc关",
                Style::default().fg(Color::DarkGray),
            )));
        }
        ExamineMenu::Action(action) => {
            lines.push(Line::from(Span::styled(
                format!("[1] {}", examine::action_label(*action)),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Enter/1/Y确认  Esc关",
                Style::default().fg(Color::DarkGray),
            )));
        }
        ExamineMenu::Empty => return,
    }

    let height = (lines.len() as u16 + 2).clamp(5, 12);
    let width = 36u16.min(map_area.width.saturating_sub(4)).max(24);
    let x = map_area.x + (map_area.width.saturating_sub(width)) / 2;
    let y = map_area.y + 1;

    let area = Rect {
        x,
        y,
        width,
        height,
    };

    frame.render_widget(Clear, area);
    let title = format!(" 查看 ({}, {}) ", tx, ty);
    let widget = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(widget, area);
}
