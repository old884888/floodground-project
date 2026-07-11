use crate::app::{App, ExamineAction, ExamineMenu, ExamineState};
use crate::components::{
    Bed, Boulder, BlocksMovement, BlocksVision, Bush, BushState, Captive, ContainerTag, Door, Hands, Pile, Position, Tree, Wall,
};
use crate::items::pile_at;

/// 打开对 (x,y) 的检查菜单（CDDA 式 e）
pub fn open_at(app: &mut App, x: i32, y: i32) {
    if !app.map.in_bounds(x, y) {
        app.push_log("那边什么都没有。".into());
        return;
    }

    let (px, py) = app.actor_pos();
    let dx = (x - px).abs();
    let dy = (y - py).abs();
    if dx > 1 || dy > 1 {
        app.push_log("太远了，够不着。".into());
        return;
    }

    let menu = detect_menu(app, x, y);
    if matches!(menu, ExamineMenu::Empty) {
        app.push_log(format!("({}, {}) 这里什么都没有。", x, y));
        return;
    }

    app.examine = Some(ExamineState {
        x,
        y,
        menu,
        cursor: 0,
    });
}

pub fn close(app: &mut App) {
    app.examine = None;
}

/// 面朝方向的目标格；默认朝南
#[allow(dead_code)]
pub fn facing_target(app: &App) -> (i32, i32) {
    let (px, py) = app.actor_pos();
    (px + app.facing.0, py + app.facing.1)
}

#[allow(dead_code)]
pub fn open_facing(app: &mut App) {
    open_at(app, facing_target(app).0, facing_target(app).1);
}

pub fn open_underfoot(app: &mut App) {
    let (px, py) = app.actor_pos();
    open_at(app, px, py);
}

fn detect_menu(app: &App, x: i32, y: i32) -> ExamineMenu {
    for (e, (pos, door)) in app.world.query::<(&Position, &Door)>().iter() {
        if pos.x == x && pos.y == y {
            let _ = e;
            return if door.open {
                ExamineMenu::Action(ExamineAction::CloseDoor)
            } else {
                ExamineMenu::Action(ExamineAction::OpenDoor)
            };
        }
    }
    for (e, pos) in app.world.query::<&Position>().with::<&Bed>().iter() {
        if pos.x == x && pos.y == y {
            let _ = e;
            return ExamineMenu::Action(ExamineAction::Sleep);
        }
    }
    for (e, pos) in app.world.query::<&Position>().with::<&ContainerTag>().iter() {
        if pos.x == x && pos.y == y {
            let _ = e;
            return ExamineMenu::Pile;
        }
    }
    for (e, pos) in app.world.query::<&Position>().with::<&Wall>().iter() {
        if pos.x == x && pos.y == y {
            let _ = e;
            return ExamineMenu::Action(ExamineAction::BreakWall);
        }
    }
    for (e, pos) in app.world.query::<&Position>().with::<&Tree>().iter() {
        if pos.x == x && pos.y == y {
            let _ = e;
            return ExamineMenu::Action(ExamineAction::Chop);
        }
    }
    for (e, pos) in app.world.query::<&Position>().with::<&Boulder>().iter() {
        if pos.x == x && pos.y == y {
            let _ = e;
            return ExamineMenu::Action(ExamineAction::Mine);
        }
    }
    for (e, (pos, bush)) in app.world.query::<(&Position, &Bush)>().iter() {
        if pos.x == x && pos.y == y && bush.state == BushState::Fruiting {
            let _ = e;
            return ExamineMenu::Action(ExamineAction::Harvest);
        }
    }
    for (e, pos) in app.world.query::<&Position>().with::<&Captive>().iter() {
        if pos.x == x && pos.y == y {
            let _ = e;
            return ExamineMenu::Action(ExamineAction::Torture);
        }
    }
    if pile_at(app, x, y).is_some() {
        return ExamineMenu::Pile;
    }
    ExamineMenu::Empty
}

pub fn pile_len(app: &App) -> usize {
    let Some(state) = &app.examine else {
        return 0;
    };
    if !matches!(state.menu, ExamineMenu::Pile) {
        return 0;
    }
    pile_at(app, state.x, state.y)
        .and_then(|e| app.world.get::<&Pile>(e).ok().map(|p| p.len()))
        .unwrap_or(0)
}

