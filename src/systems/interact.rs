use crate::app::App;
use crate::components::{Clothing, Hands, Position};
use crate::events::GameEvent;
use crate::items::{drop_item_near, has_pile, place_item};

pub fn try_grab(app: &mut App, rng: &mut impl rand::Rng) {
    let Some(actor) = app.actor() else {
        app.push_log("没有能动的人。".into());
        return;
    };
    let (px, py) = {
        let Ok(pos) = app.world.get::<&Position>(actor) else {
            return;
        };
        (pos.x, pos.y)
    };

    // 脚下有 Pile → 打开检查菜单
    if has_pile(app, px, py) {
        crate::systems::examine::open_underfoot(app);
        return;
    }

    // 脚下没有 → 看面前那格
    let (fx, fy) = (px + app.facing.0, py + app.facing.1);
    if has_pile(app, fx, fy) {
        crate::systems::examine::open_at(app, fx, fy);
        return;
    }

    // 都没有 → 面前灌木可采
    if crate::systems::harvest::try_harvest_bush(app, rng) {
        return;
    }

    app.push_log("这里没什么可拿的。".into());
}

pub fn try_drop(app: &mut App) {
    let Some(actor) = app.actor() else {
        return;
    };
    let (px, py) = {
        let Ok(pos) = app.world.get::<&Position>(actor) else {
            return;
        };
        (pos.x, pos.y)
    };

    let empty = app
        .world
        .get::<&Hands>(actor)
        .map(|h| h.is_empty())
        .unwrap_or(true);
    if empty {
        app.push_log("手上什么都没有。".into());
        return;
    }

    let item = {
        let Ok(mut hands) = app.world.get::<&mut Hands>(actor) else {
            return;
        };
        hands.drop_one()
    };
    let Some(item) = item else {
        app.push_log("手上什么都没有。".into());
        return;
    };

    if drop_item_near(app, (px, py), (px, py), item, 1) {
        app.events.push(GameEvent::ItemDropped { item });
        let who = app.entity_label(actor);
        app.push_log(format!("{}把{}丢了出去。", who, item.label()));
    }
}

/// 穿/脱衣物（o 键）：
/// - 已穿 → 脱下丢到脚下
/// - 手上有可穿戴皮 → 穿上（从手移到 Clothing）
/// - 都没有 → 提示
pub fn try_wear(app: &mut App) {
    use crate::components::clothing_warmth;
    let Some(actor) = app.actor() else {
        app.push_log("没有能动的人。".into());
        return;
    };
    let (px, py) = {
        let Ok(pos) = app.world.get::<&Position>(actor) else { return };
        (pos.x, pos.y)
    };

    // 已穿 → 脱下
    if app.world.get::<&Clothing>(actor).is_ok() {
        let old = {
            let c = app.world.get::<&Clothing>(actor).map(|c| *c).unwrap();
            c
        };
        let _ = app.world.remove_one::<Clothing>(actor);
        place_item(app, px, py, old.item, 1);
        let who = app.entity_label(actor);
        app.push_log(format!("{}脱下了{}。", who, old.item.label()));
        return;
    }

    // 手上有可穿戴 → 穿上
    let wearable = {
        let Ok(hands) = app.world.get::<&Hands>(actor) else {
            app.push_log("你连手都没有。".into());
            return;
        };
        let left = hands.left.and_then(|(k, _)| clothing_warmth(k).map(|w| (k, w, false)));
        let right = hands.right.and_then(|(k, _)| clothing_warmth(k).map(|w| (k, w, true)));
        right.or(left)
    };

    let Some((item, warmth, from_right)) = wearable else {
        app.push_log("手上没有能穿的东西——皮或者粗皮才行。".into());
        return;
    };

    // 从手移除 1 件
    {
        let Ok(mut hands) = app.world.get::<&mut Hands>(actor) else { return };
        let slot = if from_right { &mut hands.right } else { &mut hands.left };
        if let Some((_kind, count)) = slot.as_mut() {
            *count -= 1;
            if *count == 0 { *slot = None; }
        }
    }
    let _ = app.world.insert_one(actor, Clothing { item, warmth });
    let who = app.entity_label(actor);
    app.push_log(format!("{}披上了{}——暖和多了。", who, item.label()));
}
