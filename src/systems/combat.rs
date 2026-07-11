use rand::Rng;

use crate::app::App;
use crate::components::{Colonist, Dead, Fleeing, Health, Hostile, Player, Position};
use crate::items::drop_item_near;

const PERCEPTION_RANGE: i32 = 8;
const FLEE_SAFE_DIST: i32 = 15;
const FLEE_HP_SOLO: f32 = 0.60;
const FLEE_HP_PACK: f32 = 0.40;

/// 狼群的优先攻击目标：玩家活着就追玩家；玩家死了追最近的活殖民者。
fn primary_target(app: &App) -> Option<hecs::Entity> {
    if app.world.get::<&Dead>(app.player).is_err() {
        return Some(app.player);
    }
    let (px, py) = app.player_pos();
    let mut best: Option<(hecs::Entity, i32)> = None;
    for (e, pos) in app.world.query::<&Position>().with::<&Colonist>().iter() {
        if app.world.get::<&Dead>(e).is_ok() {
            continue;
        }
        let d = (pos.x - px).abs().max((pos.y - py).abs());
        if best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((e, d));
        }
    }
    best.map(|(e, _)| e)
}

pub fn update_combat(app: &mut App, rng: &mut impl Rng) {
    // 狼群始终以「玩家角色」为优先目标（玩家死后改追殖民者）
    let target = match primary_target(app) {
        Some(t) => t,
        None => return,
    };
    let (px, py) = match app.world.get::<&Position>(target) {
        Ok(pos) => (pos.x, pos.y),
        Err(_) => return,
    };

    let enemies: Vec<(hecs::Entity, i32, i32, bool, f32, f32)> = app
        .world
        .query::<(&Position, &Health)>()
        .with::<&Hostile>()
        .iter()
        .filter(|(e, _)| app.world.get::<&Dead>(*e).is_err())
        .map(|(e, (pos, hp))| {
            let fleeing = app.world.get::<&Fleeing>(e).is_ok();
            (e, pos.x, pos.y, fleeing, hp.hp, hp.max_hp)
        })
        .collect();

    for (entity, ex, ey, fleeing, hp, max_hp) in enemies {
        let dist = (ex - px).abs().max((ey - py).abs());

        if fleeing {
            move_away(app, entity, ex, ey, px, py);
            let new_dist = app.world.get::<&Position>(entity)
                .map(|pos| (pos.x - px).abs().max((pos.y - py).abs()))
                .unwrap_or(0);
            if new_dist > FLEE_SAFE_DIST {
                let _ = app.world.remove_one::<Fleeing>(entity);
            }
            continue;
        }

        let hp_pct = if max_hp > 0.0 { hp / max_hp } else { 0.0 };
        if try_flee(app, entity, ex, ey, hp_pct, rng) {
            if app.can_see_entity(entity) {
                app.push_log("狼发出一声呜咽，转身逃了。".into());
            } else {
                app.push_log("远处传来一声呜咽。".into());
            }
            continue;
        }

        if dist <= 1 {
            let dmg = rng.gen_range(5.0..15.0);
            if let Ok(mut h) = app.world.get::<&mut Health>(target) {
                h.hp -= dmg;
            }
            let victim = app.entity_label(target);
            let attacker = app.visible_or_generic(entity, "什么东西");
            app.push_log(format!("{}咬了{}一口！", attacker, victim));
            if app.world.get::<&Health>(target).map(|h| h.hp <= 0.0).unwrap_or(false) {
                let cause = if app.world.get::<&Player>(target).is_ok() {
                    "被狼咬死"
                } else {
                    "被狼咬死（殖民者）"
                };
                app.kill(target, cause);
            }
        } else if dist <= PERCEPTION_RANGE {
            let approach = rng.gen_bool(0.80);
            if approach {
                if !move_toward(app, entity, ex, ey, px, py) {
                    random_move(app, entity, ex, ey, px, py, rng);
                }
            } else {
                random_move(app, entity, ex, ey, px, py, rng);
            }
        } else {
            random_move(app, entity, ex, ey, px, py, rng);
        }
    }
}

fn has_nearby_ally(app: &App, self_entity: hecs::Entity, self_x: i32, self_y: i32) -> bool {
    for (e, pos) in app.world.query::<&Position>().with::<&Hostile>().iter() {
        if e == self_entity {
            continue;
        }
        // 死狼不算同伴——别让尸体给活狼壮胆
        if app.world.get::<&Dead>(e).is_ok() {
            continue;
        }
        if (pos.x - self_x).abs() <= 1 && (pos.y - self_y).abs() <= 1 {
            return true;
        }
    }
    false
}

