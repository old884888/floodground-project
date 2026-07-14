//! 建造系统：CDDA 式逐格建造
//!
//! B 键打开建造菜单 → 选配方 → （非屋顶）选方向 → 收集材料 → 进度条 → 实体生成
//! 完全复用制作系统的材料搜索和 CraftingState 进度机制。

use rand::Rng;

use crate::app::{App, BuildMenuState};
use crate::components::*;
use crate::items::has_wall_at;

// ── 建造配方 ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTarget {
    WoodWall,
    StoneWall,
    WoodDoor,
    WoodRoof,
    StickTrap,
}

pub struct BuildRecipe {
    pub name: &'static str,
    /// (材料, 数量)
    pub ingredients: &'static [(ItemKind, u32)],
    pub result: BuildTarget,
    /// 基础制作进度（ticks）
    pub base_progress: u32,
    /// 是否需要篝火邻格
    pub requires_fire: bool,
    /// 最低光照等级
    pub min_light: u8,
    /// 是否建在脚下（不需要选方向）
    pub self_target: bool,
    /// 制作描述
    pub build_desc: &'static str,
    /// 配方说明（浏览视图展示）
    pub desc: &'static str,
}

pub static BUILD_RECIPES: &[BuildRecipe] = &[
    BuildRecipe {
        name: "木墙",
        ingredients: &[(ItemKind::Wood, 3)],
        result: BuildTarget::WoodWall,
        base_progress: 400,
        requires_fire: false,
        min_light: 1,
        self_target: false,
        build_desc: "正在搭建木墙...",
        desc: "用劈好的木料搭一面墙。不结实，但总比没有强——至少狼得多撞几下。",
    },
    BuildRecipe {
        name: "石墙",
        ingredients: &[(ItemKind::BigStone, 3)],
        result: BuildTarget::StoneWall,
        base_progress: 800,
        requires_fire: false,
        min_light: 1,
        self_target: false,
        build_desc: "正在垒砌石墙...",
        desc: "垒石为墙。沉重、结实、让冬天闭嘴。虽然垒起来比木墙慢一倍。",
    },
    BuildRecipe {
        name: "木门",
        ingredients: &[(ItemKind::Wood, 2)],
        result: BuildTarget::WoodDoor,
        base_progress: 300,
        requires_fire: false,
        min_light: 1,
        self_target: false,
        build_desc: "正在拼装木门...",
        desc: "一扇吱嘎响的木门。进出自如——对你是，对狼也是。记得关门。",
    },
    BuildRecipe {
        name: "木屋顶",
        ingredients: &[(ItemKind::Wood, 3), (ItemKind::Stick, 2)],
        result: BuildTarget::WoodRoof,
        base_progress: 200,
        requires_fire: false,
        min_light: 0,
        self_target: true,
        build_desc: "正在铺设屋顶...",
        desc: "建在脚下的天花板。必须挨着墙或已有屋顶——悬空的顶不存在的。有了它才算室内。",
    },
    BuildRecipe {
        name: "尖刺陷阱",
        ingredients: &[(ItemKind::Stick, 3)],
        result: BuildTarget::StickTrap,
        base_progress: 150,
        requires_fire: false,
        min_light: 1,
        self_target: false,
        build_desc: "正在削尖木棍布置陷阱...",
        desc: "削尖的木棍埋在浅坑里。踩上去不分敌我——你自己中招的那一刻最疼，也最好笑。",
    },
];

pub fn recipe_count() -> usize {
    BUILD_RECIPES.len()
}

/// UI 用：材料是否够（不检查目标格）
pub fn can_afford(app: &App, recipe_index: usize) -> bool {
    let Some(recipe) = BUILD_RECIPES.get(recipe_index) else { return false };
    let (cx, cy) = app.actor_pos();
    let counts = count_available(app, recipe, cx, cy);
    recipe.ingredients.iter().enumerate()
        .all(|(i, &(_, needed))| counts.get(i).copied().unwrap_or(0) >= needed)
}

// ── 材料检查 ──

