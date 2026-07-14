//! 制作系统：旧石器工具链
//!
//! R 键打开配方菜单 → 自动收集 dist≤2 材料 → 制作进度 tick → 产物进手

use crate::app::{App, CraftMenuState};
use crate::components::{CraftWip, CraftingState, Hands, ItemKind, LightLevel, Pile, Position};
use crate::items::{pile_at, place_item};

// ── 配方定义 ──

#[derive(Debug, Clone)]
pub struct Recipe {
    pub name: &'static str,
    pub ingredients: &'static [(ItemKind, u32)],
    pub result: (ItemKind, u32),
    pub base_progress: u32,
    pub requires_fire: bool,
    pub min_light: u8,
    pub craft_desc: &'static str,
    pub can_interrupt: bool,
    pub desc: &'static str,
    /// 需要手持的工具（None=不需要）
    pub requires_tool: Option<ItemKind>,
}

pub static RECIPES: &[Recipe] = &[
    // ── Plan 08 打制层 ──
    Recipe {
        name: "石片",
        ingredients: &[(ItemKind::SmallStone, 2)],
        result: (ItemKind::SmallFlake, 1),
        base_progress: 200,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在敲击石头打制石片...",
        can_interrupt: true,
        desc: "两块小石头互相敲出锋利的薄片。运气好能多出几片——旧石器时代的边角料工业。",
        requires_tool: None,
    },
    Recipe {
        name: "大石片",
        ingredients: &[(ItemKind::BigStone, 1)],
        result: (ItemKind::LargeFlake, 1),
        base_progress: 300,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在用石锤敲打大石头...",
        can_interrupt: true,
        desc: "大石头对准了用石锤砸——崩下来的大石片是旧石器所有工具的主料。必须手持石锤。",
        requires_tool: Some(ItemKind::StoneHammer),
    },
    // ── 木材加工 ──
    Recipe {
        name: "长木棍",
        ingredients: &[(ItemKind::Wood, 1)],
        result: (ItemKind::LongStick, 1),
        base_progress: 100,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在把木头削成直棍...",
        can_interrupt: true,
        desc: "一块木头削直了就是长木棍——这辈子最朴素的一次加工。需要手持石刀、木刀或骨刀。",
        requires_tool: Some(ItemKind::StoneKnife),
    },
    Recipe {
        name: "削尖长棍",
        ingredients: &[(ItemKind::LongStick, 1)],
        result: (ItemKind::WoodSpear, 1),
        base_progress: 150,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在削尖长木棍...",
        can_interrupt: true,
        desc: "把长木棍一头削尖了——比火烤矛差远了，但比拳头长。需要手持切割工具。",
        requires_tool: Some(ItemKind::StoneKnife),
    },
    // ── 绳 ──
    Recipe {
        name: "绳子",
        ingredients: &[(ItemKind::Vine, 3)],
        result: (ItemKind::Rope, 1),
        base_progress: 150,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在把藤条搓成绳子...",
        can_interrupt: true,
        desc: "三根藤条拧成一股——旧石器时代的工程奇迹。没绳子就别想绑东西。",
        requires_tool: None,
    },
    // ── 旧石器工具层（从大石片出）──
    Recipe {
        name: "石刀",
        ingredients: &[(ItemKind::LargeFlake, 1), (ItemKind::Vine, 1)],
        result: (ItemKind::StoneKnife, 1),
        base_progress: 300,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在修整石刀并缠握柄...",
        can_interrupt: true,
        desc: "大石片绑上藤条握柄。比碎石头拼的强多了——拿在手里终于像个工具了。",
        requires_tool: None,
    },
    Recipe {
        name: "石斧",
        ingredients: &[
            (ItemKind::LargeFlake, 1),
            (ItemKind::LongStick, 1),
            (ItemKind::Vine, 1),
        ],
        result: (ItemKind::StoneAxe, 1),
        base_progress: 800,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在装柄制作石斧...",
        can_interrupt: true,
        desc: "大石片绑在长木棍上——旧石器时代的瑞士军刀。砍树效率翻倍，砸人效率也翻倍。",
        requires_tool: None,
    },
    Recipe {
        name: "石锤",
        ingredients: &[
            (ItemKind::LargeFlake, 1),
            (ItemKind::LongStick, 1),
            (ItemKind::Vine, 1),
        ],
        result: (ItemKind::StoneHammer, 1),
        base_progress: 700,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在装柄制作石锤...",
        can_interrupt: true,
        desc: "厚石片绑在长木棍上——砸大石头出大石片，挖矿也比空手好使。旧石器版的镐子。",
        requires_tool: None,
    },
    Recipe {
        name: "石铲",
        ingredients: &[
            (ItemKind::LargeFlake, 1),
            (ItemKind::LongStick, 1),
            (ItemKind::Vine, 1),
        ],
        result: (ItemKind::StoneShovel, 1),
        base_progress: 650,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在磨制石铲并装柄...",
        can_interrupt: true,
        desc: "石片磨出薄刃再绑上长柄——挖地坑比木铲快一倍。旧石器时代的挖掘机。",
        requires_tool: None,
    },
    Recipe {
        name: "石钻",
        ingredients: &[
            (ItemKind::SmallFlake, 2),
            (ItemKind::Stick, 1),
            (ItemKind::Vine, 1),
        ],
        result: (ItemKind::StoneDrill, 1),
        base_progress: 500,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在制作石钻...",
        can_interrupt: true,
        desc: "尖锐的小石片装在手柄上——人类偷火的起点。搓起来手疼，但总比等闪电强。",
        requires_tool: None,
    },
    // ── 木质工具（便宜低效）──
    Recipe {
        name: "木刀",
        ingredients: &[(ItemKind::LongStick, 1), (ItemKind::SmallFlake, 1)],
        result: (ItemKind::WoodKnife, 1),
        base_progress: 200,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在把石片镶在木柄上...",
        can_interrupt: true,
        desc: "削尖的木片镶了个石片刃。切东西聊胜于无——别指望它做精细活。",
        requires_tool: None,
    },
    Recipe {
        name: "木斧",
        ingredients: &[
            (ItemKind::LongStick, 1),
            (ItemKind::SmallFlake, 1),
            (ItemKind::Vine, 1),
        ],
        result: (ItemKind::WoodAxe, 1),
        base_progress: 350,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在把石片绑在长木棍上...",
        can_interrupt: true,
        desc: "长木棍上绑了个石片——简陋得可怜。砍树比空手快，但也只比空手快。",
        requires_tool: None,
    },
    Recipe {
        name: "木铲",
        ingredients: &[(ItemKind::LongStick, 1)],
        result: (ItemKind::WoodShovel, 1),
        base_progress: 120,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在削尖长木棍做铲子...",
        can_interrupt: true,
        desc: "一根削尖的长木棍——挖地坑可以凑合着用。慢，但比手刨强。",
        requires_tool: Some(ItemKind::StoneKnife),
    },
    // ── 骨器 ──
    Recipe {
        name: "骨刀",
        ingredients: &[
            (ItemKind::Bone, 1),
            (ItemKind::SmallFlake, 1),
        ],
        result: (ItemKind::BoneKnife, 1),
        base_progress: 350,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在磨制骨刀...",
        can_interrupt: true,
        desc: "骨头磨薄了镶上石片刃——锋利得吓人。切东西飞快，但骨头脆，撑不了多久。需要手持石钻。",
        requires_tool: Some(ItemKind::StoneDrill),
    },
    Recipe {
        name: "骨针",
        ingredients: &[(ItemKind::Bone, 1)],
        result: (ItemKind::BoneNeedle, 1),
        base_progress: 300,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在钻磨骨针...",
        can_interrupt: true,
        desc: "骨头用石钻打孔再用石刀磨尖——等有皮子就能缝。需要手持石钻和石刀。",
        requires_tool: Some(ItemKind::StoneDrill),
    },
    // ── 遗留配方（已改名）──
    Recipe {
        name: "削尖棍",
        ingredients: &[(ItemKind::StoneKnife, 1), (ItemKind::Stick, 1)],
        result: (ItemKind::SharpStick, 1),
        base_progress: 200,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在用石刀削尖木棍...",
        can_interrupt: true,
        desc: "用石刀把木棍一端削尖。比徒手掰断强一百倍——虽然还是根棍子。",
        requires_tool: None,
    },
    Recipe {
        name: "火烤矛",
        ingredients: &[(ItemKind::SharpStick, 1)],
        result: (ItemKind::Spear, 1),
        base_progress: 400,
        requires_fire: true,
        min_light: 0,
        craft_desc: "正在火烤硬化矛尖...",
        can_interrupt: true,
        desc: "削尖棍在火上烤硬，矛尖乌黑发亮。刺进肉里比牙好用一万倍。需要篝火。",
        requires_tool: None,
    },
    Recipe {
        name: "火把",
        ingredients: &[(ItemKind::Stick, 1)],
        result: (ItemKind::Torch, 1),
        base_progress: 20,
        requires_fire: true,
        min_light: 0,
        craft_desc: "正在点燃木棍...",
        can_interrupt: true,
        desc: "木棍蘸上树脂点燃，照亮五格黑夜。烧不了多久——但够你摸黑找到下一根木棍。需要篝火。",
        requires_tool: None,
    },
    Recipe {
        name: "搓火",
        ingredients: &[(ItemKind::Leaves, 5)],
        result: (ItemKind::Torch, 1), // 产物不生成物品，finish_crafting 特殊处理
        base_progress: 400,
        requires_fire: false,
        min_light: 1,
        craft_desc: "正在疯狂搓动石钻...",
        can_interrupt: false,
        desc: "石钻对准引火物猛搓——35%成功率。失败浪费树叶，成功脚下出篝火。需要手持石钻。",
        requires_tool: Some(ItemKind::StoneDrill),
    },
];