fn try_flee(app: &mut App, entity: hecs::Entity, ex: i32, ey: i32, hp_pct: f32, rng: &mut impl Rng) -> bool {
    let has_ally = has_nearby_ally(app, entity, ex, ey);
    let threshold = if has_ally { FLEE_HP_PACK } else { FLEE_HP_SOLO };
    if hp_pct > threshold {
        return false;
    }
    let should_flee = if has_ally {
        true
    } else {
        rng.gen_bool(0.50)
    };
    if should_flee {
        let _ = app.world.insert_one(entity, Fleeing);
        return true;
    }
    false
}

fn move_toward(app: &mut App, entity: hecs::Entity, ex: i32, ey: i32, tx: i32, ty: i32) -> bool {
    let dx = tx - ex;
    let dy = ty - ey;
    let dirs = if dx.abs() >= dy.abs() {
        [(dx.signum(), 0), (0, dy.signum()), (-dx.signum(), 0), (0, -dy.signum())]
    } else {
        [(0, dy.signum()), (dx.signum(), 0), (0, -dy.signum()), (-dx.signum(), 0)]
    };
    try_move(app, entity, ex, ey, &dirs)
}

fn move_away(app: &mut App, entity: hecs::Entity, ex: i32, ey: i32, px: i32, py: i32) {
    let dx = ex - px;
    let dy = ey - py;
    let dirs = if dx.abs() >= dy.abs() {
        [(dx.signum(), 0), (0, dy.signum()), (-dx.signum(), 0), (0, -dy.signum())]
    } else {
        [(0, dy.signum()), (dx.signum(), 0), (0, -dy.signum()), (-dx.signum(), 0)]
    };
    try_move(app, entity, ex, ey, &dirs);
}

fn random_move(app: &mut App, entity: hecs::Entity, ex: i32, ey: i32, _px: i32, _py: i32, rng: &mut impl Rng) {
    let mut dirs = [(1, 0), (-1, 0), (0, 1), (0, -1), (0, 0)];
    shuffle(&mut dirs, rng);
    try_move(app, entity, ex, ey, &dirs);
}

fn try_move(app: &mut App, entity: hecs::Entity, ex: i32, ey: i32, dirs: &[(i32, i32)]) -> bool {
    // 提前查一次，别在方向循环里反复调 primary_target
    let target_pos = primary_target(app)
        .and_then(|t| app.world.get::<&Position>(t).ok().map(|p| (p.x, p.y)));
    for &(dx, dy) in dirs {
        let nx = ex + dx;
        let ny = ey + dy;
        if !app.map.is_walkable(nx, ny) {
            continue;
        }
        if app.is_blocked(nx, ny) {
            continue;
        }
        // 别往目标身上站
        if target_pos == Some((nx, ny)) {
            continue;
        }
        let other_hostile = app
            .occupied(nx, ny)
            .map(|e| e != entity && app.world.get::<&Hostile>(e).is_ok())
            .unwrap_or(false);
        if other_hostile {
            continue;
        }
        if let Ok(mut pos) = app.world.get::<&mut Position>(entity) {
            pos.x = nx;
            pos.y = ny;
        }
        app.mark_spatial_dirty();
        return true;
    }
    false
}

fn shuffle<T: Copy>(slice: &mut [T], rng: &mut impl Rng) {
    for i in (1..slice.len()).rev() {
        let j = rng.gen_range(0..=i);
        slice.swap(i, j);
    }
}

pub fn try_player_attack(app: &mut App, target_x: i32, target_y: i32, rng: &mut impl Rng) -> bool {
    let target = match app.occupied(target_x, target_y) {
        Some(e) if app.world.get::<&Hostile>(e).is_ok() => e,
        _ => return false,
    };

    // 死人不会被打第二次
    if app.world.get::<&Dead>(target).is_ok() {
        return false;
    }

    let dmg = rng.gen_range(10.0..20.0);
    let kill = {
        if let Ok(mut hp) = app.world.get::<&mut Health>(target) {
            hp.hp -= dmg;
            hp.hp <= 0.0
        } else {
            false
        }
    };

    let target_name = app.entity_label(target);
    let actor_name = app
        .actor()
        .map(|e| app.entity_label(e))
        .unwrap_or_else(|| "?".into());
    app.push_log(format!(
        "{}一拳砸在{}身上——{}点伤害。",
        actor_name, target_name, dmg as i32
    ));

    if kill {
        let loot = rng.gen_range(0..=2);
        if loot > 0 {
            drop_item_near(app, (target_x, target_y), (target_x, target_y), crate::components::ItemKind::Stick, loot as u32);
        }
        app.kill(target, "被打死");
    }

    true
}
