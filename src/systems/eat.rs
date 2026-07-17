use crate::app::App;
use crate::components::{Hands, Hunger, ItemKind, Thirst};
use crate::data::FoodMap;

pub fn try_eat(app: &mut App) {
    let Some(actor) = app.actor() else {
        app.push_log("没有能动的人。".into());
        return;
    };

    let hand_kind = app
        .world
        .get::<&Hands>(actor)
        .ok()
        .and_then(|h| h.right.or(h.left))
        .map(|(k, _)| k);

    let Some(kind) = hand_kind else {
        app.push_log("手上没东西可吃。".into());
        return;
    };

    // 不饿了就别浪费食物
    let full = app.world.get::<&Hunger>(actor).ok().map(|h| h.value >= 95.0).unwrap_or(false);
    if full {
        app.push_log("你已经饱了，再吃就是浪费。".into());
        return;
    }

    let key = item_to_food_key(kind);
    let Some(food) = app.food_data.get(&key) else {
        app.push_log(format!("{}不能吃。", kind.label()));
        return;
    };

    // 扣手中物品
    {
        let Ok(mut hands) = app.world.get::<&mut Hands>(actor) else {
            return;
        };
        hands.drop_one();
    }

    if let Ok(mut hunger) = app.world.get::<&mut Hunger>(actor) {
        hunger.value = (hunger.value + food.hunger).min(100.0);
    }
    if let Ok(mut thirst) = app.world.get::<&mut Thirst>(actor) {
        thirst.value = (thirst.value + food.thirst).min(100.0);
    }

    let who = app.entity_label(actor);
    app.push_log(format!("{}吃掉了{}。", who, food.name));
}

fn item_to_food_key(kind: ItemKind) -> String {
    // 直接用 item 的 key 去 food.ron 里查；没有就是不能吃
    kind.key().to_string()
}

pub fn _food_map(app: &App) -> &FoodMap {
    &app.food_data
}
