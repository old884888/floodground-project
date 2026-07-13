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
