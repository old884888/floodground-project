use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::components::{
    Captive, Colonist, Dead, Energy, Hands, Health, Hunger, Mood, Name, Player, Position, Thirst,
    TraitTag,
};
use crate::items::{has_pile, pile_at};
use crate::components::Pile;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    if let Some((fx, fy)) = app.focused_tile {
        draw_observe(frame, app, area, fx, fy);
        return;
    }

    let tab_name = match app.side_panel_tab {
        0 => "角色",
        1 => "双手",
        2 => "营地",
        _ => "?",
    };
    let title = format!(" {}  [{}/3] [ ]翻页 ", tab_name, app.side_panel_tab + 1);

    let lines = match app.side_panel_tab {
        0 => draw_tab_character(app),
        1 => draw_tab_hands(app),
        2 => draw_tab_camp(app),
        _ => vec![Line::from("?")],
    };

    let widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(widget, area);
}

fn draw_tab_character(app: &App) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    if let Some(entity) = app.selected {
        let name = app
            .world
            .get::<&Name>(entity)
            .map(|n| n.0.clone())
            .unwrap_or_else(|_| "?".into());

        let kind = if app.world.get::<&Dead>(entity).is_ok() {
            "（死亡）"
        } else if app.world.get::<&Player>(entity).is_ok() {
            "主角 @"
        } else if app.world.get::<&Colonist>(entity).is_ok() {
            "殖民者 C"
        } else if app.world.get::<&Captive>(entity).is_ok() {
            "俘虏 p"
        } else {
            "未知"
        };

        lines.push(Line::from(Span::styled(
            format!("{} · {}", name, kind),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        if let Ok(pos) = app.world.get::<&Position>(entity) {
            lines.push(Line::from(format!("坐标 ({}, {})", pos.x, pos.y)));
        }

        // 脚下地形
        if let Ok(pos) = app.world.get::<&Position>(entity) {
            let terrain = app.map.terrain(pos.x, pos.y);
            lines.push(Line::from(format!("地形 {}", terrain_name(terrain))));
        }

        if let Ok(t) = app.world.get::<&TraitTag>(entity) {
            lines.push(Line::from(format!("性格 {}", t.0)));
        }

        lines.push(Line::from(""));

        if let Ok(h) = app.world.get::<&Health>(entity) {
            lines.push(bar_line("HP  ", h.hp, h.max_hp, Color::Red));
        }
        if let Ok(h) = app.world.get::<&Hunger>(entity) {
            lines.push(bar_line("饥饿", h.value, 100.0, Color::Yellow));
        }
        if let Ok(t) = app.world.get::<&Thirst>(entity) {
            lines.push(bar_line("口渴", t.value, 100.0, Color::Blue));
        }
        if let Ok(e) = app.world.get::<&Energy>(entity) {
            lines.push(bar_line("精力", e.value, 100.0, Color::Cyan));
        }
        if let Ok(m) = app.world.get::<&Mood>(entity) {
            let color = if m.value < 30.0 {
                Color::Red
            } else if m.value < 50.0 {
                Color::Yellow
            } else {
                Color::Green
            };
            lines.push(bar_line("心情", m.value, 100.0, color));
        }
        if let Ok(c) = app.world.get::<&Captive>(entity) {
            lines.push(bar_line("意志", c.will, 100.0, Color::Magenta));
        }
    } else {
        lines.push(Line::from("未选中角色"));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "—— 声誉 ——",
        Style::default().fg(Color::DarkGray),
    )));
    let rep_color = if app.reputation <= -10 {
        Color::Red
    } else if app.reputation < 0 {
        Color::Yellow
    } else {
        Color::Green
    };
    lines.push(Line::from(Span::styled(
        format!("{:+}", app.reputation),
        Style::default()
            .fg(rep_color)
            .add_modifier(Modifier::BOLD),
    )));

    lines
}

