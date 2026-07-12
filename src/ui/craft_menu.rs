//! 制作弹窗 UI：配方选择 / 制作进度动画

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph};
use ratatui::Frame;

use crate::app::{App, CraftMenuState};
use crate::components::{CraftingState, LightLevel};
use crate::systems::crafting::{can_craft, wip_info_at, CraftCheck, RECIPES};

/// 弹窗尺寸
const POPUP_W: u16 = 52;
const POPUP_H: u16 = 18;

/// 旋转动画帧
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn draw(frame: &mut Frame, app: &App, parent_area: Rect) {
    let Some(craft_menu) = &app.craft_menu else {
        return;
    };

    // 居中弹窗
    let popup_area = centered_rect(parent_area, POPUP_W, POPUP_H);

    // 先清背景再画内容
    frame.render_widget(Clear, popup_area);

    match craft_menu {
        CraftMenuState::Browsing { cursor, scroll } => draw_browsing(frame, app, popup_area, *cursor, *scroll),
        CraftMenuState::Crafting { spinner_frame } => {
            draw_crafting(frame, app, popup_area, *spinner_frame)
        }
    }
}

fn draw_browsing(frame: &mut Frame, app: &App, area: Rect, cursor: usize, scroll: usize) {
    let (_cx, _cy) = app.actor_pos();
    let light = app.actor_light();
    let has_fire = app.has_fire_adjacent(app.actor_pos().0, app.actor_pos().1);

    let mut lines: Vec<Line> = Vec::new();

    // 配方列表
    let (ax, ay) = (app.actor_pos().0, app.actor_pos().1);

    for (i, recipe) in RECIPES.iter().enumerate() {
        if i < scroll { continue; }
        if lines.len() > 12 { break; }
        let check = can_craft(app, i);
        let is_selected = i == cursor;

        let prefix = if is_selected { "▶ " } else { "  " };

        // 检测脚下是否有该配方的半成品
        let has_wip = wip_info_at(app, ax, ay)
            .map(|(ri, _)| ri == i)
            .unwrap_or(false);
        let wip_suffix = if has_wip { " [续作]" } else { "" };

        let (style, suffix) = match check {
            CraftCheck::Ok => (
                if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
                wip_suffix.to_string(),
            ),
            CraftCheck::TooDark => (
                Style::default().fg(Color::DarkGray),
                wip_suffix.to_string(),
            ),
            CraftCheck::NeedFire => (
                Style::default().fg(Color::DarkGray),
                wip_suffix.to_string(),
            ),
            CraftCheck::MissingMaterials => (
                Style::default().fg(Color::DarkGray),
                wip_suffix.to_string(),
            ),
            CraftCheck::Invalid => (Style::default().fg(Color::DarkGray), String::new()),
        };

        let ingredients: Vec<String> = recipe
            .ingredients
            .iter()
            .map(|(item, n)| format!("{}×{}", item.label(), n))
            .collect();
        let ing_text = ingredients.join(" + ");

        let line_text = format!("{} {:<12} {}", prefix, recipe.name, ing_text);

        lines.push(Line::from(vec![
            Span::styled(line_text, style),
            Span::styled(suffix, Style::default().fg(Color::DarkGray)),
        ]));
        // 选中项展开描述
        if is_selected {
            lines.push(Line::from(Span::styled(
                format!("    {}", recipe.desc),
                Style::default().fg(Color::Rgb(180, 180, 160)),
            )));
        }
    }

    // 分隔线
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            format!("光照: {} [{}]", light.label(), light as u8),
            if light.can_craft() {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            },
        ),
    ]));
    lines.push(Line::from(vec![Span::raw(format!(
        "篝火邻格: {}",
        if has_fire { "是" } else { "否" }
    ))]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "↑↓选择  Enter制作  +−调速  Esc取消",
        Style::default().fg(Color::DarkGray),
    )]));

    let block = Block::default()
        .title(" 制作 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_crafting(frame: &mut Frame, app: &App, area: Rect, spinner_frame: u32) {
    let recipe_index = app
        .actor()
        .and_then(|e| app.world.get::<&CraftingState>(e).ok())
        .map(|cs| cs.recipe_index);

    let (recipe_name, craft_desc, progress, total, is_dark) =
        if let Some(idx) = recipe_index {
            let recipe = &RECIPES[idx];
            let cs = app
                .actor()
                .and_then(|e| app.world.get::<&CraftingState>(e).ok());
            let (prog, tot) = cs
                .map(|cs| (cs.progress, recipe.base_progress))
                .unwrap_or((0, recipe.base_progress));
            let light = LightLevel::from_u8(app.tile_light(app.actor_pos().0, app.actor_pos().1));
            let dark = !light.can_craft();
            (recipe.name, recipe.craft_desc, prog, tot, dark)
        } else {
            ("?", "...", 0u32, 1u32, false)
        };

    let spinner_char = SPINNER[(spinner_frame as usize) % SPINNER.len()];
    let pct = if total > 0 {
        ((progress as f64 / total as f64) * 100.0) as u32
    } else {
        0
    };

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // 标题
            Constraint::Length(1),  // 空
            Constraint::Length(1),  // 进度条
            Constraint::Length(1),  // 百分比
            Constraint::Length(2),  // 描述（可能换行，给双行）
            Constraint::Length(1),  // 空
            Constraint::Length(1),  // 提示
        ])
        .split(inner_rect(area, 2, 1));

    // 标题行
    let title_line = if is_dark {
        Line::from(vec![
            Span::styled(
                format!("{} {}", spinner_char, recipe_name),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(" (太暗暂停)", Style::default().fg(Color::Red)),
        ])
    } else {
        Line::from(vec![Span::styled(
            format!("{} {}", spinner_char, recipe_name),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )])
    };
    frame.render_widget(Paragraph::new(title_line), inner[0]);

    // 进度条
    let ratio = if total > 0 {
        progress as f64 / total as f64
    } else {
        0.0
    };
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Yellow))
        .ratio(ratio);
    frame.render_widget(gauge, inner[2]);

    // 百分比
    let pct_text = format!("{}%  {}/{}", pct, progress, total);
    frame.render_widget(
        Paragraph::new(pct_text).style(Style::default().fg(Color::White)),
        inner[3],
    );

    // 描述
    let desc_text = if is_dark {
        "太暗了，无法继续..."
    } else {
        craft_desc
    };
    frame.render_widget(
        Paragraph::new(desc_text).style(Style::default().fg(Color::Gray)),
        inner[4],
    );

    // 操作提示
    frame.render_widget(
        Paragraph::new("Esc 中断 (材料不退)")
            .style(Style::default().fg(Color::DarkGray)),
        inner[5],
    );

    // 外框
    let block = Block::default()
        .title(" 制作中 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(block, area);
}

/// 在 parent 中居中一个 w×h 的矩形
fn centered_rect(parent: Rect, w: u16, h: u16) -> Rect {
    let x = parent.x + (parent.width.saturating_sub(w) / 2);
    let y = parent.y + (parent.height.saturating_sub(h) / 2);
    Rect::new(x, y, w.min(parent.width), h.min(parent.height))
}

/// 从 area 向内缩进 (dx, dy) 各边
fn inner_rect(area: Rect, dx: u16, dy: u16) -> Rect {
    Rect::new(
        area.x + dx,
        area.y + dy,
        area.width.saturating_sub(dx * 2),
        area.height.saturating_sub(dy * 2),
    )
}
