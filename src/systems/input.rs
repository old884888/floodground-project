use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{
    App, CraftMenuState, DebugPopup, DebugSubKind, ExamineAction, ExamineMenu, GameMode, Speed,
    DEBUG_ITEM_COUNT, DEBUG_SUB_CREATURES, DEBUG_SUB_SETTLEMENTS, DEBUG_SUB_TERRAIN, DEBUG_SUB_TOOLS,
    DEBUG_SUB_TIME, DEBUG_SUB_WEATHER, DEBUG_TERRAIN_ITEMS, DEBUG_TIME_ITEMS, DEBUG_WEATHER_ITEMS,
    SIDE_TAB_COUNT, SETTLEMENT_SIZE_ITEMS,
};
use crate::components::{Building, Bush, BushState, CraftingState, Hunger, ItemKind, Position, Thirst, Tree};
use crate::items::place_item;
use crate::systems::{crafting, examine, interact};

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

fn handle_debug_popup_key(app: &mut App, key: KeyEvent) {
    let has_sub = app.debug_popup.as_ref().is_some_and(|p| p.sub.is_some());

    if has_sub {
        let (sub_kind, sub_cursor) = match &app.debug_popup {
            Some(p) => (p.sub.clone(), p.sub_cursor),
            _ => return,
        };
        let Some(sub_kind) = sub_kind else { return };

        let max_items = match sub_kind {
            DebugSubKind::Tool => crate::app::TOOL_ITEMS.len().saturating_sub(1),
            DebugSubKind::Creature => crate::app::SPAWN_ITEMS.len().saturating_sub(1),
            DebugSubKind::Settlement => SETTLEMENT_SIZE_ITEMS.len().saturating_sub(1),
            DebugSubKind::TimePeriod => DEBUG_TIME_ITEMS.len().saturating_sub(1),
            DebugSubKind::WeatherKind => DEBUG_WEATHER_ITEMS.len().saturating_sub(1),
            DebugSubKind::TerrainItem => DEBUG_TERRAIN_ITEMS.len().saturating_sub(1),
        };

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut p) = app.debug_popup {
                    p.sub_cursor = if p.sub_cursor == 0 { max_items } else { p.sub_cursor - 1 };
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut p) = app.debug_popup {
                    p.sub_cursor = if p.sub_cursor >= max_items { 0 } else { p.sub_cursor + 1 };
                }
            }
            KeyCode::Enter => {
                match sub_kind {
                    DebugSubKind::Tool => debug_spawn_tool(app, sub_cursor),
                    DebugSubKind::Creature => debug_spawn_execute(app, sub_cursor),
                    DebugSubKind::Settlement => {
                        if let Some(&(_, size)) = SETTLEMENT_SIZE_ITEMS.get(sub_cursor) {
                            app.spawn_settlement(size, &mut rand::thread_rng());
                        }
                    }
                    DebugSubKind::TimePeriod => debug_set_time(app, sub_cursor),
                    DebugSubKind::WeatherKind => debug_set_weather(app, sub_cursor),
                    DebugSubKind::TerrainItem => debug_spawn_terrain_item(app, sub_cursor),
                }
                app.debug_popup = None;
            }
            KeyCode::Esc => {
                if let Some(ref mut p) = app.debug_popup {
                    p.sub = None;
                }
            }
            _ => {}
        }
    } else {
        let cursor = app.debug_popup.as_ref().map(|p| p.cursor);

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut p) = app.debug_popup {
                    p.cursor = if p.cursor == 0 { DEBUG_ITEM_COUNT - 1 } else { p.cursor - 1 };
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut p) = app.debug_popup {
                    p.cursor = if p.cursor >= DEBUG_ITEM_COUNT - 1 { 0 } else { p.cursor + 1 };
                }
            }
            KeyCode::Enter => {
                if let Some(cur) = cursor {
                    if cur == DEBUG_SUB_TOOLS {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::Tool);
                            p.sub_cursor = 0;
                        }
                    } else if cur == DEBUG_SUB_CREATURES {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::Creature);
                            p.sub_cursor = 0;
                        }
                    } else if cur == DEBUG_SUB_SETTLEMENTS {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::Settlement);
                            p.sub_cursor = 0;
                        }
                    } else if cur == DEBUG_SUB_TIME {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::TimePeriod);
                            p.sub_cursor = 0;
                        }
                    } else if cur == DEBUG_SUB_WEATHER {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::WeatherKind);
                            p.sub_cursor = 0;
                        }
                    } else if cur == DEBUG_SUB_TERRAIN {
                        if let Some(ref mut p) = app.debug_popup {
                            p.sub = Some(DebugSubKind::TerrainItem);
                            p.sub_cursor = 0;
                        }
                    } else {
                        debug_execute(app, cur);
                        app.debug_popup = None;
                    }
                }
            }
            KeyCode::Esc | KeyCode::F(6) => {
                app.debug_popup = None;
            }
            _ => {}
        }
    }
}

