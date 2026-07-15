use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{
    App, CraftMenuState, DebugPopup, ExamineAction, ExamineMenu, GameMode, Speed,
    SIDE_TAB_COUNT,
};
use crate::components::{Building, CraftingState};
use crate::systems::{crafting, examine, interact};
use super::debug_commands::{debug_teleport, handle_debug_popup_key};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    // 主菜单：只响应菜单键
    if app.screen == crate::app::Screen::MainMenu {
        handle_menu_key(app, key);
        return;
    }
    // 加载画面：只响应 Q 退出
    if app.screen == crate::app::Screen::Loading {
        if key.code == KeyCode::Char('q') || key.code == KeyCode::Char('Q') {
            app.should_quit = true;
        }
        return;
    }

    if app.quit_menu {
        match key.code {
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if let Err(e) = crate::save::save_game(app) {
                    app.push_log(format!("存档失败: {}", e));
                } else {
                    app.push_log("已存档。".into());
                }
                app.should_quit = true;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => { app.should_quit = true; }
            KeyCode::Esc => { app.quit_menu = false; }
            _ => {}
        }
        return;
    }

    if app.debug_popup.is_some() {
        handle_debug_popup_key(app, key);
        return;
    }
    if app.craft_menu.is_some() {
        handle_craft_menu_key(app, key);
        return;
    }
    if app.build_menu.is_some() {
        handle_build_menu_key(app, key);
        return;
    }
    if app.focused_tile.is_some() {
        handle_observe_mode(app, key);
        return;
    }
    if app.examine_dir_prompt {
        handle_examine_dir_prompt_key(app, key);
        return;
    }
    if app.action_lock.is_some() {
        handle_action_lock_key(app, key);
        return;
    }
    if app.examine.is_some() {
        handle_examine_key(app, key);
        return;
    }
    if app.player_dead {
        handle_dead_key(app, key);
        return;
    }
    if app.game_mode == GameMode::Camp {
        handle_camp_key(app, key);
        return;
    }
    handle_adventure_key(app, key);
}

fn handle_dead_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => app.should_quit = true,
        KeyCode::Char('r') | KeyCode::Char('R') => {
            // 调试用：玩家死了也能 Q 退，R 让殖民者接班（保留选择）
            if let Some(next) = next_alive_colonist(app) {
                app.player = next;
                app.selected = Some(next);
                app.player_dead = false;
                app.last_actor_terrain = None; // 切角色：重置地形记忆
                app.push_log("……你以为自己死了，但身体还在动。殖民者接班控制。".into());
            } else {
                app.push_log("没有能接班的人。按 Q 退出。".into());
            }
        }
        _ => app.push_log("你已经死了。按 Q 退出，或 R 让殖民者接班。".into()),
    }
}

fn next_alive_colonist(app: &App) -> Option<hecs::Entity> {
    use crate::components::{Colonist, Dead};
    app.world.query::<&Colonist>().iter().map(|(e, _)| e).find(|&e| app.world.get::<&Dead>(e).is_err())
}

fn handle_camp_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Tab => app.toggle_game_mode(),
        KeyCode::Char('q') | KeyCode::Char('Q') => app.should_quit = true,
        _ => app.push_log("营地模式占位，按 Tab 回到冒险。".into()),
    }
}

fn handle_action_lock_key(app: &mut App, key: KeyEvent) {
    let Some((_tx, _ty, action, lx, ly)) = app.action_lock else {
        return;
    };

    let dir: Option<(i32, i32)> = match key.code {
        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') | KeyCode::Char('k') => Some((0, -1)),
        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('j') => Some((0, 1)),
        KeyCode::Left | KeyCode::Char('A') | KeyCode::Char('h') => Some((-1, 0)),
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => Some((1, 0)),
        KeyCode::Esc => {
            app.action_lock = None;
            app.push_log("你不再盯着那边了。".into());
            return;
        }
        _ => return,
    };

    let Some((dx, dy)) = dir else { return };

    if dx == lx && dy == ly {
        match action {
            ExamineAction::Chop => app.pending_chop = true,
            ExamineAction::Mine => app.pending_mine = true,
            ExamineAction::Harvest => app.pending_grab = true,
            ExamineAction::Torture => app.pending_torture = true,
            ExamineAction::BreakWall => app.pending_break_wall = true,
            _ => {} // OpenDoor/CloseDoor/Sleep handled immediately, not via lock
        }
        app.force_step = true;
    } else {
        app.action_lock = None;
        app.pending_move = Some((dx, dy));
        app.force_step = true;
    }
}

