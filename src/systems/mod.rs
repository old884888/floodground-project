pub mod ai;
pub mod building;
pub mod butcher;
pub mod combat;
pub mod crafting;
pub mod dark;
pub mod debug_commands;
pub mod eat;
pub mod examine;
pub mod farming;
pub mod harvest;
pub mod input;
pub mod interact;
pub mod movement;
pub mod nature;
pub mod needs;
pub mod prey_ai;
pub mod prey_ecology;
pub mod reaction;
pub mod terrain_gen;
pub mod weather;

use crate::app::{App, Speed};
use crate::events::GameEvent;
use crate::narrative;

pub fn run_tick(app: &mut App, rng: &mut impl rand::Rng) {
    let prev_progress = app.day_progress();
    app.tick += 1;
    app.events.push(GameEvent::Tick(app.tick));

    // 空间索引：只在上一 tick 有变动时重建（静止时跳过，省 50-75% 开销）
    app.rebuild_spatial_index();

    if app.tick.is_multiple_of(app.ticks_per_day) {
        app.day += 1;
        app.events.push(GameEvent::DayElapsed(app.day));
    }

    let progress = app.day_progress();
    if prev_progress < 0.70 && progress >= 0.70 {
        app.push_log("天黑了，你几乎看不见自己的手。篝火成了唯一的岸。".into());
    }
    if prev_progress < 0.10 && progress >= 0.10 && prev_progress > 0.05 {
        // 跨过黎明→白天（避免 tick0 误报）
    }
    if prev_progress >= 0.95 && progress < 0.10 {
        app.push_log("黎明撕开夜色。世界又一次露出牙齿。".into());
    }

    // 玩家死了：依然走时间/视野/bush/AI 衰减（殖民者要吃饭），但禁止
    // 玩家行动（移动/攻击/吃饭/刑讯）以及战斗系统找玩家。
    let player_alive = !app.player_dead;

    if player_alive {
        movement::apply_pending_move(app, rng);
    } else {
        // 清掉悬挂的玩家操作请求，避免复活后立即触发
        app.pending_move = None;
        app.pending_chop = false;
        app.pending_mine = false;
        app.pending_break_wall = false;
        app.pending_grab = false;
        app.pending_drop = false;
        app.pending_eat = false;
        app.pending_torture = false;
        app.action_lock = None;
    }

    // 移动冷却递减（所有带 MoveCooldown 的实体）
    movement::tick_cooldowns(app);

    if player_alive && app.pending_grab {
        app.pending_grab = false;
        interact::try_grab(app, rng);
    }
    if app.pending_drop {
        // 死后掉东西仍允许
        app.pending_drop = false;
        interact::try_drop(app);
    }
    if player_alive && app.pending_chop {
        app.pending_chop = false;
        harvest::try_chop(app, rng);
    }
    if player_alive && app.pending_mine {
        app.pending_mine = false;
        harvest::try_mine(app, rng);
    }

    if player_alive && app.pending_torture {
        app.pending_torture = false;
        dark::try_torture(app, rng);
    }

    if player_alive && app.pending_eat {
        app.pending_eat = false;
        eat::try_eat(app);
    }

    if player_alive && app.pending_break_wall {
        app.pending_break_wall = false;
        harvest::try_break_wall(app, rng);
    }

    weather::update_weather(app, rng);
    needs::update_needs(app);
    crafting::update_crafting(app, rng);
    building::update_building(app);
    ai::update_ai(app, rng);
    if player_alive {
        combat::update_combat(app, rng);
    }
    nature::update_bushes(app);
    terrain_gen::update_wolf_dens(app, rng);
    prey_ecology::update_ecology(app, rng);
    prey_ai::update_prey_ai(app, rng);
    farming::update_crops(app);
    combat::tick_visual_effects(app);
    tick_puddles(app, rng);

    check_action_lock(app);

    let drained = app.events.drain();
    for event in &drained {
        reaction::react(app, event, rng);
    }
    for event in drained {
        if let Some(line) = narrative::format_event(app, &event) {
            app.push_log(line);
        }
    }
}

pub fn ticks_this_frame(speed: Speed) -> u32 {
    match speed {
        Speed::Paused | Speed::Step => 0,
        Speed::Normal => 1,
        Speed::Fast => 10,
        Speed::Turbo => 15,
    }
}

use crate::app::ExamineAction;
use crate::components::{Boulder, Bush, BushState, Captive, Dead, Position, Tree, Wall};

/// Puddle 蒸发：每 tick 5%，天气晴/阴加速到 10%
fn tick_puddles(app: &mut App, rng: &mut impl rand::Rng) {
    use crate::app::Weather;
    let chance = if matches!(app.weather, Weather::Clear | Weather::Overcast) { 0.10 } else { 0.05 };
    let mut to_kill: Vec<hecs::Entity> = Vec::new();
    for (e, (pos, _)) in app.world.query::<(&crate::components::Position, &crate::components::Puddle)>().iter() {
        if rng.gen_bool(chance) {
            to_kill.push(e);
            app.events.push(crate::events::GameEvent::PuddleEvaporated { x: pos.x, y: pos.y });
        }
    }
    for e in to_kill {
        let _ = app.world.despawn(e);
        app.mark_spatial_dirty();
    }
}

fn check_action_lock(app: &mut App) {
    let Some((tx, ty, action, _, _)) = app.action_lock else {
        return;
    };
    let alive = match action {
        ExamineAction::Chop => app
            .world
            .query::<(&Position, &Tree)>()
            .iter()
            .any(|(_, (p, _))| p.x == tx && p.y == ty),
        ExamineAction::Mine => app
            .world
            .query::<(&Position, &Boulder)>()
            .iter()
            .any(|(_, (p, _))| p.x == tx && p.y == ty),
        ExamineAction::Harvest => app
            .world
            .query::<(&Position, &Bush)>()
            .iter()
            .any(|(_, (p, b))| p.x == tx && p.y == ty && b.state == BushState::Fruiting),
        ExamineAction::Torture => app
            .world
            .query::<(&Position, &Captive)>()
            .iter()
            .any(|(e, (p, _))| p.x == tx && p.y == ty && app.world.get::<&Dead>(e).is_err()),
        ExamineAction::BreakWall => app
            .world
            .query::<(&Position, &Wall)>()
            .iter()
            .any(|(_, (p, _))| p.x == tx && p.y == ty),
        _ => false,
    };
    if !alive {
        app.action_lock = None;
        app.push_log(format!(
            "目标没了——{}自动取消。",
            examine::action_label(action)
        ));
    }
}