fn debug_execute(app: &mut App, idx: usize) {
    match idx {
        0 => {
            let (px, py) = app.actor_pos();
            place_item(app, px, py, ItemKind::Berry, 5);
            app.push_log("（调试）脚下刷了 5 颗莓果。".into());
        }
        1 => {
            let (px, py) = app.actor_pos();
            place_item(app, px, py, ItemKind::Wood, 10);
            app.push_log("（调试）脚下刷了 10 根木头。".into());
        }
        2 => {
            let (px, py) = app.actor_pos();
            place_item(app, px, py, ItemKind::SmallStone, 5);
            app.push_log("（调试）脚下刷了 5 个小石头。".into());
        }
        3 => {
            let (px, py) = app.actor_pos();
            place_item(app, px, py, ItemKind::Stick, 5);
            app.push_log("（调试）脚下刷了 5 根木棍。".into());
        }
        4 => {
            let (px, py) = app.actor_pos();
            place_item(app, px, py, ItemKind::BigStone, 2);
            app.push_log("（调试）脚下刷了 2 个大石头。".into());
        }
        5 => {
            if let Some(actor) = app.actor() {
                if let Ok(mut h) = app.world.get::<&mut Hunger>(actor) {
                    h.value = 100.0;
                }
                if let Ok(mut t) = app.world.get::<&mut Thirst>(actor) {
                    t.value = 100.0;
                }
            }
            app.push_log("（调试）饥渴全满。".into());
        }
        6 => {
            let (px, py) = app.actor_pos();
            let tx = px + app.facing.0;
            let ty = py + app.facing.1;
            if app.map.is_walkable(tx, ty) && !app.is_blocked(tx, ty) {
                app.world.spawn((
                    Position { x: tx, y: ty },
                    Tree,
                    crate::components::BlocksMovement,
                    crate::components::BlocksVision,
                    crate::components::Harvestable {
                        hp: 1000.0,
                        max_hp: 1000.0,
                        yield_item: ItemKind::Wood,
                        yield_hp_step: 100.0,
                    },
                ));
                app.mark_spatial_dirty();
                app.push_log("（调试）面前长出一棵树。".into());
            } else {
                app.push_log("（调试）面前没空地。".into());
            }
        }
        7 => {
            let (px, py) = app.actor_pos();
            let tx = px + app.facing.0;
            let ty = py + app.facing.1;
            if app.map.is_walkable(tx, ty) && !app.is_blocked(tx, ty) {
                app.world.spawn((
                    Position { x: tx, y: ty },
                    Bush {
                        state: BushState::Fruiting,
                        growth_timer: 0,
                        yield_item: ItemKind::Berry,
                    },
                ));
                app.mark_spatial_dirty();
                app.push_log("（调试）面前长出一丛结果的莓果。".into());
            } else {
                app.push_log("（调试）面前没空地。".into());
            }
        }
        8..=10 | 12..=13 => {} // 子菜单，由 Enter 处理
        11 => {
            app.tick += 1200;
            app.push_log(format!(
                "（调试）时间 +2 小时。当前 {:02}:{:02}。",
                app.clock_hm().0,
                app.clock_hm().1,
            ));
        }
        _ => {}
    }
}

fn debug_spawn_execute(app: &mut App, idx: usize) {
    let (px, py) = app.actor_pos();
    let tx = px + app.facing.0;
    let ty = py + app.facing.1;
    if app.is_blocked(tx, ty) {
        app.push_log("（调试）面前没空地。".into());
        return;
    }

    match idx {
        0 => {
            app.world.spawn((
                Position { x: tx, y: ty },
                crate::components::Name("狼".into()),
                crate::components::Hostile,
                crate::components::Health {
                    hp: 50.0,
                    max_hp: 50.0,
                },
                crate::components::Wet { value: 0.0 },
                crate::components::MoveCooldown { ticks: 0 },
            ));
            app.push_log("（调试）生成一只狼。".into());
        }
        1 => {
            app.world.spawn((
                Position { x: tx, y: ty },
                crate::components::Name("新殖民者".into()),
                crate::components::Colonist,
                crate::components::Health {
                    hp: 100.0,
                    max_hp: 100.0,
                },
                Hunger { value: 80.0 },
                Thirst { value: 80.0 },
                crate::components::Energy { value: 80.0 },
                crate::components::Mood { value: 60.0 },
                crate::components::AiState {
                    current: crate::components::Act::Idle,
                },
                crate::components::TraitTag("冷静".into()),
                crate::components::Wet { value: 0.0 },
                crate::components::MoveCooldown { ticks: 0 },
            ));
            app.push_log("（调试）生成一个殖民者。".into());
        }
        2 => {
            app.world.spawn((
                Position { x: tx, y: ty },
                crate::components::Name("新俘虏".into()),
                crate::components::Captive { will: 80.0 },
                crate::components::Health {
                    hp: 70.0,
                    max_hp: 100.0,
                },
                Hunger { value: 50.0 },
                Thirst { value: 50.0 },
                crate::components::Energy { value: 50.0 },
                crate::components::Mood { value: 20.0 },
                crate::components::Wet { value: 0.0 },
                crate::components::MoveCooldown { ticks: 0 },
            ));
            app.push_log("（调试）生成一个俘虏。".into());
        }
        _ => {}
    }
    app.mark_spatial_dirty();
}

