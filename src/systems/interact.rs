use crate::app::App;
use crate::components::{Hands, Position};
use crate::events::GameEvent;
use crate::items::{drop_item_near, has_pile};

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
