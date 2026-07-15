use rand::Rng;

use crate::app::App;
use crate::components::{Colonist, DamageNumber, Dead, Fleeing, Health, HitFlash, Hostile, MoveCooldown, Player, Position};
use crate::items::drop_item_near;

const PERCEPTION_RANGE: i32 = 8;
const FLEE_SAFE_DIST: i32 = 15;
const FLEE_HP_SOLO: f32 = 0.60;
const FLEE_HP_PACK: f32 = 0.40;

/// 狼群的优先攻击目标：50%玩家/殖民者，30%猎物，20%闲逛
fn primary_target(app: &App, rng: &mut impl Rng) -> Option<hecs::Entity> {
    if app.world.get::<&Dead>(app.player).is_err() {
        return Some(app.player);
    }
    // 30% 追猎物
    if rng.gen_bool(0.30) {
        let (px, py) = app.player_pos();
        let mut best: Option<(hecs::Entity, i32)> = None;
        for (e, pos) in app.world.query::<&Position>().with::<&crate::components::Animal>().iter() {
            let d = (pos.x - px).abs().max((pos.y - py).abs());
            if d <= 15 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((e, d));
            }
        }
        if let Some((e, _)) = best { return Some(e); }
    }
    // 50% 追玩家/殖民者
    let (px, py) = app.player_pos();
    let mut best: Option<(hecs::Entity, i32)> = None;
    for (e, pos) in app.world.query::<&Position>().with::<&Colonist>().iter() {
        if app.world.get::<&Dead>(e).is_ok() { continue; }
        let d = (pos.x - px).abs().max((pos.y - py).abs());
        if best.map(|(_, bd)| d < bd).unwrap_or(true) { best = Some((e, d)); }
    }
    best.map(|(e, _)| e)
}

pub fn update_combat(app: &mut App, rng: &mut impl Rng) {
    // Plan 11: 狼 30% 追猎物，50% 追玩家/殖民者
    let target = match primary_target(app, rng) {
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
        // ── MoveCooldown：狼在地形里走慢了也得等 ──
        if let Ok(cd) = app.world.get::<&MoveCooldown>(entity) {
            if cd.ticks > 0 {
                continue;
            }
        }

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
            apply_damage(app, target, dmg, (px, py));
            let victim = app.entity_label(target);
            let attacker = app.visible_or_generic(entity, "什么东西");
            app.push_log(format!("{}咬了{}一口！", attacker, victim));
            if app.world.get::<&Health>(target).map(|h| h.hp <= 0.0).unwrap_or(false) {
                // 猎物死亡 → 留尸体
                let animal_kind = app.world.get::<&crate::components::Animal>(target).ok().map(|a| a.kind);
                if let Some(kind) = animal_kind {
                    let (tx, ty) = (px, py);
                    let uid = app.next_uid; app.next_uid += 1;
                    app.world.spawn((
                        Position { x: tx, y: ty },
                        crate::components::EntityUID(uid),
                        crate::components::Corpse { animal: kind, spoilage: 90000 },
                        crate::components::BlocksMovement,
                    ));
                    let _ = app.world.despawn(target);
                    app.mark_spatial_dirty();
                    continue;
                }
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

/// 统一伤害入口：扣血 + HitFlash + DamageNumber。所有伤害源都走这里。
pub fn apply_damage(app: &mut App, victim: hecs::Entity, amount: f32, pos: (i32, i32)) {
    // 死人不再受伤
    if app.world.get::<&Dead>(victim).is_ok() {
        return;
    }
    if let Ok(mut hp) = app.world.get::<&mut Health>(victim) {
        hp.hp = (hp.hp - amount).max(0.0);
    }
    let _ = app.world.insert_one(victim, HitFlash { frames: 3 });
    app.world.spawn((DamageNumber {
        text: format!("-{}", amount as i32),
        frame: 6,
        x: pos.0,
        y: pos.1,
    },));
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
    let target_pos = primary_target(app, &mut rand::thread_rng())
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
        // 陷阱触发
        crate::systems::building::trigger_trap_at(app, nx, ny, entity);
        // 地形移动冷却
        let cost = crate::systems::movement::terrain_move_cost(app, nx, ny);
        if cost > 0.0 {
            let cd = (1.0 / cost).ceil() as u32;
            let cd = cd.saturating_sub(1);
            let has_mc = app.world.get::<&MoveCooldown>(entity).is_ok();
            if has_mc {
                if let Ok(mut mc) = app.world.get::<&mut MoveCooldown>(entity) {
                    mc.ticks = cd;
                }
            } else {
                let _ = app.world.insert_one(entity, MoveCooldown { ticks: cd });
            }
        }
        app.mark_spatial_dirty();
        return true;
    }
    false
}

/// 每 tick 递减 HitFlash / DamageNumber 帧数，归零清理
pub fn tick_visual_effects(app: &mut App) {
    let mut dead_flash = Vec::new();
    for (e, flash) in app.world.query::<&mut HitFlash>().iter() {
        flash.frames = flash.frames.saturating_sub(1);
        if flash.frames == 0 {
            dead_flash.push(e);
        }
    }
    for e in dead_flash {
        let _ = app.world.remove_one::<HitFlash>(e);
    }

    let mut dead_dmg = Vec::new();
    for (e, dmg) in app.world.query::<&mut DamageNumber>().iter() {
        dmg.frame = dmg.frame.saturating_sub(1);
        if dmg.frame == 0 {
            dead_dmg.push(e);
        }
    }
    for e in dead_dmg {
        let _ = app.world.despawn(e);
    }
}

/// 返回玩家当前手持的最强武器伤害范围。空手 = (3.0, 8.0)
fn best_weapon_dmg(app: &App) -> (f32, f32) {
    use crate::components::{Hands, ItemKind};
    let Some(actor) = app.actor() else { return (3.0, 8.0); };
    let Ok(hands) = app.world.get::<&Hands>(actor) else { return (3.0, 8.0); };
    let items = [hands.left, hands.right];
    let mut best: (f32, f32) = (3.0, 8.0); // 空手基准
    for slot in items.iter().flatten() {
        let dmg = match slot.0 {
            ItemKind::Spear => (15.0, 25.0),        // 火烤矛
            ItemKind::WoodSpear => (8.0, 15.0),      // 削尖长棍
            ItemKind::StoneAxe => (12.0, 22.0),
            ItemKind::StoneHammer => (10.0, 18.0),
            ItemKind::WoodAxe => (6.0, 12.0),
            ItemKind::StoneKnife => (5.0, 9.0),
            ItemKind::WoodKnife => (5.0, 10.0),
            ItemKind::BoneKnife => (7.0, 12.0),
            _ => continue,
        };
        if dmg.0 + dmg.1 > best.0 + best.1 { best = dmg; }
    }
    best
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

    let dmg = {
        let best = best_weapon_dmg(app);
        rng.gen_range(best.0..best.1)
    };
    apply_damage(app, target, dmg, (target_x, target_y));
    let kill = app.world.get::<&Health>(target).map(|h| h.hp <= 0.0).unwrap_or(false);

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
