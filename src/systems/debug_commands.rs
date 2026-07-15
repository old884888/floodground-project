use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{
    App, DebugSubKind, DEBUG_ITEM_COUNT, DEBUG_SUB_CREATURES,
    DEBUG_SUB_SETTLEMENTS, DEBUG_SUB_TERRAIN, DEBUG_SUB_TIME, DEBUG_SUB_TOOLS, DEBUG_SUB_WEATHER,
    DEBUG_TERRAIN_ITEMS, DEBUG_TIME_ITEMS, DEBUG_WEATHER_ITEMS, SETTLEMENT_SIZE_ITEMS,
};
use crate::components::*;
use crate::items::place_item;

pub(crate) fn handle_debug_popup_key(app: &mut App, key: KeyEvent) {
    let has_sub = app.debug_popup.as_ref().is_some_and(|p| p.sub.is_some());

    if has_sub {
        let (sub_kind, sub_cursor) = match &app.debug_popup {
            Some(p) => (p.sub.clone(), p.sub_cursor),
            _ => return,
        };
        let Some(sub_kind) = sub_kind else { return };

        let max_items = match sub_kind {
            DebugSubKind::Tool => crate::app::TOOL_ITEMS.len().saturating_sub(1),
            DebugSubKind::Creature => crate::app::SPAWN_ITEMS.len().saturating_sub(1),
            DebugSubKind::Settlement => SETTLEMENT_SIZE_ITEMS.len().saturating_sub(1),
            DebugSubKind::TimePeriod => DEBUG_TIME_ITEMS.len().saturating_sub(1),
            DebugSubKind::WeatherKind => DEBUG_WEATHER_ITEMS.len().saturating_sub(1),
            DebugSubKind::TerrainItem => DEBUG_TERRAIN_ITEMS.len().saturating_sub(1),
        };

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut p) = app.debug_popup {
                    p.sub_cursor = if p.sub_cursor == 0 { max_items } else { p.sub_cursor - 1 };
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut p) = app.debug_popup {
                    p.sub_cursor = if p.sub_cursor >= max_items { 0 } else { p.sub_cursor + 1 };
                }
            }
            KeyCode::Enter => {
                match sub_kind {
                    DebugSubKind::Tool => debug_spawn_tool(app, sub_cursor),
                    DebugSubKind::Creature => debug_spawn_execute(app, sub_cursor),
                    DebugSubKind::Settlement => {
                        if let Some(&(_, size)) = SETTLEMENT_SIZE_ITEMS.get(sub_cursor) {
                            app.spawn_settlement(size, &mut rand::thread_rng());
                        }
                    }
                    DebugSubKind::TimePeriod => debug_set_time(app, sub_cursor),
                    DebugSubKind::WeatherKind => debug_set_weather(app, sub_cursor),
                    DebugSubKind::TerrainItem => debug_spawn_terrain_item(app, sub_cursor),
                }
                app.debug_popup = None;
            }
            KeyCode::Esc => {
                if let Some(ref mut p) = app.debug_popup {
                    p.sub = None;
                }
            }
            _ => {}
        }
    } else {
        let cursor = app.debug_popup.as_ref().map(|p| p.cursor);

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut p) = app.debug_popup {
                    p.cursor = if p.cursor == 0 { DEBUG_ITEM_COUNT - 1 } else { p.cursor - 1 };
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut p) = app.debug_popup {
                    p.cursor = if p.cursor >= DEBUG_ITEM_COUNT - 1 { 0 } else { p.cursor + 1 };
                }
            }
            KeyCode::Enter => {
                if let Some(cur) = cursor {
                    if cur == DEBUG_SUB_TOOLS {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::Tool);
                            p.sub_cursor = 0;
                        }
                    } else if cur == DEBUG_SUB_CREATURES {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::Creature);
                            p.sub_cursor = 0;
                        }
                    } else if cur == DEBUG_SUB_SETTLEMENTS {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::Settlement);
                            p.sub_cursor = 0;
                        }
                    } else if cur == DEBUG_SUB_TIME {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::TimePeriod);
                            p.sub_cursor = 0;
                        }
                    } else if cur == DEBUG_SUB_WEATHER {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::WeatherKind);
                            p.sub_cursor = 0;
                        }
                    } else if cur == DEBUG_SUB_TERRAIN {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::TerrainItem);
                            p.sub_cursor = 0;
                        }
                    } else {
                        debug_execute(app, cur);
                        app.debug_popup = None;
                    }
                }
            }
            KeyCode::Esc | KeyCode::F(6) => {
                app.debug_popup = None;
            }
            _ => {}
        }
    }
}

