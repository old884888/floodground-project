//! 殖民者 AI — 8 级优先级链。
//!
//! 每 tick 每个殖民者从高到低扫描，命中则执行。
//! AI 直接操作组件（不走玩家交互管线）。

use rand::Rng;

use crate::app::App;
use crate::components::{
    AiState, Bush, BushState, Campfire, Dead, Hands, Hunger, ItemKind,
    MoveCooldown, Pile, Position, Thirst, BodyTemp, Tree, Boulder, Corpse,
};

const MAX_CAMP_DIST: i32 = 25;
/// 搬运/添柴的"远离营火"阈值：超过此距离的散落物才搬
const HAUL_FAR_DIST: i32 = 5;
/// 添柴维持的营火邻格木头数
const FIREWOOD_TARGET: u32 = 2;

pub fn update_ai(app: &mut App, rng: &mut impl Rng) {
    let actors: Vec<hecs::Entity> = app.world.query::<&AiState>()
        .with::<&crate::components::Colonist>()
        .iter()
        .filter(|(e, _)| app.world.get::<&Dead>(*e).is_err())
        .map(|(e, _)| e)
        .collect();

    let saved_selected = app.selected;

    for entity in actors {
        if let Ok(cd) = app.world.get::<&MoveCooldown>(entity) {
            if cd.ticks > 0 { continue; }
        }

        let (ex, ey) = match app.world.get::<&Position>(entity) {
            Ok(p) => (p.x, p.y), Err(_) => continue,
        };

        let hunger_val = app.world.get::<&Hunger>(entity).map(|h| h.value).unwrap_or(50.0);
        let thirst_val = app.world.get::<&Thirst>(entity).map(|t| t.value).unwrap_or(50.0);
        let body_temp = app.world.get::<&BodyTemp>(entity).map(|b| b.value).unwrap_or(60.0);
        let hp = app.world.get::<&crate::components::Health>(entity).map(|h| h.hp).unwrap_or(100.0);

        // ── P1 自救 ──
        if body_temp < 25.0 {
            if let Some((fx, fy)) = nearest_fire(app, ex, ey) {
                crate::systems::movement::step_toward(app, entity, fx, fy);
                continue;
            }
        }
        if hunger_val < 30.0 {
            if let Some((fx, fy, slot_i)) = nearest_edible(app, ex, ey) {
                if adjacent(ex, ey, fx, fy) {
                    ai_eat(app, entity, fx, fy, slot_i);
                } else {
                    crate::systems::movement::step_toward(app, entity, fx, fy);
                }
                continue;
            }
        }
        if thirst_val < 30.0 {
            if let Some((wx, wy)) = nearest_water(app, ex, ey) {
                if adjacent(ex, ey, wx, wy) {
                    if let Ok(mut t) = app.world.get::<&mut Thirst>(entity) {
                        t.value = (t.value + 30.0).min(100.0);
                    }
                    let label = app.entity_label(entity);
                    app.push_log(format!("{}趴在水边猛灌了几口。", label));
                } else {
                    crate::systems::movement::step_toward(app, entity, wx, wy);
                }
                continue;
            }
        }
        if hp < 30.0 {
            let cx = crate::world::CAMP_CX;
            let cy = crate::world::CAMP_CY;
            if (ex - cx).abs().max((ey - cy).abs()) > 3 {
                crate::systems::movement::step_toward(app, entity, cx, cy);
                continue;
            }
        }

        // ── P1.5 运货归还：双手有材料 → 回营火丢 ──
        // 工具（非 stackable）不丢，留着干活；材料（stackable）搬回营地。
        if let Some((_item, from_right)) = carrying_material(app, entity) {
            if let Some((dx, dy)) = campfire_adjacent_drop_tile(app, ex, ey) {
                if adjacent(ex, ey, dx, dy) {
                    ai_drop_carry(app, entity, dx, dy, from_right);
                } else {
                    crate::systems::movement::step_toward(app, entity, dx, dy);
                }
                continue;
            } else {
                // 找不到营火邻格空地 → 原地丢，避免卡死
                ai_drop_carry(app, entity, ex, ey, from_right);
                continue;
            }
        }

        // ── P2 屠宰：有刀 + 视野内有尸体 → 走过去剥 ──
        if has_knife(app, entity) {
            if let Some((cx, cy)) = nearest_corpse(app, ex, ey) {
                if adjacent(ex, ey, cx, cy) {
                    let _ = crate::systems::butcher::butcher_corpse(app, entity, cx, cy, rng);
                } else {
                    crate::systems::movement::step_toward(app, entity, cx, cy);
                }
                continue;
            }
        }

        // ── P3 添柴：营火邻格木头 < 阈值 → 去捡木头 ──
        if campfire_adjacent_wood_count(app) < FIREWOOD_TARGET {
            if let Some((px, py, si)) = far_pile_with_item(app, ex, ey, ItemKind::Wood) {
                if adjacent(ex, ey, px, py) {
                    ai_pickup(app, entity, px, py, si);
                } else {
                    crate::systems::movement::step_toward(app, entity, px, py);
                }
                continue;
            }
        }

        // ── P4 采摘 ──
        if let Some((bx, by)) = nearest_fruiting_bush(app, ex, ey) {
            if adjacent(ex, ey, bx, by) {
                app.selected = Some(entity);
                crate::systems::harvest::try_harvest_bush(app, rng);
            } else {
                crate::systems::movement::step_toward(app, entity, bx, by);
            }
            continue;
        }

        // ── P5 砍树 ──
        if has_item(app, entity, ItemKind::StoneAxe) || has_item(app, entity, ItemKind::WoodAxe) {
            if let Some((tx, ty)) = nearest_tree(app, ex, ey) {
                if adjacent(ex, ey, tx, ty) {
                    app.selected = Some(entity);
                    crate::systems::harvest::try_chop(app, rng);
                } else {
                    crate::systems::movement::step_toward(app, entity, tx, ty);
                }
                continue;
            }
        }

        // ── P6 挖矿 ──
        if has_item(app, entity, ItemKind::StoneHammer) {
            if let Some((rx, ry)) = nearest_boulder(app, ex, ey) {
                if adjacent(ex, ey, rx, ry) {
                    app.selected = Some(entity);
                    crate::systems::harvest::try_mine(app, rng);
                } else {
                    crate::systems::movement::step_toward(app, entity, rx, ry);
                }
                continue;
            }
        }

        // ── P7 搬运：双手空 + 远处有散落物 → 捡 ──
        if hands_empty(app, entity) {
            if let Some((px, py, si)) = far_pile_any(app, ex, ey) {
                if adjacent(ex, ey, px, py) {
                    ai_pickup(app, entity, px, py, si);
                } else {
                    crate::systems::movement::step_toward(app, entity, px, py);
                }
                continue;
            }
        }

        // ── P8 晃荡 ──
        random_wander(app, entity, ex, ey, rng);
    }

    app.selected = saved_selected;
}

