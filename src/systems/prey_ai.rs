//! 猎物 AI：鹿逃跑 / 野猪反击 / 兔子跳开

use rand::Rng;

use crate::app::App;
use crate::components::*;
use crate::world::{MAP_HEIGHT, MAP_WIDTH};

/// 每 tick 更新猎物行为
pub fn update_prey_ai(app: &mut App, rng: &mut impl Rng) {
    let player_pos = app.actor_pos();
    let mut actions: Vec<(hecs::Entity, i32, i32)> = Vec::new(); // (entity, dx, dy)

    for (e, (animal, pos)) in app.world.query::<(&Animal, &Position)>().iter() {
        if let Ok(_cd) = app.world.get::<&MoveCooldown>(e) {
            if app.world.get::<&MoveCooldown>(e).map(|c| c.ticks > 0).unwrap_or(false) { continue; }
        }
        let dist = (pos.x - player_pos.0).abs().max((pos.y - player_pos.1).abs());

        match animal.kind {
            AnimalKind::Deer => {
                if dist <= 10 {
                    let (dx, dy) = (pos.x - player_pos.0, pos.y - player_pos.1);
                    let mx = if dx.abs() >= dy.abs() { dx.signum() } else { 0 };
                    let my = if dy.abs() > dx.abs() { dy.signum() } else { 0 };
                    actions.push((e, mx, my));
                    if mx == 0 && my == 0 { if let Some(a) = actions.last_mut() { a.1 = 1; a.2 = 0; } }
                }
            }
            AnimalKind::Boar => {
                if dist <= 6 && rng.gen_bool(0.30) {
                    let (dx, dy) = (player_pos.0 - pos.x, player_pos.1 - pos.y);
                    let mx = if dx.abs() >= dy.abs() { dx.signum() } else { 0 };
                    let my = if dy.abs() > dx.abs() { dy.signum() } else { 0 };
                    actions.push((e, mx, my));
                }
            }
            AnimalKind::Rabbit => {
                if dist <= 8 {
                    let (nx, ny) = (
                        (pos.x + rng.gen_range(-5..=5)).clamp(0, MAP_WIDTH - 1),
                        (pos.y + rng.gen_range(-5..=5)).clamp(0, MAP_HEIGHT - 1),
                    );
                    if app.map.is_walkable(nx, ny) && !app.is_blocked(nx, ny) {
                        if let Ok(mut p) = app.world.get::<&mut Position>(e) {
                            p.x = nx; p.y = ny;
                        }
                    }
                }
            }
        }
    }

    for (e, dx, dy) in actions {
        let (ex, ey) = app.world.get::<&Position>(e).map(|p| (p.x, p.y)).unwrap_or((0, 0));
        let (nx, ny) = (ex + dx, ey + dy);
        if app.map.is_walkable(nx, ny) && !app.is_blocked(nx, ny) {
            if let Ok(mut p) = app.world.get::<&mut Position>(e) { p.x = nx; p.y = ny; }
            let cost = crate::systems::movement::terrain_move_cost(app, nx, ny);
            if cost > 0.0 {
                let cd = (1.0 / cost).ceil() as u32;
                if let Ok(mut mc) = app.world.get::<&mut MoveCooldown>(e) { mc.ticks = cd.saturating_sub(1); }
            }
        }
    }
}
