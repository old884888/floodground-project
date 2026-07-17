//! 屠宰系统：DF 式尸体剥皮

use rand::Rng;

use crate::app::App;
use crate::components::*;
use crate::items::drop_item_near;

/// 屠宰 (x,y) 处的尸体。核心逻辑，接受显式 actor（玩家/殖民者皆可）。
/// 返回 true = 成功剥完。
pub fn butcher_corpse(app: &mut App, actor: hecs::Entity, x: i32, y: i32, rng: &mut impl Rng) -> bool {
    let corpse_e = app.world.query::<(&Position, &Corpse)>().iter()
        .find(|(_, (p, _))| p.x == x && p.y == y).map(|(e, _)| e);

    let Some(e) = corpse_e else { return false };

    // 工具检测：骨刀>石刀>木刀
    let has_tool = |app: &App, item: ItemKind| {
        app.world.get::<&Hands>(actor)
            .map(|h| h.left.is_some_and(|(k,_)| k == item) || h.right.is_some_and(|(k,_)| k == item))
            .unwrap_or(false)
    };
    let speed = if has_tool(app, ItemKind::BoneKnife) { 1.8 }
        else if has_tool(app, ItemKind::StoneKnife) { 1.0 }
        else if has_tool(app, ItemKind::WoodKnife) { 0.6 }
        else {
            app.push_log("需要一把刀才能屠宰——石刀、木刀或骨刀。".into());
            return false;
        };

    let (ax, ay) = match app.world.get::<&Position>(actor) {
        Ok(p) => (p.x, p.y), Err(_) => return false,
    };
    if (x - ax).abs() + (y - ay).abs() > 1 {
        app.push_log("太远了，够不着尸体。".into());
        return false;
    }

    // 基础 300 tick 按工具速度——v1 直接完成（简化）
    let total = (300.0 / speed) as u32;
    let _ = total;

    let corpse = app.world.get::<&Corpse>(e).map(|c| *c).unwrap();
    let _ = app.world.despawn(e);
    app.mark_spatial_dirty();

    let (meat, bone, leather, fat) = match corpse.animal {
        AnimalKind::Deer => (rng.gen_range(3..=5), rng.gen_range(1..=2), 1, 0),
        AnimalKind::Boar => (rng.gen_range(5..=8), rng.gen_range(2..=3), 1, rng.gen_range(1..=2)),
        AnimalKind::Rabbit => (rng.gen_range(1..=2), 0, 1, 0),
    };
    let leather_kind = if corpse.animal == AnimalKind::Deer || corpse.animal == AnimalKind::Boar {
        ItemKind::Leather
    } else {
        ItemKind::RoughLeather
    };

    for _ in 0..meat { drop_item_near(app, (x, y), (ax, ay), ItemKind::RawMeat, 1); }
    for _ in 0..bone { drop_item_near(app, (x, y), (ax, ay), ItemKind::Bone, 1); }
    for _ in 0..leather { drop_item_near(app, (x, y), (ax, ay), leather_kind, 1); }
    for _ in 0..fat { drop_item_near(app, (x, y), (ax, ay), ItemKind::Fat, 1); }

    let name = match corpse.animal {
        AnimalKind::Deer => "鹿", AnimalKind::Boar => "野猪", AnimalKind::Rabbit => "兔子",
    };
    let label = app.entity_label(actor);
    app.push_log(format!("{}剥完了{}——尸骨散了一地。", label, name));
    // 磨损工具
    crate::systems::harvest::apply_wear(app, actor, rng);
    true
}

/// 玩家入口：用当前 actor 屠宰。成功后步进一帧。
pub fn try_butcher(app: &mut App, x: i32, y: i32, rng: &mut impl Rng) -> bool {
    let Some(actor) = app.actor() else {
        app.push_log("没有能动的人。".into());
        return false;
    };
    let ok = butcher_corpse(app, actor, x, y, rng);
    if ok { app.force_step = true; }
    ok
}