fn debug_spawn_tool(app: &mut App, idx: usize) {
    let (px, py) = app.actor_pos();
    let (item, label): (ItemKind, &str) = match idx {
        0 => (ItemKind::StoneKnife, "石刀"),
        1 => (ItemKind::SharpStick, "削尖棍"),
        2 => (ItemKind::Spear, "矛"),
        3 => (ItemKind::StoneAxe, "石斧"),
        4 => (ItemKind::Torch, "火把"),
        _ => return,
    };
    place_item(app, px, py, item, 1);
    app.push_log(format!("（调试）脚下刷了一把{}。", label));
}

/// 直接跳到指定时段（二级菜单选择）
fn debug_set_time(app: &mut App, idx: usize) {
    // 黎明=5%, 白天=35%, 黄昏=65%, 夜晚=85%
    let target = match idx {
        0 => 0.05, // 黎明
        1 => 0.35, // 白天
        2 => 0.65, // 黄昏
        _ => 0.85, // 夜晚
    };
    jump_day_progress(app, target);
    app.push_log(format!(
        "（调试）时间跳到{} {:02}:{:02}。",
        app.period_label(),
        app.clock_hm().0,
        app.clock_hm().1,
    ));
}

/// 直接设置天气（二级菜单选择）
fn debug_set_weather(app: &mut App, idx: usize) {
    use crate::app::Weather;
    let weather = match idx {
        0 => Weather::Clear,
        1 => Weather::Overcast,
        2 => Weather::Drizzle,
        3 => Weather::Rain,
        4 => Weather::Heavy,
        _ => Weather::Thunder,
    };
    let old = app.weather;
    app.weather = weather;
    app.weather_timer = 500; // 给个够长的持续时间
    app.weather_mood_tracker.clear();
    app.events.push(crate::events::GameEvent::WeatherChanged { from: old, to: weather });
    app.push_log(format!("（调试）天气设为{}。", weather.label()));
}

fn debug_spawn_terrain_item(app: &mut App, idx: usize) {
    let (px, py) = app.actor_pos();
    match idx {
        0 => {
            // 脚下刷草药
            place_item(app, px, py, ItemKind::Herb, 3);
            app.push_log("（调试）脚下刷了 3 株草药。".into());
        }
        1 => {
            // 脚下刷黏土
            place_item(app, px, py, ItemKind::Clay, 3);
            app.push_log("（调试）脚下刷了 3 团黏土。".into());
        }
        2 => {
            // 脚下刷金属矿
            place_item(app, px, py, ItemKind::MetalOre, 3);
            app.push_log("（调试）脚下刷了 3 块金属矿。".into());
        }
        3 => {
            // 脚下刷毒蘑菇
            place_item(app, px, py, ItemKind::PoisonMush, 3);
            app.push_log("（调试）脚下刷了 3 朵毒蘑菇。".into());
        }
        4 => {
            // 面前放狼巢穴
            let tx = px + app.facing.0;
            let ty = py + app.facing.1;
            if app.map.is_walkable(tx, ty) && !app.is_blocked(tx, ty) {
                app.world.spawn((
                    Position { x: tx, y: ty },
                    crate::components::WolfDen,
                ));
                app.mark_spatial_dirty();
                app.push_log("（调试）面前放了一个狼巢穴。".into());
            } else {
                app.push_log("（调试）面前没空地放巢穴。".into());
            }
        }
        _ => {}
    }
}

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

fn jump_day_progress(app: &mut App, target_progress: f32) {
    let tpd = app.ticks_per_day.max(1);
    let target = (tpd as f32 * target_progress) as u64;
    let cur = app.tick % tpd;
    let add = if target > cur {
        target - cur
    } else {
        tpd - cur + target
    };
    app.tick += add;
}

// ── 建造菜单键盘处理 ──

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