// ── 辅助函数 ──

fn adjacent(x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
    (x1 - x2).abs() + (y1 - y2).abs() <= 1
}

fn has_item(app: &App, entity: hecs::Entity, item: ItemKind) -> bool {
    app.world.get::<&Hands>(entity)
        .map(|h| h.left.is_some_and(|(k, _)| k == item)
              || h.right.is_some_and(|(k, _)| k == item))
        .unwrap_or(false)
}

fn has_knife(app: &App, entity: hecs::Entity) -> bool {
    has_item(app, entity, ItemKind::StoneKnife)
        || has_item(app, entity, ItemKind::WoodKnife)
        || has_item(app, entity, ItemKind::BoneKnife)
}

fn hands_empty(app: &App, entity: hecs::Entity) -> bool {
    app.world.get::<&Hands>(entity)
        .map(|h| h.is_empty())
        .unwrap_or(true)
}

/// 是否正扛着材料（stackable 物品）。返回 (物品, 是否右手)。
/// 只有两手都没有工具时才算"运货"——有一手是工具就留着干活。
fn carrying_material(app: &App, entity: hecs::Entity) -> Option<(ItemKind, bool)> {
    let hands = app.world.get::<&Hands>(entity).ok()?;
    let left = hands.left;
    let right = hands.right;
    let has_tool = |slot: Option<(ItemKind, u32)>| {
        slot.is_some_and(|(k, _)| !crate::data::item_def(k.key()).stackable)
    };
    if has_tool(left) || has_tool(right) { return None; }
    // 没工具，有材料 → 返回材料（右手优先，跟 drop_one 一致）
    if let Some((k, _)) = right { return Some((k, true)); }
    if let Some((k, _)) = left { return Some((k, false)); }
    None
}