/// 统计 dist≤2 范围内每种材料的可用量
fn count_available(app: &App, recipe: &BuildRecipe, cx: i32, cy: i32) -> Vec<u32> {
    let mut counts = vec![0u32; recipe.ingredients.len()];
    // 双手
    if let Some(actor) = app.actor() {
        if let Ok(hands) = app.world.get::<&Hands>(actor) {
            for (i, &(item, _)) in recipe.ingredients.iter().enumerate() {
                counts[i] += count_in_hand(&hands, item);
            }
        }
    }
    // 脚下 + dist≤2
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            if dx.abs() + dy.abs() > 2 { continue; }
            let x = cx + dx;
            let y = cy + dy;
            if let Some(pe) = crate::items::pile_at(app, x, y) {
                if let Ok(pile) = app.world.get::<&Pile>(pe) {
                    for (i, &(item, _)) in recipe.ingredients.iter().enumerate() {
                        for slot in &pile.slots {
                            if slot.item == item {
                                counts[i] += slot.count;
                            }
                        }
                    }
                }
            }
        }
    }
    counts
}

fn count_in_hand(hands: &Hands, item: ItemKind) -> u32 {
    let mut n = 0u32;
    if let Some((kind, c)) = hands.left {
        if kind == item { n += c; }
    }
    if let Some((kind, c)) = hands.right {
        if kind == item { n += c; }
    }
    n
}

// ── 材料消耗 ──

/// 消耗指定数量的材料。返回 true 表示全部消耗成功。
fn consume_ingredients(app: &mut App, recipe: &BuildRecipe, cx: i32, cy: i32, rng: &mut impl rand::Rng) -> bool {
    let Some(actor) = app.actor() else { return false; };
    let mut remaining: Vec<u32> = recipe.ingredients.iter().map(|&(_, n)| n).collect();

    // 1. 双手
    if let Ok(mut hands) = app.world.get::<&mut Hands>(actor) {
        for (i, &(item, _)) in recipe.ingredients.iter().enumerate() {
            let took = take_from_hand(&mut hands, item, remaining[i]);
            remaining[i] -= took;
        }
    }

    // 2. dist≤2 地面 Pile
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            if dx.abs() + dy.abs() > 2 { continue; }
            let x = cx + dx;
            let y = cy + dy;
            for (i, &(item, _)) in recipe.ingredients.iter().enumerate() {
                if remaining[i] == 0 { continue; }
                let took = take_from_pile(app, x, y, item, remaining[i], rng);
                remaining[i] -= took;
            }
        }
    }

    remaining.iter().all(|&r| r == 0)
}

fn take_from_hand(hands: &mut Hands, item: ItemKind, needed: u32) -> u32 {
    if needed == 0 { return 0; }
    let mut took = 0u32;
    if let Some((kind, count)) = hands.right.as_mut() {
        if *kind == item {
            let n = (*count).min(needed - took);
            *count -= n;
            took += n;
            if *count == 0 { hands.right = None; }
        }
    }
    if took < needed {
        if let Some((kind, count)) = hands.left.as_mut() {
            if *kind == item {
                let n = (*count).min(needed - took);
                *count -= n;
                took += n;
                if *count == 0 { hands.left = None; }
            }
        }
    }
    took
}

fn take_from_pile(app: &mut App, x: i32, y: i32, item: ItemKind, needed: u32, _rng: &mut impl rand::Rng) -> u32 {
    let Some(pe) = crate::items::pile_at(app, x, y) else { return 0; };
    let mut took = 0u32;
    {
        let Ok(mut pile) = app.world.get::<&mut Pile>(pe) else { return 0; };
        let mut i = 0usize;
        while i < pile.slots.len() && took < needed {
            if pile.slots[i].item == item {
                let n = pile.slots[i].count.min(needed - took);
                pile.slots[i].count -= n;
                took += n;
                if pile.slots[i].count == 0 {
                    pile.slots.swap_remove(i);
                    continue;
                }
            }
            i += 1;
        }
    } // drop pile ref
    // 检查是否需要清理空 Pile
    let empty = app.world.get::<&Pile>(pe).map(|p| p.is_empty()).unwrap_or(false);
    if empty {
        let _ = app.world.despawn(pe);
        app.mark_spatial_dirty();
    }
    took
}