pub(crate) fn debug_execute(app: &mut App, idx: usize) {
    match idx {
        0 => {
            let (px, py) = app.actor_pos();
            place_item(app, px, py, ItemKind::Berry, 5);
            app.push_log("（调试）脚下刷了 5 颗莓果。".into());
        }
        1 => {
            let (px, py) = app.actor_pos();
            place_item(app, px, py, ItemKind::Wood, 10);
            app.push_log("（调试）脚下刷了 10 根木头。".into());
        }
        2 => {
            let (px, py) = app.actor_pos();
            place_item(app, px, py, ItemKind::SmallStone, 5);
            app.push_log("（调试）脚下刷了 5 个小石头。".into());
        }
        3 => {
            let (px, py) = app.actor_pos();
            place_item(app, px, py, ItemKind::Stick, 5);
            app.push_log("（调试）脚下刷了 5 根木棍。".into());
        }
        4 => {
            let (px, py) = app.actor_pos();
            place_item(app, px, py, ItemKind::BigStone, 2);
            app.push_log("（调试）脚下刷了 2 个大石头。".into());
        }
        5 => {
            if let Some(actor) = app.actor() {
                if let Ok(mut h) = app.world.get::<&mut Hunger>(actor) {
                    h.value = 100.0;
                }
                if let Ok(mut t) = app.world.get::<&mut Thirst>(actor) {
                    t.value = 100.0;
                }
            }
            app.push_log("（调试）饥渴全满。".into());
        }
        6 => {
            let (px, py) = app.actor_pos();
            let tx = px + app.facing.0;
            let ty = py + app.facing.1;
            if app.map.is_walkable(tx, ty) && !app.is_blocked(tx, ty) {
                app.world.spawn((
                    Position { x: tx, y: ty },
                    Tree,
                    crate::components::BlocksMovement,
                    crate::components::BlocksVision,
                    crate::components::Harvestable {
                        hp: 1000.0,
                        max_hp: 1000.0,
                        yield_item: ItemKind::Wood,
                        yield_hp_step: 100.0,
                    },
                ));
                app.mark_spatial_dirty();
                app.push_log("（调试）面前长出一棵树。".into());
            } else {
                app.push_log("（调试）面前没空地。".into());
            }
        }
        7 => {
            let (px, py) = app.actor_pos();
            let tx = px + app.facing.0;
            let ty = py + app.facing.1;
            if app.map.is_walkable(tx, ty) && !app.is_blocked(tx, ty) {
                app.world.spawn((
                    Position { x: tx, y: ty },
                    Bush {
                        state: BushState::Fruiting,
                        growth_timer: 0,
                        yield_item: ItemKind::Berry,
                    },
                ));
                app.mark_spatial_dirty();
                app.push_log("（调试）面前长出一丛结果的莓果。".into());
            } else {
                app.push_log("（调试）面前没空地。".into());
            }
        }
        8..=10 | 12..=13 => {} // 子菜单，由 Enter 处理
        11 => {
            app.tick += 1200;
            app.push_log(format!(
                "（调试）时间 +2 小时。当前 {:02}:{:02}。",
                app.clock_hm().0,
                app.clock_hm().1,
            ));
        }
        _ => {}
    }
}