fn handle_adventure_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('r') | KeyCode::Char('R') => {
            app.craft_menu = Some(CraftMenuState::Browsing { cursor: 0, scroll: 0 });
            app.push_log("打开制作菜单。↑↓选择，Enter制作，Esc取消。".into());
        }
        KeyCode::Char('b') | KeyCode::Char('B') => {
            crate::systems::building::open_build_menu(app);
            app.push_log("打开建造菜单。↑↓选择，Enter确认，Esc取消。".into());
        }
        KeyCode::Char('q') | KeyCode::Char('Q') => app.should_quit = true,
        KeyCode::Char('e') => {
            app.examine_dir_prompt = true;
            app.push_log("想看哪个方向？请按方向键。Esc取消。".into());
        }
        KeyCode::Char('x') => {
            let (px, py) = app.actor_pos();
            app.focused_tile = Some((px, py));
            app.observe_scroll = 0;
            app.push_log("观察模式：方向键移动光标，[ ]滚动侧栏，X或Esc退出。".into());
        }
        KeyCode::Esc => {
            if app.focused_tile.is_some() {
                app.focused_tile = None;
                app.push_log("你收回了目光。".into());
            } else if app.examine_dir_prompt {
                app.examine_dir_prompt = false;
                app.push_log("算了，不看了。".into());
            }
        }
        KeyCode::Char('E') => {
            app.pending_eat = true;
            app.force_step = true;
        }
        KeyCode::Char('.') => app.force_step = true,
        KeyCode::Char('[') => {
            app.side_panel_tab = (app.side_panel_tab + SIDE_TAB_COUNT - 1) % SIDE_TAB_COUNT;
        }
        KeyCode::Char(']') => {
            app.side_panel_tab = (app.side_panel_tab + 1) % SIDE_TAB_COUNT;
        }
        KeyCode::F(6) => {
            if app.debug_popup.is_some() {
                app.debug_popup = None;
            } else {
                app.debug_popup = Some(DebugPopup {
                    cursor: 0,
                    sub: None,
                    sub_cursor: 0,
                });
            }
        }
        KeyCode::Char(' ') => match app.speed {
            Speed::Step | Speed::Paused => app.force_step = true,
            _ => app.speed = Speed::Paused,
        },
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.speed = match app.speed {
                Speed::Paused => Speed::Step,
                Speed::Step => Speed::Normal,
                Speed::Normal => Speed::Fast,
                Speed::Fast => Speed::Turbo,
                Speed::Turbo => Speed::Turbo,
            };
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            app.speed = match app.speed {
                Speed::Turbo => Speed::Fast,
                Speed::Fast => Speed::Normal,
                Speed::Normal => Speed::Step,
                Speed::Step => Speed::Paused,
                Speed::Paused => Speed::Paused,
            };
        }
        KeyCode::Char('p') | KeyCode::Char('P') => {
            app.speed = if app.speed == Speed::Paused {
                Speed::Normal
            } else {
                Speed::Paused
            };
        }
        KeyCode::Tab => app.toggle_game_mode(),
        KeyCode::Char(';') => app.cycle_character(),
        KeyCode::Char('1') => app.select_character_slot(1),
        KeyCode::Char('2') => app.select_character_slot(2),
        KeyCode::Char('3') => app.select_character_slot(3),
        KeyCode::Char('4') => app.select_character_slot(4),
        KeyCode::Char('t') | KeyCode::Char('T') => {
            app.pending_torture = true;
            app.force_step = true;
        }
        KeyCode::Char('g') | KeyCode::Char('G') => {
            app.pending_grab = true;
            app.force_step = true;
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            app.pending_drop = true;
            app.force_step = true;
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            app.pending_chop = true;
            app.force_step = true;
        }
        KeyCode::Char('m') | KeyCode::Char('M') => {
            app.pending_mine = true;
            app.force_step = true;
        }
        KeyCode::Left | KeyCode::Char('A') | KeyCode::Char('h') => {
            app.pending_move = Some((-1, 0));
            app.force_step = true;
            app.focused_tile = None;
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
            app.pending_move = Some((1, 0));
            app.force_step = true;
            app.focused_tile = None;
        }
        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') | KeyCode::Char('k') => {
            app.pending_move = Some((0, -1));
            app.force_step = true;
            app.focused_tile = None;
        }
        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('j') => {
            app.pending_move = Some((0, 1));
            app.force_step = true;
            app.focused_tile = None;
        }
        _ => {}
    }
}