// ── 屋顶支撑检测 ──

pub fn can_build_roof(app: &App, x: i32, y: i32) -> bool {
    if app.map.has_roof(x, y) { return false; }
    for (dx, dy) in &[(0, -1), (1, 0), (0, 1), (-1, 0)] {
        let (nx, ny) = (x + dx, y + dy);
        if has_wall_at(app, nx, ny) { return true; }
        if app.map.has_roof(nx, ny) { return true; }
    }
    false
}

// ── 可行性检查 ──

pub fn can_build(app: &App, recipe_index: usize, target_x: i32, target_y: i32) -> BuildCheck {
    let Some(recipe) = BUILD_RECIPES.get(recipe_index) else {
        return BuildCheck::Invalid;
    };
    let (cx, cy) = app.actor_pos();

    // 光照
    let light = LightLevel::from_u8(app.tile_light(cx, cy));
    if !light.can_craft() || (light as u8) < recipe.min_light {
        return BuildCheck::TooDark;
    }
    if recipe.requires_fire && !app.has_fire_adjacent(cx, cy) {
        return BuildCheck::NeedFire;
    }

    // 目标格合法性
    if recipe.self_target {
        // 屋顶：建在脚下
        if target_x != cx || target_y != cy { return BuildCheck::Invalid; }
        if !can_build_roof(app, cx, cy) {
            return BuildCheck::NoSupport;
        }
        // 浅水不可建屋顶
        let terrain = app.map.terrain(cx, cy);
        if terrain == TerrainKind::ShallowWater || terrain == TerrainKind::Water {
            return BuildCheck::NoBuildTerrain;
        }
    } else {
        // 邻格建造
        let dx = (target_x - cx).abs();
        let dy = (target_y - cy).abs();
        if dx > 1 || dy > 1 || (dx == 0 && dy == 0) {
            return BuildCheck::Invalid;
        }
        if !app.map.is_walkable(target_x, target_y) {
            return BuildCheck::Blocked;
        }
        // 浅水/深水不可建造
        let terrain = app.map.terrain(target_x, target_y);
        if terrain == TerrainKind::ShallowWater || terrain == TerrainKind::Water {
            return BuildCheck::NoBuildTerrain;
        }
        if app.is_blocked(target_x, target_y) {
            return BuildCheck::Blocked;
        }
    }

    // 材料
    let counts = count_available(app, recipe, cx, cy);
    for (i, &(_, needed)) in recipe.ingredients.iter().enumerate() {
        if counts.get(i).copied().unwrap_or(0) < needed {
            return BuildCheck::MissingMaterials;
        }
    }

    BuildCheck::Ok
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildCheck {
    Ok,
    TooDark,
    NeedFire,
    Blocked,
    NoSupport,
    MissingMaterials,
    Invalid,
    NoBuildTerrain, // 浅水不可建造
}

impl BuildCheck {
    pub fn hint(self) -> &'static str {
        match self {
            BuildCheck::Ok => "",
            BuildCheck::TooDark => "太暗了",
            BuildCheck::NeedFire => "需要篝火",
            BuildCheck::Blocked => "格位被占",
            BuildCheck::NoSupport => "没有支撑",
            BuildCheck::MissingMaterials => "材料不足",
            BuildCheck::Invalid => "无效",
            BuildCheck::NoBuildTerrain => "水里建不了",
        }
    }
}

// ── 建造入口：B 键 → 选配方 ──

pub fn open_build_menu(app: &mut App) {
    if recipe_count() == 0 { return; }
    app.build_menu = Some(BuildMenuState::Browsing { cursor: 0, scroll: 0 });
}

pub fn close_build_menu(app: &mut App) {
    app.build_menu = None;
}

