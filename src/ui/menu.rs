//! 主菜单画面

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // 竖直居中
    let v_pad = area.height.saturating_sub(10) / 2;
    let h_pad = area.width.saturating_sub(40) / 2;
    let menu_area = Rect {
        x: area.x + h_pad,
        y: area.y + v_pad,
        width: 40.min(area.width),
        height: 10.min(area.height),
    };

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(menu_area);

    // 标题
    let title = Line::from(vec![
        Span::styled(
            "血  壤",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(title).centered(),
        inner[0],
    );

    // 副标题
    frame.render_widget(
        Paragraph::new(
            Line::from(Span::styled(
                "Bloodsoil",
                Style::default().fg(Color::DarkGray),
            )),
        )
        .centered(),
        inner[1],
    );

    // 菜单项
    let has_save = std::path::Path::new("saves/slot_01.ron.gz").exists();
    let items = ["开始游戏", "继续游戏", "退出游戏"];
    for (i, &item) in items.iter().enumerate() {
        let disabled = i == 1 && !has_save;
        let label = if disabled { "继续游戏 (无存档)" } else { item };
        let prefix = if i as u8 == app.menu.cursor { "▶ " } else { "  " };
        let style = if disabled {
            Style::default().fg(Color::DarkGray)
        } else if i as u8 == app.menu.cursor {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("{}{}", prefix, label),
                style,
            ))),
            inner[2 + i],
        );
    }

    // 底栏
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "↑↓选择  Enter确认",
            Style::default().fg(Color::DarkGray),
        )))
        .centered(),
        inner[4],
    );

    // 外框
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    frame.render_widget(block, menu_area);
}