fn nearest_fire(app: &App, ex: i32, ey: i32) -> Option<(i32, i32)> {
    use crate::components::LightSource;
    let mut best: Option<((i32, i32), i32)> = None;
    for (_, pos) in app.world.query::<&Position>().with::<&LightSource>().iter() {
        let dist = (pos.x - ex).abs().max((pos.y - ey).abs());
        if dist <= 20 && best.as_ref().map(|(_, d)| dist < *d).unwrap_or(true) {
            best = Some(((pos.x, pos.y), dist));
        }
    }
    best.map(|(p, _)| p)
}

/// 最近的营火（Campfire 标记，不是火把）
fn nearest_campfire(app: &App, ex: i32, ey: i32) -> Option<(i32, i32)> {
    let mut best: Option<((i32, i32), i32)> = None;
    for (_, pos) in app.world.query::<&Position>().with::<&Campfire>().iter() {
        let dist = (pos.x - ex).abs().max((pos.y - ey).abs());
        if best.as_ref().map(|(_, d)| dist < *d).unwrap_or(true) {
            best = Some(((pos.x, pos.y), dist));
        }
    }
    best.map(|(p, _)| p)
}

/// 找一个营火邻格的空地（可走、没被占），用于丢东西。
fn campfire_adjacent_drop_tile(app: &App, ex: i32, ey: i32) -> Option<(i32, i32)> {
    let (fx, fy) = nearest_campfire(app, ex, ey)?;
    for &(dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
        let nx = fx + dx;
        let ny = fy + dy;
        if app.map.is_walkable(nx, ny) && !app.is_blocked(nx, ny) {
            return Some((nx, ny));
        }
    }
    None
}

/// 统计所有营火邻格 Pile 里的 Wood 总数
fn campfire_adjacent_wood_count(app: &App) -> u32 {
    let mut total: u32 = 0;
    let fires: Vec<(i32, i32)> = app.world.query::<&Position>().with::<&Campfire>().iter()
        .map(|(_, p)| (p.x, p.y)).collect();
    for (fx, fy) in fires {
        for &(dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
            if let Some(e) = crate::items::pile_at(app, fx + dx, fy + dy) {
                if let Ok(pile) = app.world.get::<&Pile>(e) {
                    for slot in &pile.slots {
                        if slot.item == ItemKind::Wood { total = total.saturating_add(slot.count); }
                    }
                }
            }
        }
    }
    total
}

fn nearest_corpse(app: &App, ex: i32, ey: i32) -> Option<(i32, i32)> {
    let mut best: Option<((i32, i32), i32)> = None;
    for (_, (pos, _corpse)) in app.world.query::<(&Position, &Corpse)>().iter() {
        let dist = (pos.x - ex).abs().max((pos.y - ey).abs());
        if dist <= 15 && best.as_ref().map(|(_, d)| dist < *d).unwrap_or(true) {
            best = Some(((pos.x, pos.y), dist));
        }
    }
    best.map(|(p, _)| p)
}

/// 找远离营火（>HAUL_FAR_DIST）且含指定物品的 Pile。
/// 返回 (x, y, slot_index)。
fn far_pile_with_item(app: &App, ex: i32, ey: i32, item: ItemKind) -> Option<(i32, i32, usize)> {
    let cx = crate::world::CAMP_CX;
    let cy = crate::world::CAMP_CY;
    let mut best: Option<((i32, i32), usize, i32)> = None;
    for (_, (pos, pile)) in app.world.query::<(&Position, &Pile)>().iter() {
        let from_camp = (pos.x - cx).abs().max((pos.y - cy).abs());
        if from_camp <= HAUL_FAR_DIST { continue; }
        for (si, slot) in pile.slots.iter().enumerate() {
            if slot.item != item { continue; }
            let dist = (pos.x - ex).abs().max((pos.y - ey).abs());
            if dist <= 20 && best.as_ref().map(|(_, _, d)| dist < *d).unwrap_or(true) {
                best = Some(((pos.x, pos.y), si, dist));
            }
        }
    }
    best.map(|((x, y), si, _)| (x, y, si))
}