/// 方向选定后 → 开始建造
pub fn start_build(app: &mut App, target_x: i32, target_y: i32, rng: &mut impl rand::Rng) -> bool {
    let recipe_index = match &app.build_menu {
        Some(BuildMenuState::Browsing { cursor, .. }) | Some(BuildMenuState::PickingDir { cursor, .. }) => *cursor,
        _ => return false,
    };
    let recipe = &BUILD_RECIPES[recipe_index];
    let (cx, cy) = app.actor_pos();
    let tx = if recipe.self_target { cx } else { target_x };
    let ty = if recipe.self_target { cy } else { target_y };

    let check = can_build(app, recipe_index, tx, ty);
    if check != BuildCheck::Ok {
        app.push_log(format!("无法建造：{}。", check.hint()));
        return false;
    }

    // ── 地形建造代价修正 ──
    let build_terrain = if recipe.self_target {
        app.map.terrain(cx, cy)
    } else {
        app.map.terrain(tx, ty)
    };
    let (extra_wood, terrain_progress_mult) = terrain_build_cost(build_terrain);

    // 浅沼额外消耗 Wood×2
    if extra_wood > 0
        && !consume_extra_wood(app, extra_wood, cx, cy, rng)
    {
        app.push_log("浅沼建造需要额外木头打地基。".into());
        return false;
    }

    // 消耗材料
    if !consume_ingredients(app, recipe, cx, cy, rng) {
        app.push_log("材料不足。".into());
        return false;
    }

    // 自动加速
    app.pre_build_speed = Some(app.speed);
    if !matches!(app.speed, crate::app::Speed::Turbo) {
        app.speed = crate::app::Speed::Fast;
    }

    // 开始进度——保持弹窗，切换到 Building 状态
    let actor = match app.actor() { Some(a) => a, None => return false };
    let adjusted_total = (recipe.base_progress as f32 * terrain_progress_mult).round() as u32;
    let _ = app.world.insert_one(actor, Building {
        recipe_index,
        progress: 0,
        total: adjusted_total,
    });
    app.build_target = Some((tx, ty, recipe.result));
    app.build_menu = Some(BuildMenuState::Building { recipe_index });
    true
}

/// 地形建造代价：(额外木头, 进度倍率)
fn terrain_build_cost(terrain: TerrainKind) -> (u32, f32) {
    match terrain {
        TerrainKind::ShallowMarsh => (2, 1.3), // 浅沼：+Wood×2，耗时×1.3
        TerrainKind::Sand => (0, 1.1),         // 沙地：耗时×1.1
        TerrainKind::Hill => (0, 1.4),         // 丘陵：耗时×1.4
        _ => (0, 1.0),
    }
}

/// 消耗额外木头（浅沼地基）
fn consume_extra_wood(app: &mut App, needed: u32, cx: i32, cy: i32, rng: &mut impl rand::Rng) -> bool {
    // 先检查是否够
    let mut available = 0u32;
    if let Some(actor) = app.actor() {
        if let Ok(hands) = app.world.get::<&Hands>(actor) {
            available += count_in_hand(&hands, ItemKind::Wood);
        }
    }
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            if dx.abs() + dy.abs() > 2 { continue; }
            if let Some(pe) = crate::items::pile_at(app, cx + dx, cy + dy) {
                if let Ok(pile) = app.world.get::<&Pile>(pe) {
                    for slot in &pile.slots {
                        if slot.item == ItemKind::Wood {
                            available += slot.count;
                        }
                    }
                }
            }
        }
    }
    if available < needed {
        return false;
    }
    // 消耗
    let mut remaining = needed;
    if let Some(actor) = app.actor() {
        if let Ok(mut hands) = app.world.get::<&mut Hands>(actor) {
            remaining -= take_from_hand(&mut hands, ItemKind::Wood, remaining);
        }
    }
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            if dx.abs() + dy.abs() > 2 { continue; }
            if remaining == 0 { break; }
            remaining -= take_from_pile(app, cx + dx, cy + dy, ItemKind::Wood, remaining, rng);
        }
    }
    remaining == 0
}

