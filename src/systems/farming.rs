//! 种植系统：翻土 / 播种 / 生长 / 收获
//!
//! 'f' 键上下文敏感：
//! - 面前草地 + 手持铲 → 翻土（生成 Farmland）
//! - 面前 Farmland 空地 + 手持种子 → 播种
//! - 面前 Farmland 成熟作物 → 收获

use rand::Rng;

use crate::app::App;
use crate::components::*;
use crate::items::place_item;

/// 各阶段累计 tick 阈值（从播种起算）
const STAGE_SPROUT: u32 = 2000;
const STAGE_GROWING: u32 = 6000;
const STAGE_RIPE: u32 = 12000;

/// 'f' 键：耕/种/收，按面前格子状态分流
pub fn try_farm(app: &mut App, rng: &mut impl Rng) {
    let Some(actor) = app.actor() else {
        app.push_log("没有能动的人。".into());
        return;
    };
    let (px, py) = match app.world.get::<&Position>(actor) {
        Ok(p) => (p.x, p.y), Err(_) => return,
    };
    let (fx, fy) = (px + app.facing.0, py + app.facing.1);

    if !app.map.in_bounds(fx, fy) {
        app.push_log("那边什么都没有。".into());
        return;
    }

    // 找面前格的 Farmland 实体
    let farmland_e = app.world.query::<&Position>().with::<&Farmland>().iter()
        .find(|(_, p)| p.x == fx && p.y == fy)
        .map(|(e, _)| e);

    if let Some(fe) = farmland_e {
        // ── 有耕地 ──
        let has_crop = app.world.get::<&Crop>(fe).is_ok();
        if has_crop {
            // 有作物 → 看是否成熟
            let stage = app.world.get::<&Crop>(fe).map(|c| c.stage).unwrap();
            if stage == CropStage::Ripe {
                // 收获
                let _ = app.world.remove_one::<Crop>(fe);
                let berries = rng.gen_range(2..=3);
                place_item(app, fx, fy, ItemKind::Berry, berries);
                if rng.gen_bool(0.50) {
                    place_item(app, fx, fy, ItemKind::Seed, 1);
                }
                let who = app.entity_label(actor);
                app.push_log(format!("{}收了一把浆果——种地没白干。", who));
                app.force_step = true;
            } else {
                let label = match stage {
                    CropStage::Seed => "刚播下，地里啥也看不见",
                    CropStage::Sprout => "冒出了嫩绿的小苗",
                    CropStage::Growing => "长到半人高，还没结果",
                    CropStage::Ripe => unreachable!(),
                };
                app.push_log(format!("{}——再等等。", label));
            }
        } else {
            // 空耕地 → 播种（需要手上有种子）
            let has_seed = app.world.get::<&Hands>(actor)
                .map(|h| h.left.is_some_and(|(k, _)| k == ItemKind::Seed)
                      || h.right.is_some_and(|(k, _)| k == ItemKind::Seed))
                .unwrap_or(false);
            if has_seed {
                // 消耗 1 颗种子（右手优先）
                let from_right = app.world.get::<&Hands>(actor)
                    .map(|h| h.right.is_some_and(|(k, _)| k == ItemKind::Seed))
                    .unwrap_or(false);
                {
                    let Ok(mut hands) = app.world.get::<&mut Hands>(actor) else { return };
                    let slot = if from_right { &mut hands.right } else { &mut hands.left };
                    if let Some((_k, count)) = slot.as_mut() {
                        *count -= 1;
                        if *count == 0 { *slot = None; }
                    }
                }
                let _ = app.world.insert_one(fe, Crop { stage: CropStage::Seed, growth: 0 });
                let who = app.entity_label(actor);
                app.push_log(format!("{}把种子按进了土里。", who));
                app.force_step = true;
            } else {
                app.push_log("手里没有种子——采浆果丛有概率掉种子。".into());
            }
        }
        return;
    }

    // ── 没耕地 → 尝试翻土 ──
    let terrain = app.map.terrain(fx, fy);
    if !matches!(terrain, TerrainKind::Grass | TerrainKind::LightForest | TerrainKind::Dirt) {
        app.push_log("这里没法翻地——草地、疏林或泥地才行。".into());
        return;
    }
    // 被占？
    if app.is_blocked(fx, fy) {
        app.push_log("那格有东西挡着，翻不了。".into());
        return;
    }
    // 需要铲子
    let has_shovel = app.world.get::<&Hands>(actor)
        .map(|h| {
            h.left.is_some_and(|(k, _)| matches!(k, ItemKind::StoneShovel | ItemKind::WoodShovel))
            || h.right.is_some_and(|(k, _)| matches!(k, ItemKind::StoneShovel | ItemKind::WoodShovel))
        })
        .unwrap_or(false);
    if !has_shovel {
        app.push_log("翻地得有铲子——石铲或木铲。".into());
        return;
    }

    let uid = app.next_uid; app.next_uid += 1;
    app.world.spawn((
        Position { x: fx, y: fy },
        EntityUID(uid),
        Farmland,
    ));
    app.mark_spatial_dirty();
    // 铲子磨损
    crate::systems::harvest::apply_wear(app, actor, rng);
    let who = app.entity_label(actor);
    app.push_log(format!("{}翻好了一块地——累，但值。", who));
    app.force_step = true;
}

/// 每 tick 推进所有作物生长
pub fn update_crops(app: &mut App) {
    for (_e, crop) in app.world.query::<&mut Crop>().iter() {
        crop.growth = crop.growth.saturating_add(1);
        crop.stage = if crop.growth >= STAGE_RIPE {
            CropStage::Ripe
        } else if crop.growth >= STAGE_GROWING {
            CropStage::Growing
        } else if crop.growth >= STAGE_SPROUT {
            CropStage::Sprout
        } else {
            CropStage::Seed
        };
    }
}
