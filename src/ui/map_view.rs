use rand::Rng;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, RainDrop};
use crate::ui::{entity_draw_priority, entity_glyph};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let inner_w = area.width.saturating_sub(2) as i32;
    let inner_h = area.height.saturating_sub(2) as i32;

    let target = app.focused_tile.unwrap_or_else(|| app.player_pos());
    app.camera
        .follow(target, app.map.width, app.map.height, inner_w, inner_h);
    // 视野/解雾锚点永远是玩家（X 模式相机可以飞到迷雾深处，但视野不外延）
    let viewer = app.player_pos();

    let cam_x = app.camera.x;
    let cam_y = app.camera.y;
    let view_w = inner_w.max(0) as usize;
    let view_h = inner_h.max(0) as usize;
    let nightish = app.day_progress() >= 0.80 || app.day_progress() < 0.25;

    let mut entity_map: Vec<Option<(hecs::Entity, u8)>> =
        vec![None; view_w.max(1) * view_h.max(1)];
    if view_w > 0 && view_h > 0 {
        for (e, pos) in app.world.query::<&crate::components::Position>().iter() {
            let sx = pos.x - cam_x;
            let sy = pos.y - cam_y;
            if sx >= 0 && sy >= 0 && (sx as usize) < view_w && (sy as usize) < view_h {
                let prio = entity_draw_priority(app, e);
                let idx = sy as usize * view_w + sx as usize;
                let replace = match entity_map[idx] {
                    None => true,
                    Some((_, old)) => prio >= old,
                };
                if replace {
                    entity_map[idx] = Some((e, prio));
                }
            }
        }
    }

    let selected = app.selected;
    let mut lines: Vec<Line> = Vec::with_capacity(view_h);

    for sy in 0..view_h {
        let mut spans: Vec<Span> = Vec::with_capacity(view_w);
        let wy = cam_y + sy as i32;
        for sx in 0..view_w {
            let wx = cam_x + sx as i32;
            let visible = app.can_see_tile(viewer, (wx, wy));
            let lit = app.lit_by_fire(wx, wy);

            if !visible {
                if app.map.is_revealed(wx, wy) {
                    if let Some(tile) = app.map.tile(wx, wy) {
                        let fg = memory_fg(tile.color_fg);
                        let bg = Color::Rgb(0, 3, 0);
                        spans.push(Span::styled(
                            tile.symbol.to_string(),
                            Style::default().fg(fg).bg(bg),
                        ));
                    } else {
                        spans.push(Span::styled(
                            " ",
                            Style::default().bg(Color::Black).fg(Color::Black),
                        ));
                    }
                } else {
                    spans.push(Span::styled(
                        " ",
                        Style::default().bg(Color::Black).fg(Color::Black),
                    ));
                }
                continue;
            }

            app.map.reveal(wx, wy);

            if let Some((entity, _)) = entity_map.get(sy * view_w + sx).and_then(|v| *v) {
                // 陷阱：只有建造者自己能看到
                if let Ok(trap) = app.world.get::<&crate::components::StickTrap>(entity) {
                    if Some(trap.builder) != app.actor() {
                        continue; // 非建造者 → 不渲染，露出地形
                    }
                }
                let (ch, color) = entity_glyph(app, entity);
                let mut color = dim_color(color, nightish && !lit);
                color = apply_weather_color(app, color);
                let mut style = Style::default().fg(color).add_modifier(Modifier::BOLD);
                if app
                    .focused_tile
                    .is_some_and(|(fx, fy)| fx == wx && fy == wy)
                {
                    style = style.bg(Color::Yellow).fg(Color::Black);
                } else if Some(entity) == selected {
                    style = style.bg(Color::DarkGray);
                } else if app
                    .examine
                    .as_ref()
                    .is_some_and(|s| s.x == wx && s.y == wy)
                {
                    style = style.bg(Color::Yellow).fg(Color::Black);
                } else if lit {
                    style = style.bg(Color::Rgb(40, 25, 0));
                }
                spans.push(Span::styled(ch.to_string(), style));
            } else if let Some(tile) = app.map.tile(wx, wy) {
                let (mut fg, mut bg) = terrain_colors(tile, nightish && !lit, lit);
                fg = apply_weather_color(app, fg);
                bg = apply_weather_color(app, bg);
                let highlighted = app
                    .focused_tile
                    .is_some_and(|(fx, fy)| fx == wx && fy == wy)
                    || app
                        .examine
                        .as_ref()
                        .is_some_and(|s| s.x == wx && s.y == wy);
                let (fg, bg) = if highlighted {
                    (Color::Black, Color::Yellow)
                } else {
                    (fg, bg)
                };
                spans.push(Span::styled(
                    tile.symbol.to_string(),
                    Style::default().fg(fg).bg(bg),
                ));
            } else {
                spans.push(Span::raw(" "));
            }
        }
        lines.push(Line::from(spans));
    }

    // ── 天气粒子：下落动画 ──
    if let Some((glyph, density, color_name)) = app.weather.particle() {
        let p_color = parse_color(color_name);
        let mut rng = rand::thread_rng();

        // 粒子下落：每滴速度随机，有的快有的慢，别他妈统一节奏
        let vy_max = (cam_y + view_h as i32) as f32 + 2.0;
        let vx_min = cam_x;
        let vx_max = cam_x + view_w as i32;

        // 1. 旧粒子下落
        for p in &mut app.rain_particles {
            p.wy += p.speed;
        }
        // 清理出屏粒子
        app.rain_particles.retain(|p| p.wy < vy_max);

        // 2. 顶部补新粒子（每行按密度补，总数封顶 250 防掉帧）
        const MAX_RAIN: usize = 250;
        let top = cam_y as f32 - 2.0;
        if app.rain_particles.len() < MAX_RAIN {
            for x in vx_min..vx_max {
                if app.rain_particles.len() >= MAX_RAIN { break; }
                if rng.gen_range(0..density) == 0 {
                    app.rain_particles.push(RainDrop {
                        wx: x,
                        wy: top + rng.gen_range(-3.0..0.0), // 错开，不全在同一行落下
                        speed: rng.gen_range(0.12..0.55),    // 有快有慢，别他妈跟军训似的齐步走
                        glyph: if glyph == '│' && rng.gen_bool(0.3) { '|' } else { glyph },
                    });
                }
            }
        }

        // 3. 渲染粒子（只在空地格）
        for p in &app.rain_particles {
            let sx = p.wx - cam_x;
            let sy = p.wy as i32 - cam_y;
            if sx < 0 || sy < 0 || sx as usize >= view_w || sy as usize >= view_h {
                continue;
            }
            let sx = sx as usize;
            let sy = sy as usize;
            // 不可见 / 未揭示 → 跳过
            let wx = p.wx;
            let wy = p.wy as i32;
            if !app.can_see_tile(viewer, (wx, wy)) || !app.map.is_revealed(wx, wy) {
                continue;
            }
            // 有实体不覆盖
            if entity_map.get(sy * view_w + sx).and_then(|v| *v).is_some() {
                continue;
            }
            let p_style = if app.lightning_flash > 0 {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(p_color)
            };
            if sx < lines[sy].spans.len() {
                lines[sy].spans[sx] = Span::styled(p.glyph.to_string(), p_style);
            }
        }
    } else {
        // 非雨天清空粒子
        app.rain_particles.clear();
    }

    // ── 浮动伤害数字：CDDA 风格，从受害者处向上飘，末两帧淡出 ──
    for (_e, dmg) in app.world.query::<&crate::components::DamageNumber>().iter() {
        let sx = dmg.x - cam_x;
        // 向上飘：frame=MAX 时在受害者处，frame=0 时已上飘 3 格（归零前 despawn）
        const MAX_FRAME: u8 = 6;
        let elapsed = MAX_FRAME.saturating_sub(dmg.frame);
        let offset_y = (elapsed as f32 * 0.5) as i32;
        let sy = dmg.y - cam_y - offset_y;
        if sx < 0 || sy < 0 || sx as usize >= view_w || sy as usize >= view_h {
            continue;
        }
        let sx = sx as usize;
        let sy = sy as usize;
        // 可见性
        if !app.can_see_tile(viewer, (dmg.x, dmg.y)) {
            continue;
        }
        // 颜色：伤害 ≥10 用黄色（重击），普通红
        let amount: i32 = dmg
            .text
            .trim_start_matches('-')
            .trim_end_matches('!')
            .parse()
            .unwrap_or(0);
        let color = if amount >= 10 {
            Color::Yellow
        } else {
            Color::Red
        };
        let mut dmg_style = Style::default().fg(color).add_modifier(Modifier::BOLD);
        // 末两帧淡出
        if dmg.frame <= 2 {
            dmg_style = dmg_style.add_modifier(Modifier::DIM);
        }
        if sx < lines[sy].spans.len() {
            lines[sy].spans[sx] = Span::styled(dmg.text.clone(), dmg_style);
        }
    }

    // ── 闪电白闪：3 帧闪烁，跟渲染帧数绑定 ──
    if app.lightning_flash > 0 {
        for line in &mut lines {
            for span in &mut line.spans {
                let bright = flash_color(span.style.fg.unwrap_or(Color::White));
                span.style = span.style.fg(bright);
            }
        }
        app.lightning_flash = app.lightning_flash.saturating_sub(1);
    }

    // ── X 光标标记：玩家看不到此格时画一个 X，避免光标在雾中消失 ──
    if let Some((fx, fy)) = app.focused_tile {
        let sx = fx - cam_x;
        let sy = fy - cam_y;
        if sx >= 0 && sy >= 0 && (sx as usize) < view_w && (sy as usize) < view_h
            && !app.can_see_tile(viewer, (fx, fy))
        {
            let sx_u = sx as usize;
            let sy_u = sy as usize;
            if let Some(line) = lines.get_mut(sy_u) {
                if line.spans.len() > sx_u {
                    line.spans[sx_u] = Span::styled(
                        "X",
                        Style::default()
                            .fg(Color::Yellow)
                            .bg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
        }
    }

    let title = format!(" 区域 ({},{}) ", target.0, target.1);
    let widget = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(widget, area);
}

fn terrain_colors(
    tile: &crate::world::Tile,
    dim: bool,
    lit: bool,
) -> (Color, Color) {
    let mut fg = Color::Rgb(tile.color_fg.0, tile.color_fg.1, tile.color_fg.2);
    let mut bg = Color::Rgb(tile.color_bg.0, tile.color_bg.1, tile.color_bg.2);
    if lit {
        bg = Color::Rgb(40, 25, 0);
        return (fg, bg);
    }
    if dim {
        fg = dim_color(fg, true);
        bg = Color::Rgb(0, 10, 0);
    }
    (fg, bg)
}

fn dim_color(c: Color, dim: bool) -> Color {
    if !dim {
        return c;
    }
    match c {
        Color::Green => Color::Rgb(0, 60, 0),
        Color::Cyan => Color::Rgb(0, 40, 60),
        Color::Blue => Color::Rgb(0, 0, 40),
        Color::Yellow => Color::Rgb(80, 60, 0),
        Color::White => Color::Gray,
        Color::Red => Color::Rgb(80, 0, 0),
        Color::Magenta => Color::Rgb(60, 0, 60),
        Color::Gray => Color::DarkGray,
        other => other,
    }
}

fn memory_fg(rgb: (u8, u8, u8)) -> Color {
    // 记忆色：原色压暗到 ~6%
    Color::Rgb(
        (rgb.0 as u16 * 6 / 100).min(255) as u8,
        (rgb.1 as u16 * 6 / 100).min(255) as u8,
        (rgb.2 as u16 * 6 / 100).min(255) as u8,
    )
}

fn parse_color(name: &str) -> Color {
    match name {
        "green" => Color::Green,
        "dark_green" => Color::Rgb(0, 80, 0),
        "cyan" => Color::Cyan,
        "blue" => Color::Blue,
        "yellow" => Color::Yellow,
        "red" => Color::Red,
        "white" => Color::White,
        "magenta" => Color::Magenta,
        "gray" | "grey" => Color::Gray,
        "black" => Color::Black,
        _ => Color::White,
    }
}

/// 应用天气颜色乘数
fn apply_weather_color(app: &App, c: Color) -> Color {
    let (rm, gm, bm) = app.weather_color_mult();
    if (rm - 1.0).abs() < 0.01 && (gm - 1.0).abs() < 0.01 && (bm - 1.0).abs() < 0.01 {
        return c; // 晴天无开销
    }
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as f32) * rm).min(255.0) as u8,
            ((g as f32) * gm).min(255.0) as u8,
            ((b as f32) * bm).min(255.0) as u8,
        ),
        Color::Green => Color::Rgb(
            0, (255.0 * gm).min(255.0) as u8, 0,
        ),
        // 对于命名颜色，提取近似 RGB 再乘
        other => {
            // 提取近似 RGB 再乘
            let (r, g, b) = color_to_rgb_approx(other);
            Color::Rgb(
                ((r as f32) * rm).min(255.0) as u8,
                ((g as f32) * gm).min(255.0) as u8,
                ((b as f32) * bm).min(255.0) as u8,
            )
        }
    }
}

/// 命名颜色 → 近似 RGB
fn color_to_rgb_approx(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Black => (0, 0, 0),
        Color::Red => (255, 0, 0),
        Color::Green => (0, 255, 0),
        Color::Yellow => (255, 255, 0),
        Color::Blue => (0, 0, 255),
        Color::Magenta => (255, 0, 255),
        Color::Cyan => (0, 255, 255),
        Color::Gray => (128, 128, 128),
        Color::DarkGray => (64, 64, 64),
        Color::White => (255, 255, 255),
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (255, 255, 255),
    }
}

/// 闪电闪白：所有颜色往白色拉 50%
fn flash_color(c: Color) -> Color {
    let (r, g, b) = color_to_rgb_approx(c);
    Color::Rgb(
        (r as u16 + (255 - r as u16) / 2).min(255) as u8,
        (g as u16 + (255 - g as u16) / 2).min(255) as u8,
        (b as u16 + (255 - b as u16) / 2).min(255) as u8,
    )
}
