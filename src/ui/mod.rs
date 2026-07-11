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
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, Screen};
use crate::components::Position;
use crate::entity_kind::EntityKind;

pub fn draw(frame: &mut Frame, app: &mut App) {
    if app.screen == Screen::MainMenu {
        menu::draw(frame, app);
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
    side_panel::draw(frame, app, mid[1]);
    inspect_popup::draw(frame, app, mid[0]);
    craft_menu::draw(frame, app, frame.area());
    debug_popup::draw(frame, app, frame.area());
    log_view::draw(frame, app, root[2]);
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

    let line = Line::from(vec![
        Span::styled(
            " 血壤 ",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("· Day {} · {:02}:{:02} · ", app.day, hh, mm)),
        Span::styled(period.to_string(), period_style),
        Span::raw(format!(" · {} · ", app.game_mode.label())),
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

fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let text = if app.debug_popup.is_some() {
        " [调试] ↑↓选 Enter确认 Esc关 "
    } else if app.action_lock.is_some() {
        " [锁定] 该方向键连发 · 其余方向走路退出 · Esc取消 "
    } else if app.focused_tile.is_some() {
        " [观察] 方向键移动光标 · [ ]滚动侧栏 · X/Esc退出 "
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

#[allow(dead_code)]
pub fn cycle_selection(app: &mut App) {
    app.cycle_character();
}

pub fn entity_glyph(app: &App, entity: hecs::Entity) -> (char, Color) {
    EntityKind::classify(app, entity).glyph()
}

/// 同格绘制优先级：数字越大越靠上
pub fn entity_draw_priority(app: &App, entity: hecs::Entity) -> u8 {
    EntityKind::classify(app, entity).draw_priority()
}

pub fn item_glyph(item: crate::components::ItemKind) -> (char, Color) {
    use crate::components::ItemKind;
    match item {
        ItemKind::Wood => ('=', Color::Yellow),
        ItemKind::BigStone => ('O', Color::Gray),
        ItemKind::Stick => ('/', Color::Yellow),
        ItemKind::SmallStone => ('o', Color::Gray),
        ItemKind::Berry => ('*', Color::Red),
        ItemKind::StoneKnife => ('!', Color::White),
        ItemKind::SharpStick => ('/', Color::Red),
        ItemKind::Spear => ('↑', Color::Red),
        ItemKind::StoneAxe => ('P', Color::White),
        ItemKind::Torch => ('i', Color::Yellow),
    }
}

#[allow(dead_code)]
pub fn entities_in_view(
    app: &App,
    cam_x: i32,
    cam_y: i32,
    view_w: i32,
    view_h: i32,
) -> Vec<(i32, i32, hecs::Entity)> {
    let mut out = Vec::new();
    for (e, pos) in app.world.query::<&Position>().iter() {
        if pos.x >= cam_x
            && pos.y >= cam_y
            && pos.x < cam_x + view_w
            && pos.y < cam_y + view_h
        {
            out.push((pos.x, pos.y, e));
        }
    }
    out
}
