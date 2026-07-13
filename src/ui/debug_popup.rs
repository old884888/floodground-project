use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{
    App, DebugSubKind, DEBUG_ITEMS, DEBUG_SUB_CREATURES, DEBUG_SUB_SETTLEMENTS, DEBUG_SUB_TERRAIN,
    DEBUG_SUB_TIME, DEBUG_SUB_TOOLS, DEBUG_SUB_WEATHER, DEBUG_TERRAIN_ITEMS, DEBUG_TIME_ITEMS,
    DEBUG_WEATHER_ITEMS, SPAWN_ITEMS, SETTLEMENT_SIZE_ITEMS, TOOL_ITEMS,
};

pub fn draw(frame: &mut Frame, app: &App, _map_area: Rect) {
    let Some(popup) = &app.debug_popup else {
        return;
    };

    let area = frame.area();

    if let Some(ref sub) = popup.sub {
        match sub {
            DebugSubKind::Tool => {
                draw_sub_popup(frame, area, "刷石器工具", TOOL_ITEMS, popup.sub_cursor, Color::Yellow);
            }
            DebugSubKind::Creature => {
                draw_sub_popup(frame, area, "生成生物", SPAWN_ITEMS, popup.sub_cursor, Color::Cyan);
            }
            DebugSubKind::Settlement => {
                let labels: Vec<&str> = SETTLEMENT_SIZE_ITEMS.iter().map(|(s, _)| *s).collect();
                draw_sub_popup(frame, area, "生成聚落", &labels, popup.sub_cursor, Color::Green);
            }
            DebugSubKind::TimePeriod => {
                draw_sub_popup(frame, area, "时间/日夜", DEBUG_TIME_ITEMS, popup.sub_cursor, Color::Yellow);
            }
            DebugSubKind::WeatherKind => {
                draw_sub_popup(frame, area, "天气", DEBUG_WEATHER_ITEMS, popup.sub_cursor, Color::Cyan);
            }
            DebugSubKind::TerrainItem => {
                draw_sub_popup(frame, area, "刷地形物品", DEBUG_TERRAIN_ITEMS, popup.sub_cursor, Color::Green);
            }
        }
    } else {
        draw_debug_popup(frame, area, popup.cursor);
    }
}

fn draw_debug_popup(frame: &mut Frame, area: Rect, cursor: usize) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        "—— 调试菜单 ——",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (i, item) in DEBUG_ITEMS.iter().enumerate() {
        let prefix = if i == cursor { "▶ " } else { "  " };
        let is_sub = matches!(
            i,
            DEBUG_SUB_TOOLS | DEBUG_SUB_CREATURES | DEBUG_SUB_SETTLEMENTS | DEBUG_SUB_TIME | DEBUG_SUB_WEATHER | DEBUG_SUB_TERRAIN
        );
        let style = if i == cursor {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if is_sub {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(
            format!("{}{}", prefix, item),
            style,
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑↓选  Enter确认  Esc关",
        Style::default().fg(Color::DarkGray),
    )));

    let height = (lines.len() as u16 + 2).min(area.height.saturating_sub(4));
    let width = 28u16;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    let popup_area = Rect { x, y, width, height };

    frame.render_widget(Clear, popup_area);
    let widget = Paragraph::new(lines).block(
        Block::default()
            .title(" F6 调试 ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(widget, popup_area);
}

fn draw_sub_popup(frame: &mut Frame, area: Rect, title: &str, items: &[&str], cursor: usize, color: Color) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!("—— {} ——", title),
        Style::default()
            .fg(color)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (i, item) in items.iter().enumerate() {
        let prefix = if i == cursor { "▶ " } else { "  " };
        let style = if i == cursor {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(
            format!("{}{}", prefix, item),
            style,
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "↑↓选  Enter确认  Esc返回",
        Style::default().fg(Color::DarkGray),
    )));

    let height = (lines.len() as u16 + 2).min(area.height.saturating_sub(4));
    let width = 28u16;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    let popup_area = Rect { x, y, width, height };

    frame.render_widget(Clear, popup_area);
    let widget = Paragraph::new(lines).block(
        Block::default()
            .title(format!(" {} ", title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color)),
    );
    frame.render_widget(widget, popup_area);
}