/// 找远离营火的任意 Pile（取 dominant slot）。
fn far_pile_any(app: &App, ex: i32, ey: i32) -> Option<(i32, i32, usize)> {
    let cx = crate::world::CAMP_CX;
    let cy = crate::world::CAMP_CY;
    let mut best: Option<((i32, i32), usize, i32)> = None;
    for (_, (pos, pile)) in app.world.query::<(&Position, &Pile)>().iter() {
        let from_camp = (pos.x - cx).abs().max((pos.y - cy).abs());
        if from_camp <= HAUL_FAR_DIST { continue; }
        let Some((item, _)) = pile.dominant() else { continue; };
        let si = pile.slots.iter().position(|s| s.item == item)?;
        let dist = (pos.x - ex).abs().max((pos.y - ey).abs());
        if dist <= 20 && best.as_ref().map(|(_, _, d)| dist < *d).unwrap_or(true) {
            best = Some(((pos.x, pos.y), si, dist));
        }
    }
    best.map(|((x, y), si, _)| (x, y, si))
}

fn nearest_edible(app: &App, ex: i32, ey: i32) -> Option<(i32, i32, usize)> {
    let mut best: Option<((i32, i32), usize, i32)> = None;
    for (_, (pos, pile)) in app.world.query::<(&Position, &Pile)>().iter() {
        for (si, slot) in pile.slots.iter().enumerate() {
            let edible = matches!(slot.item,
                ItemKind::CookedMeat | ItemKind::SmokedMeat | ItemKind::RawMeat | ItemKind::Berry);
            if !edible { continue; }
            let dist = (pos.x - ex).abs().max((pos.y - ey).abs());
            if dist <= 15 && best.as_ref().map(|(_, _, d)| dist < *d).unwrap_or(true) {
                best = Some(((pos.x, pos.y), si, dist));
            }
        }
    }
    best.map(|((x, y), si, _)| (x, y, si))
}

fn nearest_water(app: &App, ex: i32, ey: i32) -> Option<(i32, i32)> {
    use crate::components::TerrainKind;
    let mut best: Option<((i32, i32), i32)> = None;
    for y in (ey - 20).max(0)..(ey + 20).min(crate::world::MAP_HEIGHT) {
        for x in (ex - 20).max(0)..(ex + 20).min(crate::world::MAP_WIDTH) {
            let t = app.map.terrain(x, y);
            if !matches!(t, TerrainKind::ShallowWater | TerrainKind::Water | TerrainKind::Stream) {
                continue;
            }
            let dist = (x - ex).abs().max((y - ey).abs());
            if best.as_ref().map(|(_, d)| dist < *d).unwrap_or(true) {
                best = Some(((x, y), dist));
            }
        }
    }
    best.map(|(p, _)| p)
}

fn nearest_fruiting_bush(app: &App, ex: i32, ey: i32) -> Option<(i32, i32)> {
    let mut best: Option<((i32, i32), i32)> = None;
    for (_, (pos, bush)) in app.world.query::<(&Position, &Bush)>().iter() {
        if bush.state != BushState::Fruiting { continue; }
        let dist = (pos.x - ex).abs().max((pos.y - ey).abs());
        if dist <= 15 && best.as_ref().map(|(_, d)| dist < *d).unwrap_or(true) {
            best = Some(((pos.x, pos.y), dist));
        }
    }
    best.map(|(p, _)| p)
}

fn nearest_tree(app: &App, ex: i32, ey: i32) -> Option<(i32, i32)> {
    let mut best: Option<((i32, i32), i32)> = None;
    for (_, pos) in app.world.query::<&Position>().with::<&Tree>().iter() {
        let dist = (pos.x - ex).abs().max((pos.y - ey).abs());
        if dist <= 20 && best.as_ref().map(|(_, d)| dist < *d).unwrap_or(true) {
            best = Some(((pos.x, pos.y), dist));
        }
    }
    best.map(|(p, _)| p)
}

fn nearest_boulder(app: &App, ex: i32, ey: i32) -> Option<(i32, i32)> {
    let mut best: Option<((i32, i32), i32)> = None;
    for (_, pos) in app.world.query::<&Position>().with::<&Boulder>().iter() {
        let dist = (pos.x - ex).abs().max((pos.y - ey).abs());
        if dist <= 20 && best.as_ref().map(|(_, d)| dist < *d).unwrap_or(true) {
            best = Some(((pos.x, pos.y), dist));
        }
    }
    best.map(|(p, _)| p)
}

