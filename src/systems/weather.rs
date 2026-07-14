//! 天气系统 v1：六种天气状态机 + 潮湿计算 + 火源熄灭 + 闪电白闪

use rand::Rng;

use crate::app::{App, Weather};
use crate::components::{LightSource, Position, Wet};
use crate::events::GameEvent;

/// 每 tick 推进天气计时器 + 潮湿/火源/闪电
pub fn update_weather(app: &mut App, rng: &mut impl Rng) {
    // ── 天气计时器 ──
    if app.weather_timer == 0 {
        // 初始化（首次 tick）
        let dur = rng.gen_range(
            app.weather.duration_range().0..=app.weather.duration_range().1,
        );
        app.weather_timer = dur.max(1);
    }

    app.weather_timer = app.weather_timer.saturating_sub(1);

    if app.weather_timer == 0 {
        let old = app.weather;
        let next = old.next(rng);
        let dur =
            rng.gen_range(next.duration_range().0..=next.duration_range().1);
        app.weather = next;
        app.weather_timer = dur.max(1);
        app.events.push(GameEvent::WeatherChanged {
            from: old,
            to: next,
        });
    }

    // ── 潮湿更新 ──
    update_wet(app);

    // ── 户外火源熄灭 ──
    extinguish_fires(app, rng);

    // ── 闪电白闪（仅雷阵雨，持续 3 帧闪烁）──
    if app.weather.lightning_chance() > 0.0
        && rng.gen_bool(app.weather.lightning_chance())
    {
        app.lightning_flash = 3;
        app.events.push(GameEvent::LightningFlash);
    }
}

fn update_wet(app: &mut App) {
    let wet_rate = app.weather.wet_rate();

    // 收集需要更新 Wet 的实体及其干燥条件
    let mut wet_updates: Vec<(hecs::Entity, f32)> = Vec::new();

    for (e, (pos, _wet)) in app.world.query::<(&Position, &Wet)>().iter() {

        let mut delta = 0.0f32;
        let terrain = app.map.terrain(pos.x, pos.y);
        let terrain_def = crate::data::terrain_def(terrain.key());

        // 淋雨：户外（无屋顶）→ 涨，受地形 rain_shield 削减
        if wet_rate > 0.0 && !app.map.has_roof(pos.x, pos.y) {
            let effective_rate = wet_rate * (1.0 - terrain_def.rain_shield);
            delta += effective_rate;
        }

        // 地形自动潮湿（浅水/浅沼）
        if terrain_def.auto_wet && !app.map.has_roof(pos.x, pos.y) {
            delta += terrain.auto_wet_rate();
        }

        // 干燥条件（可叠加）
        // 室内
        if app.map.has_roof(pos.x, pos.y) {
            delta -= 0.12;
        }
        // 篝火邻格（曼哈顿 dist ≤ 2）
        if has_fire_nearby(app, pos.x, pos.y, 2) {
            delta -= 0.8;
        }
        // 晴天户外
        if app.weather == Weather::Clear && !app.map.has_roof(pos.x, pos.y) {
            delta -= 0.08;
        }

        // 手持火把（只有 actor 可能持有，简化处理）
        if Some(e) == app.actor()
            && has_torch_in_hands(app, e)
        {
            delta -= 0.05;
        }

        if delta != 0.0 {
            wet_updates.push((e, delta));
        }
    }

    for (e, delta) in wet_updates {
        if let Ok(mut wet) = app.world.get::<&mut Wet>(e) {
            wet.value = (wet.value + delta).clamp(0.0, 100.0);
        }
    }
}

/// 检查 (x,y) 曼哈顿距离 max_dist 以内是否有 LightSource 实体（篝火/火把实体）
fn has_fire_nearby(app: &App, x: i32, y: i32, max_dist: i32) -> bool {
    for dy in -max_dist..=max_dist {
        for dx in -max_dist..=max_dist {
            if dx.abs() + dy.abs() > max_dist {
                continue;
            }
            if let Some(v) = app.spatial.by_tile.get(&(x + dx, y + dy)) {
                for &e in v {
                    if app.world.get::<&LightSource>(e).is_ok() {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// 检查实体是否手持火把
fn has_torch_in_hands(app: &App, entity: hecs::Entity) -> bool {
    use crate::components::{Hands, ItemKind};
    app.world
        .get::<&Hands>(entity)
        .ok()
        .map(|h| {
            h.left.is_some_and(|(k, _)| k == ItemKind::Torch)
                || h.right.is_some_and(|(k, _)| k == ItemKind::Torch)
        })
        .unwrap_or(false)
}

/// 户外火源（无屋顶覆盖的 LightSource）按天气概率熄灭
fn extinguish_fires(app: &mut App, rng: &mut impl Rng) {
    let chance = app.weather.fire_extinguish_chance();
    if chance <= 0.0 {
        return;
    }

    let mut to_extinguish: Vec<(hecs::Entity, (i32, i32))> = Vec::new();

    for (e, (pos, _light)) in app
        .world
        .query::<(&Position, &LightSource)>()
        .iter()
    {
        // 有屋顶保护 → 不灭
        if app.map.has_roof(pos.x, pos.y) {
            continue;
        }
        if rng.gen_bool(chance) {
            to_extinguish.push((e, (pos.x, pos.y)));
        }
    }

    for (entity, pos) in to_extinguish {
        let _ = app.world.despawn(entity);
        app.mark_spatial_dirty();
        app.clear_lit_cache(); // 火源消失，照亮缓存失效
        app.events
            .push(GameEvent::FireExtinguished { pos });
        app.push_log("篝火被雨水浇灭了。".into());
    }
}