fn handle_examine_key(app: &mut App, key: KeyEvent) {
    let menu = app.examine.as_ref().map(|s| s.menu.clone());

    match menu {
        Some(ExamineMenu::Pile) => match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let len = examine::pile_len(app);
                if len > 0 {
                    if let Some(state) = app.examine.as_mut() {
                        state.cursor = if state.cursor == 0 { len - 1 } else { state.cursor - 1 };
                        state.take_qty = 1; // 换物品重置数量
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = examine::pile_len(app);
                if len > 0 {
                    if let Some(state) = app.examine.as_mut() {
                        state.cursor = if state.cursor >= len - 1 { 0 } else { state.cursor + 1 };
                        state.take_qty = 1; // 换物品重置数量
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(state) = app.examine.as_mut() {
                    if state.take_qty > 1 {
                        state.take_qty -= 1;
                    }
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let (px, py, cur, qty) = if let Some(ref s) = app.examine {
                    (s.x, s.y, s.cursor, s.take_qty)
                } else {
                    return;
                };
                let max = crate::items::pile_at(app, px, py)
                    .and_then(|e| app.world.get::<&crate::components::Pile>(e).ok())
                    .and_then(|p| p.slots.get(cur).map(|s| s.count))
                    .unwrap_or(0);
                if let Some(state) = app.examine.as_mut() {
                    if qty < max {
                        state.take_qty = qty + 1;
                    }
                }
            }
            KeyCode::Char('g') | KeyCode::Char('G') | KeyCode::Enter => {
                examine::try_grab_from_pile(app);
            }
            KeyCode::Char('d') | KeyCode::Char('D') => interact::try_drop(app),
            KeyCode::Esc => examine::close(app),
            _ => {}
        },
        Some(ExamineMenu::Action(action)) => match key.code {
            KeyCode::Esc => examine::close(app),
            KeyCode::Enter | KeyCode::Char('1') | KeyCode::Char('y') | KeyCode::Char('Y') => {
                examine::action_to_lock(app, action);
            }
            _ => {}
        },
        _ => {
            if key.code == KeyCode::Esc {
                examine::close(app);
            }
        }
    }
}

// ── 调试弹窗键盘处理 ──

fn handle_examine_dir_prompt_key(app: &mut App, key: KeyEvent) {
    let dir: Option<(i32, i32)> = match key.code {
        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') | KeyCode::Char('k') => Some((0, -1)),
        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('j') => Some((0, 1)),
        KeyCode::Left | KeyCode::Char('A') | KeyCode::Char('h') => Some((-1, 0)),
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => Some((1, 0)),
        KeyCode::Esc => {
            app.examine_dir_prompt = false;
            app.push_log("算了，不看了。".into());
            return;
        }
        _ => return,
    };

    let Some((dx, dy)) = dir else { return };

    app.examine_dir_prompt = false;
    let (px, py) = app.actor_pos();
    examine::open_at(app, px + dx, py + dy);
}

fn handle_observe_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') | KeyCode::Char('k') => {
            move_observe_cursor(app, 0, -1);
        }
        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('j') => {
            move_observe_cursor(app, 0, 1);
        }
        KeyCode::Left | KeyCode::Char('A') | KeyCode::Char('h') => {
            move_observe_cursor(app, -1, 0);
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
            move_observe_cursor(app, 1, 0);
        }
        // 侧边栏滚动
        KeyCode::Char('[') => {
            app.observe_scroll = app.observe_scroll.saturating_sub(1);
        }
        KeyCode::Char(']') => {
            app.observe_scroll = app.observe_scroll.saturating_add(1);
        }
        KeyCode::Char('x') | KeyCode::Esc => {
            app.focused_tile = None;
            app.observe_scroll = 0;
            app.push_log("你收回了目光。".into());
        }
        KeyCode::Char('g') | KeyCode::Char('G') => {
            debug_teleport(app);
        }
        _ => {}
    }
}