pub fn recipe_count() -> usize {
    RECIPES.len()
}

// ── 半成品（CraftWip）工具 ──

/// 在 (x,y) 找一个匹配 recipe_index 的 CraftWip 实体
fn find_wip_at(app: &App, x: i32, y: i32, recipe_index: usize) -> Option<hecs::Entity> {
    if let Some(v) = app.spatial.by_tile.get(&(x, y)) {
        for &e in v {
            if let Ok(wip) = app.world.get::<&CraftWip>(e) {
                if wip.recipe_index == recipe_index {
                    return Some(e);
                }
            }
        }
    }
    None
}

/// 获取脚下任意 CraftWip 的信息（用于 UI 显示）
pub fn wip_info_at(app: &App, x: i32, y: i32) -> Option<(usize, u32)> {
    if let Some(v) = app.spatial.by_tile.get(&(x, y)) {
        for &e in v {
            if let Ok(wip) = app.world.get::<&CraftWip>(e) {
                return Some((wip.recipe_index, wip.progress));
            }
        }
    }
    None
}

/// 销毁指定 (x,y) 处匹配 recipe_index 的 CraftWip，返回其 progress
fn consume_wip_at(app: &mut App, x: i32, y: i32, recipe_index: usize) -> Option<u32> {
    let entity = find_wip_at(app, x, y, recipe_index)?;
    let progress = app.world.get::<&CraftWip>(entity).ok()?.progress;
    let _ = app.world.despawn(entity);
    app.mark_spatial_dirty();
    Some(progress)
}