pub(crate) fn debug_spawn_execute(app: &mut App, idx: usize) {
    let (px, py) = app.actor_pos();
    let tx = px + app.facing.0;
    let ty = py + app.facing.1;
    if app.is_blocked(tx, ty) {
        app.push_log("（调试）面前没空地。".into());
        return;
    }

    match idx {
        0 => {
            app.world.spawn((
                Position { x: tx, y: ty },
                crate::components::Name("狼".into()),
                crate::components::Hostile,
                crate::components::Health {
                    hp: 50.0,
                    max_hp: 50.0,
                },
                crate::components::Wet { value: 0.0 },
                crate::components::MoveCooldown { ticks: 0 },
            ));
            app.push_log("（调试）生成一只狼。".into());
        }
        1 => {
            app.world.spawn((
                Position { x: tx, y: ty },
                crate::components::Name("新殖民者".into()),
                crate::components::Colonist,
                crate::components::Health {
                    hp: 100.0,
                    max_hp: 100.0,
                },
                Hunger { value: 80.0 },
                Thirst { value: 80.0 },
                crate::components::Energy { value: 80.0 },
                crate::components::Mood { value: 60.0 },
                crate::components::AiState {
                    current: crate::components::Act::Idle,
                },
                crate::components::TraitTag("冷静".into()),
                crate::components::Wet { value: 0.0 },
                crate::components::MoveCooldown { ticks: 0 },
            ));
            app.push_log("（调试）生成一个殖民者。".into());
        }
        2 => {
            app.world.spawn((
                Position { x: tx, y: ty },
                crate::components::Name("新俘虏".into()),
                crate::components::Captive { will: 80.0 },
                crate::components::Health {
                    hp: 70.0,
                    max_hp: 100.0,
                },
                Hunger { value: 50.0 },
                Thirst { value: 50.0 },
                crate::components::Energy { value: 50.0 },
                crate::components::Mood { value: 20.0 },
                crate::components::Wet { value: 0.0 },
                crate::components::MoveCooldown { ticks: 0 },
            ));
            app.push_log("（调试）生成一个俘虏。".into());
        }
        _ => {}
    }
    app.mark_spatial_dirty();
}

pub(crate) fn debug_spawn_tool(app: &mut App, idx: usize) {
    let (px, py) = app.actor_pos();
    let (item, label): (ItemKind, &str) = match idx {
        0 => (ItemKind::StoneKnife, "石刀"),
        1 => (ItemKind::SharpStick, "削尖棍"),
        2 => (ItemKind::Spear, "矛"),
        3 => (ItemKind::StoneAxe, "石斧"),
        4 => (ItemKind::Torch, "火把"),
        _ => return,
    };
    place_item(app, px, py, item, 1);
    app.push_log(format!("（调试）脚下刷了一把{}。", label));
}

/// 直接跳到指定时段（二级菜单选择）
pub(crate) fn debug_set_time(app: &mut App, idx: usize) {
    // 黎明=5%, 白天=35%, 黄昏=65%, 夜晚=85%
    let target = match idx {
        0 => 0.05, // 黎明
        1 => 0.35, // 白天
        2 => 0.65, // 黄昏
        _ => 0.85, // 夜晚
    };
    jump_day_progress(app, target);
    app.push_log(format!(
        "（调试）时间跳到{} {:02}:{:02}。",
        app.period_label(),
        app.clock_hm().0,
        app.clock_hm().1,
    ));
}

/// 直接设置天气（二级菜单选择）
pub(crate) fn debug_set_weather(app: &mut App, idx: usize) {
    use crate::app::Weather;
    let weather = match idx {
        0 => Weather::Clear,
        1 => Weather::Overcast,
        2 => Weather::Drizzle,
        3 => Weather::Rain,
        4 => Weather::Heavy,
        _ => Weather::Thunder,
    };
    let old = app.weather;
    app.weather = weather;
    app.weather_timer = 500; // 给个够长的持续时间
    app.weather_mood_tracker.clear();
    app.events.push(crate::events::GameEvent::WeatherChanged { from: old, to: weather });
    app.push_log(format!("（调试）天气设为{}。", weather.label()));
}

