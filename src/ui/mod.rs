mod craft_menu;
mod debug_popup;
mod inspect_popup;
mod log_view;
mod map_view;
mod menu;
mod side_panel;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, Screen};
use crate::components::{DamageNumber, HitFlash};
use crate::entity_kind::EntityKind;

pub fn draw(frame: &mut Frame, app: &mut App) {
    if app.screen == Screen::MainMenu {
        menu::draw(frame, app);
        return;
    }

    if app.screen == Screen::Loading {
        draw_loading(frame, app);
        return;
    }

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(8),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_header(frame, app, root[0]);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(root[1]);

    map_view::draw(frame, app, mid[0]);
    draw_build_popup(frame, app, frame.area());
    side_panel::draw(frame, app, mid[1]);
    inspect_popup::draw(frame, app, mid[0]);
    craft_menu::draw(frame, app, frame.area());
    debug_popup::draw(frame, app, frame.area());
    log_view::draw(frame, app, root[2]);
    draw_quit_menu(frame, app);
    draw_help(frame, app, root[3]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let rep_style = if app.reputation <= -10 {
        Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else if app.reputation < 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    };

    let (hh, mm) = app.clock_hm();
    let period = app.period_label();
    let period_style = match period {
        "夜晚" => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
        "黄昏" | "黎明" => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::White),
    };

    let (cx, cy) = app.actor_pos();
    let terrain_name = crate::data::terrain_def(app.map.terrain(cx, cy).key()).display_name.clone();

    let line = Line::from(vec![
        Span::styled(
            " 血壤 ",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("· Day {} · {:02}:{:02} · ", app.day, hh, mm)),
        Span::styled(period.to_string(), period_style),
        Span::raw(format!(" · {} · {} · ", app.game_mode.label(), terrain_name)),
        Span::styled(format!("声誉 {:+}", app.reputation), rep_style),
        Span::raw(format!(" · {} · ", app.weather.label())),
        Span::raw(app.speed.label().to_string()),
        if app.player_dead {
            Span::styled(" · 已死", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        } else {
            Span::raw("")
        },
    ]);

    let widget = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(widget, area);
}

fn draw_quit_menu(frame: &mut Frame, app: &App) {
    if app.saving {
        let area = frame.area();
        const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let spinner = SPINNER[(app.save_frame as usize / 2) % SPINNER.len()];
        let bar_w = 20usize;
        let filled = (app.save_frame as usize * 7).min(bar_w);
        let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(bar_w - filled));
        let lines = vec![
            Line::from(Span::styled(format!("{} 存档中...", spinner), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(Span::styled(bar, Style::default().fg(Color::Green))),
        ];
        let w = 30u16; let h = 5u16;
        let (x, y) = ((area.width - w) / 2, (area.height - h) / 2);
        let popup = Rect { x, y, width: w, height: h };
        frame.render_widget(Clear, popup);
        frame.render_widget(Paragraph::new(lines).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Yellow))), popup);
        return;
    }
    if !app.quit_menu { return; }
    let area = frame.area();
    let items = ["存档并退出", "直接退出（不存档）", "取消，继续游戏"];
    let mut lines = vec![
        Line::from(Span::styled("退出前要存档吗？", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
    ];
    for (i, &item) in items.iter().enumerate() {
        let (prefix, style) = if i as u8 == app.quit_cursor {
            ("▶ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        } else {
            ("  ", Style::default().fg(Color::White))
        };
        lines.push(Line::from(Span::styled(format!("{} {}", prefix, item), style)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("↑↓选择 Enter确认 Esc取消", Style::default().fg(Color::DarkGray))));
    let h = (lines.len() + 2) as u16;
    let w = 30u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup = Rect { x, y, width: w, height: h };
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Yellow))), popup);
}

fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let text = if app.debug_popup.is_some() {
        " [调试] ↑↓选 Enter确认 Esc关 "
    } else if app.action_lock.is_some() {
        " [锁定] 该方向键连发 · 其余方向走路退出 · Esc取消 "
    } else if app.focused_tile.is_some() {
        " [观察] 方向键移动光标 · [ ]滚动侧栏 · G瞬移 · X/Esc退出 "
    } else if app.examine_dir_prompt {
        " [查看] 按方向键选择要查看的格子 · Esc取消 "
    } else if app.examine.is_some() {
        " [查看] Enter确认 · Esc关 "
    } else if app.player_dead {
        " [已死亡] Q退出 · R让殖民者接班 "
    } else if app.craft_menu.is_some() {
        " [制作] Esc取消/中断 "
    } else if app.game_mode == crate::app::GameMode::Camp {
        " [营地占位] Tab回冒险 · Q退出 "
    } else {
        " 方向/WAS(L右) · R制作 · e查看 · .等待 · ;/1-4选人 · []侧栏 · +−调速 · G/D/C/M/T · F6 · Q "
    };
    let help = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);
}

pub fn entity_glyph(app: &App, entity: hecs::Entity) -> (char, Color) {
    if let Ok(flash) = app.world.get::<&HitFlash>(entity) {
        if flash.frames % 2 == 0 {
            return ('!', Color::Red);
        }
    }
    EntityKind::classify(app, entity).glyph()
}