// ── 制作可行性检查 ──

/// 检查配方是否可以制作：材料够 + 光照够 + （需火→邻格有火）
/// 如果脚下有该配方的半成品，跳过材料检查。
pub fn can_craft(app: &App, recipe_index: usize) -> CraftCheck {
    let Some(recipe) = RECIPES.get(recipe_index) else {
        return CraftCheck::Invalid;
    };
    let (cx, cy) = app.actor_pos();

    // 光照检查
    let light = LightLevel::from_u8(app.tile_light(cx, cy));
    if !light.can_craft() {
        return CraftCheck::TooDark;
    }
    if (light as u8) < recipe.min_light {
        return CraftCheck::TooDark;
    }

    // 篝火邻格检查
    if recipe.requires_fire && !app.has_fire_adjacent(cx, cy) {
        return CraftCheck::NeedFire;
    }

    // 工具需求检查
    if let Some(tool) = recipe.requires_tool {
        if !actor_has_item(app, tool) {
            return CraftCheck::NeedTool;
        }
    }

    // 脚下有半成品 → 免材料
    if find_wip_at(app, cx, cy, recipe_index).is_some() {
        return CraftCheck::Ok;
    }

    // 材料检查
    let enough = count_available_materials(app, recipe, cx, cy);
    for (i, &(_, needed)) in recipe.ingredients.iter().enumerate() {
        if enough.get(i).copied().unwrap_or(0) < needed {
            return CraftCheck::MissingMaterials;
        }
    }

    CraftCheck::Ok
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftCheck {
    Ok,
    TooDark,
    NeedFire,
    MissingMaterials,
    NeedTool,
    Invalid,
}

impl CraftCheck {
    pub fn hint(self) -> &'static str {
        match self {
            CraftCheck::Ok => "",
            CraftCheck::TooDark => "太暗了",
            CraftCheck::NeedFire => "需要篝火",
            CraftCheck::MissingMaterials => "材料不足",
            CraftCheck::NeedTool => "缺少工具",
            CraftCheck::Invalid => "无效配方",
        }
    }
}