pub(crate) fn debug_spawn_terrain_item(app: &mut App, idx: usize) {
    let (px, py) = app.actor_pos();
    match idx {
        0 => {
            // 脚下刷草药
            place_item(app, px, py, ItemKind::Herb, 3);
            app.push_log("（调试）脚下刷了 3 株草药。".into());
        }
        1 => {
            // 脚下刷黏土
            place_item(app, px, py, ItemKind::Clay, 3);
            app.push_log("（调试）脚下刷了 3 团黏土。".into());
        }
        2 => {
            // 脚下刷金属矿
            place_item(app, px, py, ItemKind::MetalOre, 3);
            app.push_log("（调试）脚下刷了 3 块金属矿。".into());
        }
        3 => {
            // 脚下刷毒蘑菇
            place_item(app, px, py, ItemKind::PoisonMush, 3);
            app.push_log("（调试）脚下刷了 3 朵毒蘑菇。".into());
        }
        4 => {
            // 面前放草药植株
            let tx = px + app.facing.0;
            let ty = py + app.facing.1;
            if app.map.is_walkable(tx, ty) && !app.is_blocked(tx, ty) {
                app.world.spawn((
                    Position { x: tx, y: ty },
                    crate::components::Bush {
                        state: crate::components::BushState::Fruiting,
                        growth_timer: 0,
                        yield_item: ItemKind::Herb,
                    },
                ));
                app.mark_spatial_dirty();
                app.push_log("（调试）面前长了一株草药。".into());
            } else {
                app.push_log("（调试）面前没空地。".into());
            }
        }
        5 => {
            // 面前放狼巢穴
            let tx = px + app.facing.0;
            let ty = py + app.facing.1;
            if app.map.is_walkable(tx, ty) && !app.is_blocked(tx, ty) {
                app.world.spawn((
                    Position { x: tx, y: ty },
                    crate::components::WolfDen,
                ));
                app.mark_spatial_dirty();
                app.push_log("（调试）面前放了一个狼巢穴。".into());
            } else {
                app.push_log("（调试）面前没空地放巢穴。".into());
            }
        }
        // Plan 08 新物品
        6 => { place_item(app, px, py, ItemKind::Branch, 5); app.push_log("（调试）脚下刷了 5 根树枝。".into()); }
        7 => { place_item(app, px, py, ItemKind::Leaves, 5); app.push_log("（调试）脚下刷了 5 片树叶。".into()); }
        8 => { place_item(app, px, py, ItemKind::LongStick, 3); app.push_log("（调试）脚下刷了 3 根长木棍。".into()); }
        9 => { place_item(app, px, py, ItemKind::Vine, 3); app.push_log("（调试）脚下刷了 3 根藤条。".into()); }
        10 => { place_item(app, px, py, ItemKind::Rope, 3); app.push_log("（调试）脚下刷了 3 根绳子。".into()); }
        11 => { place_item(app, px, py, ItemKind::SmallFlake, 5); app.push_log("（调试）脚下刷了 5 片石片。".into()); }
        12 => { place_item(app, px, py, ItemKind::LargeFlake, 3); app.push_log("（调试）脚下刷了 3 片大石片。".into()); }
        13 => { place_item(app, px, py, ItemKind::Bone, 3); app.push_log("（调试）脚下刷了 3 根骨头。".into()); }
        _ => {}
    }
}

pub(crate) fn debug_teleport(app: &mut App) {
    let Some(actor) = app.actor() else {
        return;
    };
    let Some((tx, ty)) = app.focused_tile else {
        return;
    };
    if !app.map.in_bounds(tx, ty) {
        app.push_log("光标越界了。".into());
        return;
    }
    if !app.map.is_walkable(tx, ty) {
        app.push_log("那个地方走不动——你又不是鱼。".into());
        return;
    }
    if app.is_blocked(tx, ty) {
        app.push_log("那个位置被别的什么东西占了。".into());
        return;
    }
    if let Ok(mut pos) = app.world.get::<&mut Position>(actor) {
        pos.x = tx;
        pos.y = ty;
    }
    app.focused_tile = None;
    app.observe_scroll = 0;
    app.last_actor_terrain = None; // 瞬移后：让下一步按新地形重新判断日志
    app.push_log(format!("你瞬移到了 ({}, {})。", tx, ty));
}

pub(crate) fn jump_day_progress(app: &mut App, target_progress: f32) {
    let tpd = app.ticks_per_day.max(1);
    let target = (tpd as f32 * target_progress) as u64;
    let cur = app.tick % tpd;
    let add = if target > cur {
        target - cur
    } else {
        tpd - cur + target
    };
    app.tick += add;
}

// ── 建造菜单键盘处理 ──