fn move_observe_cursor(app: &mut App, dx: i32, dy: i32) {
    if let Some((x, y)) = app.focused_tile {
        let nx = x + dx;
        let ny = y + dy;
        if app.map.in_bounds(nx, ny) {
            app.focused_tile = Some((nx, ny));
            app.observe_scroll = 0; // 换格重置滚动
        }
    }
}

// ── 主菜单键盘处理 ──

fn handle_menu_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => {
            app.menu.cursor = if app.menu.cursor == 0 { 1 } else { app.menu.cursor - 1 };
        }
        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') => {
            app.menu.cursor = if app.menu.cursor >= 1 { 0 } else { app.menu.cursor + 1 };
        }
        KeyCode::Enter => match app.menu.cursor {
            0 => {
                app.screen = crate::app::Screen::Loading;
                app.loading_tick = 0;
            }
            1 => {
                app.should_quit = true;
            }
            _ => {}
        },
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            app.should_quit = true;
        }
        _ => {}
    }
}

fn handle_build_menu_key(app: &mut App, key: KeyEvent) {
    use crate::app::BuildMenuState;
    use crate::systems::building;

    let state = app.build_menu.clone();
    match state {
        Some(BuildMenuState::Building { .. }) if key.code == KeyCode::Esc => {
            // 建造中：只能 Esc 取消（材料不退）
            if let Some(actor) = app.actor() {
                let _ = app.world.remove_one::<Building>(actor);
            }
            app.speed = app.pre_build_speed.take().unwrap_or(app.speed);
            app.build_target = None;
            building::close_build_menu(app);
            app.push_log("建造取消了——材料已消耗，半点不剩。".into());
        }
        Some(BuildMenuState::Building { .. }) => {}
        Some(BuildMenuState::PickingDir { cursor: _, .. }) => {
            let (px, py) = app.actor_pos();
            match key.code {
                KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => {
                    let _ = building::start_build(app, px, py - 1, &mut rand::thread_rng());
                }
                KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') => {
                    let _ = building::start_build(app, px, py + 1, &mut rand::thread_rng());
                }
                KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('h') => {
                    let _ = building::start_build(app, px - 1, py, &mut rand::thread_rng());
                }
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
                    let _ = building::start_build(app, px + 1, py, &mut rand::thread_rng());
                }
                KeyCode::Esc => building::close_build_menu(app),
                _ => {}
            }
        }
        Some(BuildMenuState::Browsing { cursor: _, scroll: _ }) => {
            let count = building::recipe_count();
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(BuildMenuState::Browsing { cursor, scroll }) = app.build_menu.as_mut() {
                        *cursor = if *cursor == 0 { count.saturating_sub(1) } else { *cursor - 1 };
                        if *cursor < *scroll { *scroll = *cursor; }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(BuildMenuState::Browsing { cursor, scroll }) = app.build_menu.as_mut() {
                        *cursor = if *cursor >= count.saturating_sub(1) { 0 } else { *cursor + 1 };
                        let vis = 5usize; // 可见行数
                        if *cursor >= *scroll + vis { *scroll = (*cursor).saturating_sub(vis - 1); }
                    }
                }
                KeyCode::Enter => {
                    let recipe_idx = match &app.build_menu {
                        Some(BuildMenuState::Browsing { cursor, .. }) => *cursor,
                        _ => 0,
                    };
                    let recipe = &building::BUILD_RECIPES[recipe_idx];
                    if recipe.self_target {
                        let (px, py) = app.actor_pos();
                        let _ = building::start_build(app, px, py, &mut rand::thread_rng());
                    } else {
                        app.build_menu = Some(BuildMenuState::PickingDir { cursor: recipe_idx, scroll: 0 });
                    }
                }
                KeyCode::Esc => building::close_build_menu(app),
                _ => {}
            }
        }
        None => {}
    }
}