/// 检查当前角色双手是否持有指定物品
pub fn actor_has_item(app: &App, item: ItemKind) -> bool {
    let Some(actor) = app.actor() else { return false; };
    if let Ok(hands) = app.world.get::<&Hands>(actor) {
        hands.left.is_some_and(|(k, _)| k == item)
            || hands.right.is_some_and(|(k, _)| k == item)
    } else {
        false
    }
}

// ── 材料收集 ──

/// 统计 dist≤2 范围内每种材料的可用量（不含双手已持有那部分）
fn count_available_materials(app: &App, recipe: &Recipe, cx: i32, cy: i32) -> Vec<u32> {
    let mut counts = vec![0u32; recipe.ingredients.len()];

    // 双手
    if let Some(actor) = app.actor() {
        if let Ok(hands) = app.world.get::<&Hands>(actor) {
            for (i, &(item, _)) in recipe.ingredients.iter().enumerate() {
                counts[i] += count_in_hand(&hands, item);
            }
        }
    }

    // 脚下 + 周围两格
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            if (dx.abs() + dy.abs()) > 2 {
                continue;
            }
            let x = cx + dx;
            let y = cy + dy;
            if let Some(pile_entity) = pile_at(app, x, y) {
                if let Ok(pile) = app.world.get::<&Pile>(pile_entity) {
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
    if let Some((k, c)) = hands.left {
        if k == item {
            n += c;
        }
    }
    if let Some((k, c)) = hands.right {
        if k == item {
            n += c;
        }
    }
    n
}

/// 扣除材料：双手优先 → 脚下 → dist=1 → dist=2
/// 调用前已确认材料够，不应失败。
fn consume_ingredients(app: &mut App, recipe: &Recipe, cx: i32, cy: i32) {
    let actor = app.actor(); // 暂存

    for &(item, mut needed) in recipe.ingredients {
        // 1. 双手（右手先）
        if let Some(actor) = actor {
            if let Ok(mut hands) = app.world.get::<&mut Hands>(actor) {
                needed -= take_from_hand_slot(&mut hands.right, item, needed);
                if needed > 0 {
                    needed -= take_from_hand_slot(&mut hands.left, item, needed);
                }
            }
        }
        if needed == 0 {
            continue;
        }

        // 2. 脚下
        needed = try_consume_from_pile_at(app, cx, cy, item, needed);
        if needed == 0 {
            continue;
        }

        // 3. dist=1
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if (dx.abs() + dy.abs()) > 1 {
                    continue;
                }
                needed = try_consume_from_pile_at(app, cx + dx, cy + dy, item, needed);
                if needed == 0 {
                    break;
                }
            }
            if needed == 0 {
                break;
            }
        }
        if needed == 0 {
            continue;
        }

        // 4. dist=2
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let mh = dx.abs() + dy.abs();
                if mh <= 1 || mh > 2 {
                    continue;
                }
                needed = try_consume_from_pile_at(app, cx + dx, cy + dy, item, needed);
                if needed == 0 {
                    break;
                }
            }
            if needed == 0 {
                break;
            }
        }

        if needed > 0 {
            // 理论上不该到这里（前面 count_available 已确认够）
            app.push_log(format!(
                "严重错误：材料{}扣除失败，还差{}个。",
                item.label(),
                needed
            ));
        }
    }
}

