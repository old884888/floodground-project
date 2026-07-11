//! 地面 Pile 放置 / 查询

use hecs::Entity;

use crate::app::App;
use crate::components::{ItemKind, Pile, Position};

/// 在 (x,y) 放入物品：已有 Pile 则合并，否则新建。满了返回 false。
pub fn place_item(app: &mut App, x: i32, y: i32, item: ItemKind, count: u32) -> bool {
    if count == 0 {
        return true;
    }
    if let Some(entity) = pile_at(app, x, y) {
        let Ok(mut pile) = app.world.get::<&mut Pile>(entity) else {
            return false;
        };
        return pile.add(item, count);
    }
    let mut pile = Pile::default();
    if !pile.add(item, count) {
        return false;
    }
    app.world.spawn((Position { x, y }, pile));
    app.mark_spatial_dirty();
    true
}

pub fn pile_at(app: &App, x: i32, y: i32) -> Option<Entity> {
    if let Some(v) = app.spatial.by_tile.get(&(x, y)) {
        for &e in v {
            if app.world.get::<&Pile>(e).is_ok() {
                return Some(e);
            }
        }
    }
    None
}

pub fn has_pile(app: &App, x: i32, y: i32) -> bool {
    pile_at(app, x, y).is_some()
}

/// 掉落：优先 behind，再 from 附近可走格。失败打日志。
pub fn drop_item_near(
    app: &mut App,
    from: (i32, i32),
    player: (i32, i32),
    item: ItemKind,
    count: u32,
) -> bool {
    let behind = (from.0 * 2 - player.0, from.1 * 2 - player.1);
    let mut candidates = vec![behind, from, player];
    for dy in -2..=2 {
        for dx in -2..=2 {
            if dx == 0 && dy == 0 {
                continue;
            }
            candidates.push((from.0 + dx, from.1 + dy));
        }
    }
    candidates.sort_by_key(|(x, y)| (x - from.0).abs() + (y - from.1).abs());
    candidates.dedup();

    for (x, y) in candidates {
        if !app.map.is_walkable(x, y) || app.is_blocked(x, y) {
            continue;
        }
        if place_item(app, x, y, item, count) {
            return true;
        }
    }
    app.push_log("地上已经他妈没办法再塞更多东西了。".into());
    false
}