pub fn try_grab_from_pile(app: &mut App) {
    let Some(state) = app.examine.clone() else {
        return;
    };
    if !matches!(state.menu, ExamineMenu::Pile) {
        return;
    }

    let Some(actor) = app.actor() else {
        return;
    };
    let Some(entity) = pile_at(app, state.x, state.y) else {
        close(app);
        return;
    };

    let cursor = state.cursor;
    let can = app
        .world
        .get::<&Hands>(actor)
        .ok()
        .and_then(|h| {
            let pile = app.world.get::<&Pile>(entity).ok()?;
            let slot = pile.slots.get(cursor)?;
            Some(h.can_take(slot.item))
        })
        .unwrap_or(false);

    if !can {
        app.push_log("手满了，先丢掉点什么吧。".into());
        return;
    }

    let taken = {
        let Ok(mut pile) = app.world.get::<&mut Pile>(entity) else {
            return;
        };
        pile.take_slot(cursor, 1)
    };

    let Some((item, n)) = taken else {
        return;
    };

    {
        let Ok(mut hands) = app.world.get::<&mut Hands>(actor) else {
            return;
        };
        hands.take_n(item, n);
    }
    app.push_log(format!("你捡起了{}。", item.label()));

    let empty = {
        if let Ok(pile) = app.world.get::<&Pile>(entity) {
            let len = pile.len();
            if len > 0 {
                if let Some(ex) = app.examine.as_mut() {
                    ex.cursor = ex.cursor.min(len - 1);
                }
            }
            pile.is_empty()
        } else {
            true
        }
    };
    if empty {
        let _ = app.world.despawn(entity);
        app.mark_spatial_dirty();
        close(app);
    }
}

pub fn action_label(action: ExamineAction) -> &'static str {
    match action {
        ExamineAction::Chop => "砍伐",
        ExamineAction::Mine => "开采",
        ExamineAction::Harvest => "采摘莓果",
        ExamineAction::Torture => "刑讯",
        ExamineAction::OpenDoor => "开门",
        ExamineAction::CloseDoor => "关门",
        ExamineAction::Sleep => "睡觉",
        ExamineAction::BreakWall => "砸墙",
    }
}

/// 确认动作 → 锁定目标，之后该方向键连发
pub fn action_to_lock(app: &mut App, action: ExamineAction) {
    let Some(state) = &app.examine else {
        return;
    };
    let (tx, ty) = (state.x, state.y);
    let (px, py) = app.actor_pos();
    let dx = tx - px;
    let dy = ty - py;
    if dx == 0 && dy == 0
        && action != ExamineAction::Sleep {
            app.push_log("你没法对自己这么做。".into());
            close(app);
            return;
        }
    if dx.abs() > 1 || dy.abs() > 1 || dx.abs() + dy.abs() > 1 {
        if dx == 0 && dy == 0 && action == ExamineAction::Sleep {
            // OK — sleeping at own tile
        } else {
            app.push_log("太远了。".into());
            close(app);
            return;
        }
    }

    match action {
        ExamineAction::OpenDoor | ExamineAction::CloseDoor => {
            close(app);
            toggle_door(app, tx, ty);
        }
        ExamineAction::Sleep => {
            close(app);
            if let Some(actor) = app.actor() {
                if let Ok(mut energy) = app.world.get::<&mut crate::components::Energy>(actor) {
                    energy.value = (energy.value + 50.0).min(100.0);
                }
            }
            app.push_log("你躺下睡了一觉，精力恢复了不少。".into());
            app.force_step = true;
        }
        _ => {
            close(app);
            app.action_lock = Some((tx, ty, action, dx, dy));
            let dir = match (dx, dy) {
                (0, -1) => "北",
                (0, 1) => "南",
                (-1, 0) => "西",
                (1, 0) => "东",
                _ => "那边",
            };
            app.push_log(format!("你盯住了{}的{}，{}方向键连发。Esc退出。", dir, action_label(action), dir));
        }
    }
}

fn toggle_door(app: &mut App, x: i32, y: i32) {
    let door_entity = app.world.query::<&Position>().with::<&Door>().iter()
        .find(|(_, pos)| pos.x == x && pos.y == y)
        .map(|(e, _)| e);

    let Some(e) = door_entity else { return };

    let was_open = app.world.get::<&Door>(e).map(|d| d.open).unwrap_or(false);
    if was_open {
        let _ = app.world.insert_one(e, BlocksMovement);
        let _ = app.world.insert_one(e, BlocksVision);
        if let Ok(mut door) = app.world.get::<&mut Door>(e) { door.open = false; }
        app.push_log("门关上了。".into());
    } else {
        let _ = app.world.remove_one::<BlocksMovement>(e);
        let _ = app.world.remove_one::<BlocksVision>(e);
        if let Ok(mut door) = app.world.get::<&mut Door>(e) { door.open = true; }
        app.push_log("门打开了。".into());
    }
}