fn take_from_hand_slot(slot: &mut Option<(ItemKind, u32)>, item: ItemKind, needed: u32) -> u32 {
    if let Some((kind, count)) = slot.as_mut() {
        if *kind == item {
            let take = needed.min(*count);
            *count -= take;
            if *count == 0 {
                *slot = None;
            }
            return take;
        }
    }
    0
}

fn try_consume_from_pile_at(app: &mut App, x: i32, y: i32, item: ItemKind, needed: u32) -> u32 {
    let Some(entity) = pile_at(app, x, y) else {
        return needed;
    };
    let Ok(mut pile) = app.world.get::<&mut Pile>(entity) else {
        return needed;
    };

    let slot_idx = pile.slots.iter().position(|s| s.item == item);
    let Some(idx) = slot_idx else {
        return needed;
    };

    let available = pile.slots[idx].count;
    let take = needed.min(available);
    pile.slots[idx].count -= take;
    if pile.slots[idx].count == 0 {
        pile.slots.swap_remove(idx);
    }
    // Pile 彻底空 → 删实体
    if pile.is_empty() {
        drop(pile); // 释放 RefMut
        let _ = app.world.despawn(entity);
        app.mark_spatial_dirty();
    }
    needed - take
}

// ── 制作开始/完成 ──

/// 开始制作：扣除材料（或消耗半成品续进度），挂 CraftingState
pub fn start_crafting(app: &mut App, recipe_index: usize) {
    let Some(recipe) = RECIPES.get(recipe_index) else { return };
    let (cx, cy) = app.actor_pos();

    // 自动加速
    app.pre_build_speed = Some(app.speed);
    if !matches!(app.speed, crate::app::Speed::Turbo) {
        app.speed = crate::app::Speed::Fast;
    }

    // 检查是否有半成品可续
    let resume_progress = consume_wip_at(app, cx, cy, recipe_index);

    if let Some(prev) = resume_progress {
        // 续作：不消耗材料，从上次进度继续
        if let Some(actor) = app.actor() {
            let _ = app.world.insert_one(
                actor,
                CraftingState {
                    recipe_index,
                    progress: prev,
                },
            );
        }
        app.craft_menu = Some(CraftMenuState::Crafting { spinner_frame: 0 });
        app.push_log(format!(
            "你捡起半成品{}，继续制作。（{}/{}）",
            recipe.name,
            prev,
            recipe.base_progress
        ));
    } else {
        // 新制作：扣除材料
        consume_ingredients(app, recipe, cx, cy);

        if let Some(actor) = app.actor() {
            let _ = app.world.insert_one(
                actor,
                CraftingState {
                    recipe_index,
                    progress: 0,
                },
            );
        }
        app.craft_menu = Some(CraftMenuState::Crafting { spinner_frame: 0 });
        app.push_log(format!("你开始制作{}。", recipe.name));
    }
}

/// 取消制作：移除 CraftingState，生成半成品掉在制作者脚下
pub fn cancel_crafting(app: &mut App) {
    let entities: Vec<(hecs::Entity, usize, u32, (i32, i32))> = {
        let mut result = Vec::new();
        for (e, (cs, pos)) in app
            .world
            .query::<(&CraftingState, &Position)>()
            .iter()
        {
            result.push((e, cs.recipe_index, cs.progress, (pos.x, pos.y)));
        }
        result
    };

    for (entity, recipe_index, progress, (cx, cy)) in entities {
        let _ = app.world.remove_one::<CraftingState>(entity);

        if progress > 0 {
            let recipe_name = RECIPES
                .get(recipe_index)
                .map(|r| r.name)
                .unwrap_or("?");
            app.world.spawn((
                Position { x: cx, y: cy },
                CraftWip {
                    recipe_index,
                    progress,
                },
            ));
            app.mark_spatial_dirty();
            let total = RECIPES
                .get(recipe_index)
                .map(|r| r.base_progress)
                .unwrap_or(progress);
            app.push_log(format!(
                "你把做了一半的{}放在地上。（{}/{}）",
                recipe_name, progress, total
            ));
        } else {
            app.push_log("制作中断——还没开始，材料不退。阿弥陀佛。".into());
        }
    }
    app.craft_menu = None;
    app.speed = app.pre_build_speed.take().unwrap_or(app.speed);
}

