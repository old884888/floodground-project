use crate::app::App;
use crate::components::{MoveCooldown, Position};
use crate::data::terrain_def;
use crate::events::GameEvent;

pub fn apply_pending_move(app: &mut App, rng: &mut impl rand::Rng) {
    let Some((dx, dy)) = app.pending_move.take() else {
        return;
    };

    if dx != 0 || dy != 0 {
        app.facing = (dx, dy);
    }

    let Some(actor) = app.actor() else {
        app.push_log("没有能动的人。".into());
        return;
    };

    // ── MoveCooldown 检查：地形 move_cost 引起的冷却 ──
    {
        if let Ok(cd) = app.world.get::<&MoveCooldown>(actor) {
            if cd.ticks > 0 {
                // 冷却中，不响应移动
                return;
            }
        }
    }

    let from = {
        let Ok(pos) = app.world.get::<&Position>(actor) else {
            return;
        };
        (pos.x, pos.y)
    };
    let to = (from.0 + dx, from.1 + dy);

    // 撞到敌人 → 攻击，不走路
    if crate::systems::combat::try_player_attack(app, to.0, to.1, rng) {
        return;
    }

    if !app.map.is_walkable(to.0, to.1) {
        app.push_log("那边过不去。".into());
        return;
    }

    if app.is_blocked(to.0, to.1) {
        // 区分树/岩/人
        if let Some(other) = app.actor_or_blocker_at(to.0, to.1) {
            if other != actor {
                let msg = if app.world.get::<&crate::components::Tree>(other).is_ok() {
                    "一棵树挡在路上。".into()
                } else if app.world.get::<&crate::components::Boulder>(other).is_ok() {
                    "一块岩石挡在路上。".into()
                } else {
                    app.world
                        .get::<&crate::components::Name>(other)
                        .map(|n| format!("{}挡在路上。", n.0))
                        .unwrap_or_else(|_| "有东西挡在路上。".into())
                };
                app.push_log(msg);
                return;
            }
        } else {
            app.push_log("那边过不去。".into());
            return;
        }
    }

    // 角色互撞（不带 BlocksMovement 的 Name 实体）
    if let Some(other) = app.occupied(to.0, to.1) {
        if other != actor
            && app.world.get::<&crate::components::Name>(other).is_ok() {
                let blocker = app
                    .world
                    .get::<&crate::components::Name>(other)
                    .map(|n| n.0.clone())
                    .unwrap_or_else(|_| "有人".into());
                app.push_log(format!("{}挡在路上。", blocker));
                return;
            }
            // 地面物品 / 灌木：可以踩上去
    }

    if let Ok(mut pos) = app.world.get::<&mut Position>(actor) {
        pos.x = to.0;
        pos.y = to.1;
    }

    // ── 设置移动冷却：根据落点地形 move_cost ──
    let move_cost = terrain_move_cost(app, to.0, to.1);
    if move_cost > 0.0 {
        let cooldown = (1.0 / move_cost).ceil() as u32;
        let cd = cooldown.saturating_sub(1);
        if let Ok(mut mc) = app.world.get::<&mut MoveCooldown>(actor) {
            mc.ticks = cd;
        }
    }

    // ── 陷阱触发 ──
    crate::systems::building::trigger_trap_at(app, to.0, to.1, actor);

    app.mark_spatial_dirty();

    app.events.push(GameEvent::CharacterMoved {
        entity: actor,
        from,
        to,
    });
}

/// 查地形 move_cost：从 terrain.ron 注册表查表
pub fn terrain_move_cost(app: &App, x: i32, y: i32) -> f32 {
    let kind = app.map.terrain(x, y);
    terrain_def(kind.key()).move_cost
}

/// 每 tick 递减所有 MoveCooldown
pub fn tick_cooldowns(app: &mut App) {
    for (_e, cd) in app.world.query::<&mut MoveCooldown>().iter() {
        if cd.ticks > 0 {
            cd.ticks -= 1;
        }
    }
}

/// 让 entity 朝 (tx, ty) 走一步（曼哈顿步进）。
/// 返回 true = 已到达目标，false = 还在路上/被挡住。
pub fn step_toward(app: &mut App, entity: hecs::Entity, tx: i32, ty: i32) -> bool {
    let (sx, sy) = match app.world.get::<&Position>(entity) {
        Ok(pos) => (pos.x, pos.y),
        Err(_) => return true,
    };

    // 已到达
    if sx == tx && sy == ty {
        return true;
    }

    // 移动冷却
    if let Ok(cd) = app.world.get::<&MoveCooldown>(entity) {
        if cd.ticks > 0 {
            return false;
        }
    }

    // 计算一步方向
    let dx = (tx - sx).signum();
    let dy = (ty - sy).signum();
    let nx = sx + dx;
    let ny = sy + dy;

    // 碰撞检测
    if !app.map.is_walkable(nx, ny) || app.is_blocked(nx, ny) {
        // 被挡：尝试绕行
        let alts = [(dx, 0), (0, dy)];
        for &(adx, ady) in &alts {
            if adx == 0 && ady == 0 { continue; }
            let alt_x = sx + adx;
            let alt_y = sy + ady;
            if (alt_x == sx && alt_y == sy) || (alt_x == nx && alt_y == ny) { continue; }
            if app.map.is_walkable(alt_x, alt_y) && !app.is_blocked(alt_x, alt_y) {
                if let Ok(mut pos) = app.world.get::<&mut Position>(entity) {
                    pos.x = alt_x; pos.y = alt_y;
                }
                // 陷阱触发
                crate::systems::building::trigger_trap_at(app, alt_x, alt_y, entity);
                set_move_cooldown(app, entity, alt_x, alt_y);
                app.mark_spatial_dirty();
                return false;
            }
        }
        return false; // 彻底挡住
    }

    // 移动
    if let Ok(mut pos) = app.world.get::<&mut Position>(entity) {
        pos.x = nx; pos.y = ny;
    }
    crate::systems::building::trigger_trap_at(app, nx, ny, entity);
    set_move_cooldown(app, entity, nx, ny);
    app.mark_spatial_dirty();

    // 到达检查
    nx == tx && ny == ty
}

fn set_move_cooldown(app: &App, entity: hecs::Entity, x: i32, y: i32) {
    let cost = terrain_move_cost(app, x, y);
    if cost > 0.0 {
        let cooldown = (1.0 / cost).ceil() as u32;
        let cd = cooldown.saturating_sub(1);
        if let Ok(mut mc) = app.world.get::<&mut MoveCooldown>(entity) {
            mc.ticks = cd;
        }
    }
}
