//! 天气系统 v1：六种天气状态机 + 潮湿计算 + 火源熄灭 + 闪电白闪

use rand::Rng;

use crate::app::{App, Weather};
use crate::components::{LightSource, PitShelter, Position, Wet};
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

    // ── 水洼生成 ──
    spawn_puddles(app, rng);

    // ── 地坑庇护所塌方 ──
    collapse_pit_shelters(app, rng);

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
            wet.value += delta;
            wet.clamp();
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

/// 雨后水洼：暴雨 10%/雷阵雨 5% 在浅沼上生成，全图最多 20 个
fn spawn_puddles(app: &mut App, rng: &mut impl Rng) {
    use crate::components::{Puddle, TerrainKind};
    let chance = match app.weather {
        Weather::Heavy => 0.10,
        Weather::Thunder => 0.05,
        _ => return,
    };
    let puddle_count = app.world.query::<&Puddle>().iter().count();
    if puddle_count >= 20 { return; }
    // 随机找浅沼格
    for _ in 0..50 {
        if app.world.query::<&Puddle>().iter().count() >= 20 { break; }
        let x = rng.gen_range(0..500);
        let y = rng.gen_range(0..500);
        if app.map.terrain(x, y) == TerrainKind::ShallowMarsh
            && app.map.is_walkable(x, y)
            && !app.is_blocked(x, y)
            && rng.gen_bool(chance)
        {
            app.world.spawn((Position { x, y }, Puddle));
            app.mark_spatial_dirty();
        }
    }
}

/// 地坑庇护所塌方：暴雨 3%/雷阵雨 5%，塌后变废墟+人在里头受伤
fn collapse_pit_shelters(app: &mut App, rng: &mut impl Rng) {
    let chance = match app.weather {
        Weather::Heavy => 0.03,
        Weather::Thunder => 0.05,
        _ => return,
    };

    let mut to_collapse = Vec::new();
    for (e, (pos, _)) in app.world.query::<(&Position, &PitShelter)>().iter() {
        // 露天才有塌方风险（无屋顶保护）
        if app.map.has_roof(pos.x, pos.y) { continue; }
        if rng.gen_bool(chance) {
            to_collapse.push((e, (pos.x, pos.y)));
        }
    }

    for (entity, (px, py)) in to_collapse {
        // 检查里面有没有人，先收集实体再处理（避免借用冲突）
        let mut victims: Vec<hecs::Entity> = Vec::new();
        if let Some(v) = app.spatial.by_tile.get(&(px, py)) {
            for &e in v {
                if e != entity && app.world.get::<&crate::components::Health>(e).is_ok() {
                    victims.push(e);
                }
            }
        }
        for v in victims {
            crate::systems::combat::apply_damage(app, v, rng.gen_range(10.0..20.0), (px, py));
            let name = app.entity_label(v);
            app.push_log(format!("地坑塌了！{}被埋在了泥土和树叶下面。", name));
        }
        // 移除地坑 + 掉材料
        let _ = app.world.despawn(entity);
        app.map.set_roof(px, py, false);
        crate::items::place_item(app, px, py, crate::components::ItemKind::LongStick, rng.gen_range(1..=2));
        crate::items::place_item(app, px, py, crate::components::ItemKind::Leaves, rng.gen_range(3..=8));
        app.mark_spatial_dirty();
    }
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
