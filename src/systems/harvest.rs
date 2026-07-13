use rand::Rng;

use crate::app::App;
use crate::components::{
    Boulder, Bush, BushState, Hands, Harvestable, ItemKind, Position, TerrainKind, Tree, Wall,
};
use crate::events::GameEvent;
use crate::items::drop_item_near;

pub fn try_chop(app: &mut App, rng: &mut impl Rng) {
    hit_harvestable(app, rng, TargetKind::Tree);
}

pub fn try_mine(app: &mut App, rng: &mut impl Rng) {
    hit_harvestable(app, rng, TargetKind::Boulder);
}

pub fn try_break_wall(app: &mut App, rng: &mut impl Rng) {
    hit_harvestable(app, rng, TargetKind::WallTarget);
}

enum TargetKind {
    Tree,
    Boulder,
    WallTarget,
}

/// 地形差异化产出倍率（砍树/挖矿通用）
fn terrain_yield_multiplier(kind: &TargetKind, terrain: TerrainKind) -> u32 {
    match (kind, terrain) {
        // 密林砍树：木头×2
        (TargetKind::Tree, TerrainKind::DenseForest) => 2,
        // 疏林砍树：木头×1（正常）
        (TargetKind::Tree, TerrainKind::LightForest) => 1,
        // 其他地形砍树：正常
        (TargetKind::Tree, _) => 1,
        // 挖矿不翻倍（丘陵的额外产出单独处理）
        (TargetKind::Boulder, _) => 1,
        // 砸墙不受地形影响
        (TargetKind::WallTarget, _) => 1,
    }
}

fn hit_harvestable(app: &mut App, rng: &mut impl Rng, kind: TargetKind) {
    let Some(actor) = app.actor() else {
        return;
    };
    let (px, py) = match app.world.get::<&Position>(actor) {
        Ok(pos) => (pos.x, pos.y),
        Err(_) => return,
    };

    let mut target: Option<(hecs::Entity, f32, f32, ItemKind, f32)> = None;
    for (e, (pos, h)) in app.world.query::<(&Position, &Harvestable)>().iter() {
        let dist = (pos.x - px).abs() + (pos.y - py).abs();
        if dist != 1 {
            continue;
        }
        let ok = match kind {
            TargetKind::Tree => app.world.get::<&Tree>(e).is_ok(),
            TargetKind::Boulder => app.world.get::<&Boulder>(e).is_ok(),
            TargetKind::WallTarget => app.world.get::<&Wall>(e).is_ok(),
        };
        if ok {
            target = Some((e, h.hp, h.max_hp, h.yield_item, h.yield_hp_step));
            break;
        }
    }

    let Some((entity, hp, max_hp, yield_item, step)) = target else {
        let msg = match kind {
            TargetKind::Tree => "旁边没有树可砍。走近点再按 C。",
            TargetKind::Boulder => "旁边没有岩石可采。走近点再按 M。",
            TargetKind::WallTarget => "旁边没有墙可砸。",
        };
        app.push_log(msg.into());
        return;
    };

    let (dmg_lo, dmg_hi) = match kind {
        TargetKind::Tree => (30.0, 60.0),
        TargetKind::Boulder => (20.0, 45.0),
        TargetKind::WallTarget => (25.0, 50.0),
    };
    let damage = rng.gen_range(dmg_lo..dmg_hi);
    let old_hp = hp;
    let new_hp = (hp - damage).max(0.0);

    // 越过 yield 边界就掉落
    let mut drops = 0u32;
    if step > 0.0 {
        let old_band = (old_hp / step).ceil() as i32;
        let new_band = (new_hp / step).ceil() as i32;
        if new_hp > 0.0 {
            drops = (old_band - new_band).max(0) as u32;
        } else if old_hp > 0.0 {
            // 最后一击：把剩余未掉的 step 也吐出来
            drops = old_band.max(0) as u32;
        }
    }

    if let Ok(mut h) = app.world.get::<&mut Harvestable>(entity) {
        h.hp = new_hp;
    }

    let tx = app
        .world
        .get::<&Position>(entity)
        .map(|p| (p.x, p.y))
        .unwrap_or((px, py));

    // ── 地形差异化产出 ──
    let actor_terrain = app.map.terrain(px, py);
    let multiplier = terrain_yield_multiplier(&kind, actor_terrain);
    let total_drops = drops * multiplier;
    for _ in 0..total_drops {
        drop_item_near(app, tx, (px, py), yield_item, 1);
    }

    // 额外产出：丘陵挖矿 20% 概率额外掉金属矿
    if matches!(kind, TargetKind::Boulder)
        && actor_terrain == TerrainKind::Hill
        && rng.gen_bool(0.20)
    {
        drop_item_near(app, tx, (px, py), ItemKind::MetalOre, 1);
    }

    match kind {
        TargetKind::Tree => {
            app.events.push(GameEvent::TreeChopped {
                damage,
                hp_left: new_hp,
                max_hp,
            });
            if new_hp <= 0.0 {
                let _ = app.world.despawn(entity);
                app.mark_spatial_dirty();
                app.events.push(GameEvent::TreeFelled);
                app.push_log("树轰然倒下。".into());
            } else {
                let flavor = chop_flavor(new_hp, max_hp, rng);
                app.push_log(format!(
                    "{}（HP {:.0}/{:.0}）。",
                    flavor, new_hp, max_hp
                ));
            }
        }
        TargetKind::Boulder => {
            app.events.push(GameEvent::BoulderMined {
                damage,
                hp_left: new_hp,
                max_hp,
            });
            if new_hp <= 0.0 {
                let _ = app.world.despawn(entity);
                app.mark_spatial_dirty();
                app.events.push(GameEvent::BoulderDestroyed);
                app.push_log("岩石碎成一堆渣。".into());
            } else {
                app.push_log(format!(
                    "你一镐砸在岩石上，石屑飞溅（HP {:.0}/{:.0}）。",
                    new_hp, max_hp
                ));
            }
        }
        TargetKind::WallTarget => {
            if new_hp <= 0.0 {
                let _ = app.world.despawn(entity);
                app.mark_spatial_dirty();
                app.push_log("墙垮了！".into());
            } else {
                app.push_log(format!(
                    "你一锤砸在墙上——裂痕蔓延（HP {:.0}/{:.0}）。",
                    new_hp, max_hp
                ));
            }
        }
    }
}