// ── 进度推进（每 tick 调用） ──

pub fn update_building(app: &mut App) {
    let build_target = match app.build_target {
        Some(bt) => bt,
        None => return,
    };
    let (tx, ty, target) = build_target;

    let Some(actor) = app.actor() else { return };

    // 先读 total，立刻 drop 引用
    let total = match app.world.get::<&Building>(actor) {
        Ok(b) => b.total,
        Err(_) => { app.build_target = None; return; }
    };

    // 光照检查
    let (cx, cy) = app.actor_pos();
    let light = LightLevel::from_u8(app.tile_light(cx, cy));
    if !light.can_craft() { return; }

    // 推进进度
    {
        let Ok(mut bld) = app.world.get::<&mut Building>(actor) else {
            app.build_target = None; return;
        };
        bld.progress += 1;
        if bld.progress < total { return; }
    } // drop bld

    // 完成！
    app.build_target = None;
    app.build_menu = None;
    app.speed = app.pre_build_speed.take().unwrap_or(app.speed);
    let _ = app.world.remove_one::<Building>(actor);

    match target {
        BuildTarget::WoodWall => {
            app.world.spawn((
                Position { x: tx, y: ty },
                WoodWall,
                Wall,
                BlocksMovement,
                BlocksVision,
                Harvestable { hp: 500.0, max_hp: 500.0, yield_item: ItemKind::Wood, yield_hp_step: 100.0 },
            ));
            app.push_log("木墙建好了。".into());
        }
        BuildTarget::StoneWall => {
            app.world.spawn((
                Position { x: tx, y: ty },
                StoneWall,
                Wall,
                BlocksMovement,
                BlocksVision,
                Harvestable { hp: 1200.0, max_hp: 1200.0, yield_item: ItemKind::BigStone, yield_hp_step: 100.0 },
            ));
            app.push_log("石墙垒好了——这玩意能挡一整个冬天。".into());
        }
        BuildTarget::WoodDoor => {
            app.world.spawn((
                Position { x: tx, y: ty },
                Door { open: false },
                Wall,
                BlocksMovement,
                BlocksVision,
                Harvestable { hp: 300.0, max_hp: 300.0, yield_item: ItemKind::Wood, yield_hp_step: 100.0 },
            ));
            app.push_log("门装好了。进出自如，只是铰链已经开始呻吟了。".into());
        }
        BuildTarget::WoodRoof => {
            app.map.set_roof(tx, ty, true);
            app.push_log("屋顶铺好了。抬头看不见天——这就是室内了。".into());
        }
        BuildTarget::StickTrap => {
            app.world.spawn((
                Position { x: tx, y: ty },
                StickTrap { builder: actor },
            ));
            app.push_log("尖刺陷阱布置好了。记住你把它放哪了——踩上去可不分敌我。".into());
        }
    }
    app.rebuild_spatial_index();
    app.mark_spatial_dirty();
}

// ── 陷阱触发 ──

/// 检测 (x,y) 是否有 StickTrap，有就触发伤害
pub fn trigger_trap_at(app: &mut App, x: i32, y: i32, walker: hecs::Entity) {
    let trap_entity = match find_trap_at(app, x, y) {
        Some(e) => e,
        None => return,
    };

    let damage = rand::thread_rng().gen_range(20..=40);
    let _ = app.world.despawn(trap_entity);

    crate::systems::combat::apply_damage(app, walker, damage as f32, (x, y));

    let name = app.entity_label(walker);
    app.push_log(format!(
        "{}踩中了尖刺陷阱！木刺扎穿了脚掌，伤害 {}。陷阱毁了。",
        name, damage
    ));

    app.mark_spatial_dirty();
}

fn find_trap_at(app: &App, x: i32, y: i32) -> Option<hecs::Entity> {
    if let Some(v) = app.spatial.by_tile.get(&(x, y)) {
        for &e in v {
            if app.world.get::<&StickTrap>(e).is_ok() {
                return Some(e);
            }
        }
    }
    None
}