/// 同格绘制优先级：数字越大越靠上
pub fn entity_draw_priority(app: &App, entity: hecs::Entity) -> u8 {
    if app.world.get::<&DamageNumber>(entity).is_ok() {
        return 200; // 浮动数字始终在最上层
    }
    EntityKind::classify(app, entity).draw_priority()
}

pub fn item_glyph(item: crate::components::ItemKind) -> (char, Color) {
    let def = crate::data::item_def(item.key());
    let color = match def.color.as_str() {
        "yellow" => Color::Yellow,
        "gray" | "grey" => Color::Gray,
        "red" => Color::Red,
        "white" => Color::White,
        "green" => Color::Green,
        "blue" => Color::Blue,
        "cyan" => Color::Cyan,
        "magenta" => Color::Magenta,
        _ => Color::White,
    };
    (def.glyph, color)
}

fn draw_build_popup(frame: &mut Frame, app: &App, area: Rect) {
    use crate::app::BuildMenuState;
    let Some(ref menu) = app.build_menu else { return };

    let mut lines: Vec<Line> = Vec::new();

    match menu {
        BuildMenuState::Building { recipe_index } => {
            let recipe = &crate::systems::building::BUILD_RECIPES[*recipe_index];
            // 读进度
            let (prog, total) = app.actor()
                .and_then(|a| app.world.get::<&crate::components::Building>(a).ok())
                .map(|b| (b.progress, b.total))
                .unwrap_or((0, 1));
            let pct = if total > 0 { (prog as f32 / total as f32 * 100.0) as u32 } else { 0 };

            // 旋转动画
            const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let spinner = SPINNER[(prog as usize / 2) % SPINNER.len()];

            // 进度条
            let bar_w = 20usize;
            let filled = (pct as usize * bar_w / 100).min(bar_w);
            let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(bar_w - filled));

            lines.push(Line::from(Span::styled(
                format!("{} {}", spinner, recipe.name),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                recipe.build_desc,
                Style::default().fg(Color::White),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(bar, Style::default().fg(Color::Green))));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("  {}%", pct),
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Esc 取消（材料不退）",
                Style::default().fg(Color::DarkGray),
            )));
        }
        BuildMenuState::Browsing { cursor, scroll } | BuildMenuState::PickingDir { cursor, scroll } => {
            let recipes = crate::systems::building::BUILD_RECIPES;
            let scroll = *scroll;
            let cursor = *cursor;
            lines.push(Line::from(Span::styled(
                "—— 建造菜单 ——",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            for (i, r) in recipes.iter().enumerate() {
                if i < scroll { continue; }
                if lines.len() > 14 { break; } // 留空间给帮助行
                let affordable = crate::systems::building::can_afford(app, i);
                let prefix = if i == cursor { "▶ " } else { "  " };
                let sty = if !affordable {
                    Style::default().fg(Color::DarkGray)
                } else if i == cursor {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let mat: Vec<String> = r.ingredients.iter()
                    .map(|(item, n)| format!("{}×{}", item.label(), n))
                    .collect();
                lines.push(Line::from(Span::styled(
                    format!("{}{}  ({})", prefix, r.name, mat.join(" + ")),
                    sty,
                )));
                // 描述行——选中项展开，未选中缩进灰色
                if i == cursor {
                    lines.push(Line::from(Span::styled(
                        format!("    {}", r.desc),
                        Style::default().fg(Color::Rgb(180, 180, 160)),
                    )));
                }
            }
            lines.push(Line::from(""));
            let hint = if matches!(menu, BuildMenuState::PickingDir { .. }) {
                "方向键选位置  Esc取消"
            } else {
                "↑↓选  Enter确认  Esc关"
            };
            lines.push(Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray))));
        }
    }

    let h = (lines.len() as u16 + 2).min(18);
    let w = 38u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + 2;
    let popup = Rect { x, y, width: w, height: h };
    frame.render_widget(Clear, popup);
    let widget = Paragraph::new(lines).block(
        Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(widget, popup);
}

fn draw_loading(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let t = app.loading_tick as f32 / 40.0; // 0.0..1.0

    let (stage_name, _pct) = if t < 0.20 {
        ("校验模板...", t / 0.20)
    } else if t < 0.45 {
        ("生成地形...", (t - 0.20) / 0.25)
    } else if t < 0.65 {
        ("散布植被...", (t - 0.45) / 0.20)
    } else if t < 0.85 {
        ("建立村庄...", (t - 0.65) / 0.20)
    } else {
        ("释放狼群...", (t - 0.85) / 0.15)
    };

    let bar_w = 30usize;
    let filled = (t * bar_w as f32) as usize;
    let bar = format!(
        "[{}{}]",
        "█".repeat(filled),
        "░".repeat(bar_w.saturating_sub(filled))
    );

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  血壤 · Bloodsoil",
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        stage_name,
        Style::default().fg(Color::Yellow),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        bar,
        Style::default().fg(Color::Green),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("  {:.0}%", t * 100.0),
        Style::default().fg(Color::DarkGray),
    )));

    let h = lines.len() as u16 + 2;
    let w = 40u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup = Rect { x, y, width: w, height: h };

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(widget, popup);
}