fn ai_eat(app: &mut App, entity: hecs::Entity, x: i32, y: i32, slot_index: usize) {
    let pile_e = app.world.query::<&Position>().with::<&Pile>().iter()
        .find(|(_, p)| p.x == x && p.y == y)
        .map(|(e, _)| e);

    let Some(pile_e) = pile_e else { return };

    let mut hunger_add: f32 = 20.0;
    let mut thirst_add: f32 = 0.0;
    let do_despawn: bool;
    {
        let Ok(mut pile) = app.world.get::<&mut Pile>(pile_e) else { return };
        if let Some((item_kind, _)) = pile.take_slot(slot_index, 1) {
            if let Some(food) = app.food_data.get(item_kind.key()) {
                hunger_add = food.hunger;
                thirst_add = food.thirst;
            }
        }
        do_despawn = pile.is_empty();
    }

    if do_despawn {
        let _ = app.world.despawn(pile_e);
        app.mark_spatial_dirty();
    }

    if let Ok(mut h) = app.world.get::<&mut Hunger>(entity) {
        h.value = (h.value + hunger_add).min(100.0);
    }
    if let Ok(mut t) = app.world.get::<&mut Thirst>(entity) {
        t.value = (t.value + thirst_add).min(100.0);
    }
    app.push_log(format!("{}狼吞虎咽地吃了东西。", app.entity_label(entity)));
}

/// AI 从 (x,y) 的 Pile slot 捡 1 件到双手
fn ai_pickup(app: &mut App, entity: hecs::Entity, x: i32, y: i32, slot_index: usize) {
    let pile_e = app.world.query::<&Position>().with::<&Pile>().iter()
        .find(|(_, p)| p.x == x && p.y == y)
        .map(|(e, _)| e);
    let Some(pile_e) = pile_e else { return };

    let taken: Option<(ItemKind, u32)>;
    let do_despawn: bool;
    {
        let Ok(mut pile) = app.world.get::<&mut Pile>(pile_e) else { return };
        taken = pile.take_slot(slot_index, 1);
        do_despawn = pile.is_empty();
    }
    if let Some((item, count)) = taken {
        if let Ok(mut hands) = app.world.get::<&mut Hands>(entity) {
            hands.take_n(item, count);
        }
        let label = app.entity_label(entity);
        app.push_log(format!("{}弯腰捡起了{}。", label, item.label()));
    }
    if do_despawn {
        let _ = app.world.despawn(pile_e);
        app.mark_spatial_dirty();
    }
}

/// AI 把手上材料丢 1 件到 (x,y)
fn ai_drop_carry(app: &mut App, entity: hecs::Entity, x: i32, y: i32, from_right: bool) {
    let dropped: Option<ItemKind>;
    {
        let Ok(mut hands) = app.world.get::<&mut Hands>(entity) else { return };
        dropped = drop_one_from_slot(&mut hands, from_right);
    }
    if let Some(item) = dropped {
        if crate::items::place_item(app, x, y, item, 1) {
            let label = app.entity_label(entity);
            app.push_log(format!("{}把{}放下了。", label, item.label()));
        }
    }
}

/// 从指定手（右/左）丢 1 件。只动那只手。
fn drop_one_from_slot(hands: &mut Hands, from_right: bool) -> Option<ItemKind> {
    let slot = if from_right { &mut hands.right } else { &mut hands.left };
    let (kind, count) = slot.as_mut()?;
    let item = *kind;
    *count -= 1;
    if *count == 0 { *slot = None; }
    Some(item)
}

fn random_wander(app: &mut App, entity: hecs::Entity, ex: i32, ey: i32, rng: &mut impl Rng) {
    let cx = crate::world::CAMP_CX;
    let cy = crate::world::CAMP_CY;
    let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1), (0, 0)];
    let (dx, dy) = dirs[rng.gen_range(0..dirs.len())];
    let nx = ex + dx;
    let ny = ey + dy;
    if (nx - cx).abs().max((ny - cy).abs()) > MAX_CAMP_DIST { return; }
    if !app.map.is_walkable(nx, ny) || app.is_blocked(nx, ny) { return; }
    if let Ok(mut pos) = app.world.get::<&mut Position>(entity) {
        pos.x = nx; pos.y = ny;
    }
    crate::systems::building::trigger_trap_at(app, nx, ny, entity);
    app.mark_spatial_dirty();
}