/// 完成制作：移除指定实体的 CraftingState，产物进手
fn finish_crafting(app: &mut App, entity: hecs::Entity, recipe_index: usize, rng: &mut impl rand::Rng) {
    let Some(recipe) = RECIPES.get(recipe_index) else { return };
    let (result_item, mut result_count) = recipe.result;

    let _ = app.world.remove_one::<CraftingState>(entity);
    app.speed = app.pre_build_speed.take().unwrap_or(app.speed);

    // ── 打制副产 ──
    let (px, py) = app.actor_pos();
    if result_item == ItemKind::SmallFlake {
        // 1-3 片随机
        result_count = rng.gen_range(1..=3);
    }
    if result_item == ItemKind::LargeFlake {
        // 50% 额外一片
        if rng.gen_bool(0.5) { result_count += 1; }
        // 边角料：SmallFlake×2-3
        let bonus = rng.gen_range(2..=3);
        crate::items::place_item(app, px, py, ItemKind::SmallFlake, bonus);
    }

    // ── 石钻生火特殊处理 ──
    if recipe.name == "搓火" {
        if rng.gen_bool(0.35) {
            // 成功：脚下生成篝火
            app.world.spawn((
                Position { x: px, y: py },
                crate::components::Campfire,
                crate::components::BlocksMovement,
                crate::components::LightSource { radius: 15, brightness: 2 },
            ));
            app.mark_spatial_dirty();
            app.push_log("火星溅到引火物上——火着了！橘色的光重新回到了世界上。".into());
        } else {
            app.push_log("你拼命搓了半天——只搓出了一手汗和几缕烟。引火物废了。".into());
        }
        return;
    }

    // 产物进手
    let took = try_put_in_hands(app, result_item, result_count);

    if took > 0 {
        app.push_log(format!("你做好了{}。", result_item.label()));
    }
    if took < result_count {
        // 手满了，掉脚下
        let (cx, cy) = app.actor_pos();
        let remaining = result_count - took;
        if place_item(app, cx, cy, result_item, remaining) {
            app.push_log(format!(
                "手中已满，多余{}掉在地上。",
                result_item.label()
            ));
        } else {
            app.push_log(format!(
                "手中和地上都满了——{}滑落消失在泥土中。",
                result_item.label()
            ));
        }
    }

    app.craft_menu = None;
}

/// 尝试把物品放入 actor 双手，返回实际放入数量
pub fn try_put_in_hands(app: &mut App, item: ItemKind, count: u32) -> u32 {
    let Some(actor) = app.actor() else { return 0 };
    let Ok(mut hands) = app.world.get::<&mut Hands>(actor) else {
        return 0;
    };
    hands.take_n(item, count)
}

// ── 每 tick 更新 ──

const BASE_CRAFT_SPEED: u32 = 10;

pub fn update_crafting(app: &mut App, rng: &mut impl rand::Rng) {
    let actor = app.actor();
    let actor_pos = app.actor_pos();

    // 推进 spinner
    if let Some(CraftMenuState::Crafting { ref mut spinner_frame }) = app.craft_menu {
        *spinner_frame = spinner_frame.wrapping_add(1);
    }

    // 推进制作进度
    let recipe_index = match actor {
        Some(e) => app
            .world
            .get::<&CraftingState>(e)
            .ok()
            .map(|cs| cs.recipe_index),
        None => None,
    };
    let Some(recipe_index) = recipe_index else { return };
    let Some(recipe) = RECIPES.get(recipe_index) else { return };

    // 查光照 → 速度倍率
    let light_level = LightLevel::from_u8(app.tile_light(actor_pos.0, actor_pos.1));
    let speed_mult = light_level.craft_speed_multiplier();

    if speed_mult <= 0.0 {
        // 完全黑暗：制作暂停但不取消
        return;
    }

    let progress_add = (BASE_CRAFT_SPEED as f32 * speed_mult) as u32;

    let should_finish = if let Some(actor) = actor {
        if let Ok(mut cs) = app.world.get::<&mut CraftingState>(actor) {
            cs.progress += progress_add;
            cs.progress >= recipe.base_progress
        } else {
            false
        }
    } else {
        false
    };
    if should_finish {
        if let Some(actor) = actor {
            finish_crafting(app, actor, recipe_index, rng);
        }
    }
}