fn draw_tab_hands(app: &App) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        "—— 双手 ——",
        Style::default().fg(Color::Yellow),
    )));

    if let Ok(hands) = app.world.get::<&Hands>(app.player) {
        lines.push(Line::from(format!(
            "左手: {}",
            Hands::format_hand(hands.left)
        )));
        lines.push(Line::from(format!(
            "右手: {}",
            Hands::format_hand(hands.right)
        )));
    } else {
        lines.push(Line::from("左手: ?"));
        lines.push(Line::from("右手: ?"));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "脚下:",
        Style::default().fg(Color::DarkGray),
    )));

    let (px, py) = app.actor_pos();
    if let Some(e) = pile_at(app, px, py) {
        if let Ok(pile) = app.world.get::<&Pile>(e) {
            let preview: Vec<String> = pile
                .slots
                .iter()
                .take(4)
                .map(|s| format!("{}×{}", s.item.label(), s.count))
                .collect();
            if preview.is_empty() {
                lines.push(Line::from("  (空)"));
            } else {
                lines.push(Line::from(format!("  {}", preview.join("  "))));
                if pile.len() > 4 {
                    lines.push(Line::from(format!("  …共 {} 种  按 e 查看", pile.len())));
                } else {
                    lines.push(Line::from("  按 e 查看完整"));
                }
            }
        }
    } else {
        lines.push(Line::from("  (空)"));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "图例",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(".草 ~水 ,营 T树 A岩"));
    lines.push(Line::from("%果 ^火 @你 C民 p俘"));
        lines.push(Line::from("/=木棍 o石 =木头"));
        lines.push(Line::from("w狼"));

    let _ = has_pile;
    lines
}

fn draw_tab_camp(app: &App) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "—— 营地 ——",
            Style::default().fg(Color::Cyan),
        )),
        Line::from("落脚点 · 非上帝殖民"),
        Line::from(""),
        Line::from(Span::styled(
            "殖民者",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    for (_e, (name, mood)) in app.world.query::<(&Name, &Mood)>().with::<&Colonist>().iter()
    {
        lines.push(Line::from(format!(
            "  {}  心情{:.0}",
            name.0, mood.value
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "俘虏",
        Style::default().fg(Color::DarkGray),
    )));
    let mut any = false;
    for (_e, (name, cap)) in app.world.query::<(&Name, &Captive)>().iter() {
        any = true;
        lines.push(Line::from(format!(
            "  {}  意志{:.0}",
            name.0, cap.will
        )));
    }
    if !any {
        lines.push(Line::from("  (无)"));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("篝火半径 15 · 夜里的岸"));
    lines
}

fn bar_line(label: &str, value: f32, max: f32, color: Color) -> Line<'static> {
    let width = 10;
    let filled = ((value / max) * width as f32).round() as usize;
    let filled = filled.min(width);
    let bar: String = std::iter::repeat_n('█', filled)
        .chain(std::iter::repeat_n('░', width - filled))
        .collect();
    Line::from(vec![
        Span::raw(format!("{} ", label)),
        Span::styled(bar, Style::default().fg(color)),
        Span::raw(format!(" {:>3.0}", value)),
    ])
}

/// 地形显示名
fn terrain_name(kind: crate::components::TerrainKind) -> &'static str {
    use crate::components::TerrainKind::*;
    match kind {
        Grass => "草地",
        LightForest => "疏林",
        DenseForest => "密林",
        Hill => "丘陵",
        ShallowMarsh => "浅沼",
        ShallowWater => "浅水",
        Sand => "沙地",
        Stream => "溪流",
        Water => "深水",
        Dirt => "泥土",
    }
}

fn draw_observe(frame: &mut Frame, app: &App, area: Rect, fx: i32, fy: i32) {
    let all: Vec<String> = crate::desc::describe_tile(app, fx, fy);
    let total = all.len();

    // 边框占 2 行；标题栏 + 底栏提示各占 1 行 → 可用行数 = area.height - 4
    let visible_rows = (area.height.saturating_sub(4)) as usize;
    let max_scroll = total.saturating_sub(visible_rows);
    let scroll = app.observe_scroll.min(max_scroll);

    let visible: Vec<Line> = all
        .iter()
        .skip(scroll)
        .take(visible_rows)
        .map(|s| Line::from(Span::raw(s.clone())))
        .collect();

    let mut render_lines = visible;
    // 空行填满剩余空间，然后底栏
    while render_lines.len() < visible_rows {
        render_lines.push(Line::from(""));
    }
    let scroll_hint = if max_scroll > 0 {
        format!(" [ ]滚动  {}/{}", scroll + 1, total)
    } else {
        String::new()
    };
    render_lines.push(Line::from(Span::styled(
        scroll_hint,
        Style::default().fg(Color::DarkGray),
    )));

    let title = format!(" 观察 ({},{}) X退出 ", fx, fy);
    let widget = Paragraph::new(render_lines)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );
    frame.render_widget(widget, area);
}