// ── 制作菜单键盘处理 ──

fn handle_craft_menu_key(app: &mut App, key: KeyEvent) {
    match &app.craft_menu {
        Some(CraftMenuState::Browsing { cursor, scroll }) => {
            let cursor = *cursor;
            let scroll = *scroll;
            let max = crafting::recipe_count().saturating_sub(1);
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    let new_cursor = if cursor == 0 { max } else { cursor - 1 };
                    let new_scroll = if new_cursor < scroll { new_cursor } else { scroll };
                    app.craft_menu = Some(CraftMenuState::Browsing { cursor: new_cursor, scroll: new_scroll });
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let new_cursor = if cursor >= max { 0 } else { cursor + 1 };
                    let vis = 5usize;
                    let new_scroll = if new_cursor >= scroll + vis { new_cursor.saturating_sub(vis - 1) } else { scroll };
                    app.craft_menu = Some(CraftMenuState::Browsing { cursor: new_cursor, scroll: new_scroll });
                }
                KeyCode::Enter => {
                    let check = crafting::can_craft(app, cursor);
                    if check == crafting::CraftCheck::Ok {
                        crafting::start_crafting(app, cursor);
                    } else {
                        app.push_log(format!("无法制作：{}。", check.hint()));
                    }
                }
                KeyCode::Esc => {
                    app.craft_menu = None;
                    app.push_log("你放下手中的活计。".into());
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    app.speed = match app.speed {
                        Speed::Paused => Speed::Step,
                        Speed::Step => Speed::Normal,
                        Speed::Normal => Speed::Fast,
                        Speed::Fast => Speed::Turbo,
                        Speed::Turbo => Speed::Turbo,
                    };
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    app.speed = match app.speed {
                        Speed::Turbo => Speed::Fast,
                        Speed::Fast => Speed::Normal,
                        Speed::Normal => Speed::Step,
                        Speed::Step => Speed::Paused,
                        Speed::Paused => Speed::Paused,
                    };
                }
                _ => {}
            }
        }
        Some(CraftMenuState::Crafting { .. }) => {
            match key.code {
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    app.speed = match app.speed {
                        Speed::Paused => Speed::Step,
                        Speed::Step => Speed::Normal,
                        Speed::Normal => Speed::Fast,
                        Speed::Fast => Speed::Turbo,
                        Speed::Turbo => Speed::Turbo,
                    };
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    app.speed = match app.speed {
                        Speed::Turbo => Speed::Fast,
                        Speed::Fast => Speed::Normal,
                        Speed::Normal => Speed::Step,
                        Speed::Step => Speed::Paused,
                        Speed::Paused => Speed::Paused,
                    };
                }
                KeyCode::Esc => {
                    // 检查是否允许中断
                    let can_interrupt = app
                        .actor()
                        .and_then(|e| app.world.get::<&CraftingState>(e).ok())
                        .and_then(|cs| {
                            crate::systems::crafting::RECIPES
                                .get(cs.recipe_index)
                                .map(|r| r.can_interrupt)
                        })
                        .unwrap_or(true);
                    if can_interrupt {
                        crafting::cancel_crafting(app);
                    } else {
                        app.push_log("这个制作不能中断——必须一口气做完。".into());
                    }
                }
                _ => {} // 制作中只能 Esc 取消
            }
        }
        None => {}
    }
}