fn chop_flavor(hp: f32, max: f32, rng: &mut impl Rng) -> &'static str {
    let ratio = hp / max;
    if ratio < 0.15 {
        "再补一下就能砍倒它"
    } else if ratio < 0.4 {
        ["斧刃嵌进了树干", "木屑飞溅", "树干发出沉闷的呻吟"][rng.gen_range(0..3)]
    } else {
        ["你一斧砍在树上", "斧头啃进树皮", "木屑溅到你脸上"][rng.gen_range(0..3)]
    }
}

/// 邻格结果灌木采摘（由 G 调用）
pub fn try_harvest_bush(app: &mut App, rng: &mut impl Rng) -> bool {
    let Some(actor) = app.actor() else {
        return false;
    };
    let (px, py) = match app.world.get::<&Position>(actor) {
        Ok(pos) => (pos.x, pos.y),
        Err(_) => return false,
    };

    let mut target: Option<(hecs::Entity, ItemKind)> = None;
    for (e, (pos, bush)) in app.world.query::<(&Position, &Bush)>().iter() {
        let dist = (pos.x - px).abs() + (pos.y - py).abs();
        if dist == 1 && bush.state == BushState::Fruiting {
            target = Some((e, bush.yield_item));
            break;
        }
    }
    let Some((entity, yield_item)) = target else {
        return false;
    };

    let roll = rng.gen_range(1..=100);
    let count = match roll {
        1..=34 => 1,
        35..=94 => 2,
        95 => 3,
        _ => 0,
    };

    if let Ok(mut bush) = app.world.get::<&mut Bush>(entity) {
        bush.state = BushState::None;
        bush.growth_timer = 0;
    }

    if count == 0 {
        app.push_log("灌木是空的，什么都没摘到。虫子先你一步。".into());
        app.events.push(GameEvent::BushHarvested { count: 0 });
        return true;
    }

    let mut remaining = count;
    {
        if let Ok(mut hands) = app.world.get::<&mut Hands>(actor) {
            let took = hands.take_n(yield_item, remaining);
            remaining -= took;
        }
    }

    let taken = count - remaining;
    if remaining > 0 {
        drop_item_near(app, (px, py), (px, py), yield_item, remaining);
    }

    // ── 地形额外产出 ──
    let actor_terrain = app.map.terrain(px, py);
    let extra = terrain_harvest_extra(actor_terrain, rng);
    if let Some((extra_item, extra_count)) = extra {
        let mut extra_remaining = extra_count;
        if let Ok(mut hands) = app.world.get::<&mut Hands>(actor) {
            extra_remaining -= hands.take_n(extra_item, extra_remaining);
        }
        if extra_remaining > 0 {
            drop_item_near(app, (px, py), (px, py), extra_item, extra_remaining);
        }
    }

    let item_name = yield_item.label();
    if taken > 0 && remaining > 0 {
        app.push_log(format!(
            "你从灌木摘了 {} 个{}，手里拿不下的掉在地上。",
            count, item_name
        ));
    } else if taken > 0 {
        app.push_log(format!("你从灌木摘了 {} 个{}。", taken, item_name));
    } else {
        app.push_log(format!("{} 个{}全掉在地上。", count, item_name));
    }
    app.events.push(GameEvent::BushHarvested { count });
    true
}

/// 地形采摘额外产出（浅沼出黏土40%，密林出草药10%）
fn terrain_harvest_extra(terrain: TerrainKind, rng: &mut impl Rng) -> Option<(ItemKind, u32)> {
    match terrain {
        TerrainKind::ShallowMarsh => {
            if rng.gen_bool(0.40) {
                Some((ItemKind::Clay, 1))
            } else {
                None
            }
        }
        TerrainKind::DenseForest => {
            if rng.gen_bool(0.10) {
                Some((ItemKind::Herb, 1))
            } else {
                None
            }
        }
        _ => None,
    }
}
