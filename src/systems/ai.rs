use rand::Rng;

use crate::app::App;
use crate::components::{Act, AiState, Colonist, Energy, Hunger, Name, Position, Thirst};
use crate::events::GameEvent;

pub fn update_ai(app: &mut App, rng: &mut impl Rng) {
    let mut actions: Vec<(hecs::Entity, Act, String)> = Vec::new();
    let mut moves: Vec<(hecs::Entity, i32, i32)> = Vec::new();

    let mut decisions: Vec<(hecs::Entity, Act)> = Vec::new();
    for (entity, (hunger, thirst, energy, _ai, name)) in app
        .world
        .query::<(&Hunger, &Thirst, &Energy, &AiState, &Name)>()
        .with::<&Colonist>()
        .iter()
    {
        let next = decide(hunger.value, thirst.value, energy.value);
        decisions.push((entity, next));
        let _ = name;
    }

    for (entity, next) in decisions {
        let name = app
            .world
            .get::<&Name>(entity)
            .map(|n| n.0.clone())
            .unwrap_or_else(|_| "?".into());

        if let Ok(mut ai) = app.world.get::<&mut AiState>(entity) {
            ai.current = next;
        }

        match next {
            Act::Eating => {
                if let Ok(mut hunger) = app.world.get::<&mut Hunger>(entity) {
                    hunger.value = 80.0;
                }
                if let Ok(mut thirst) = app.world.get::<&mut Thirst>(entity) {
                    thirst.value = (thirst.value + 5.0).min(100.0);
                }
                actions.push((entity, Act::Eating, name));
            }
            Act::Sleeping => {
                if let Ok(mut energy) = app.world.get::<&mut Energy>(entity) {
                    energy.value = (energy.value + 8.0).min(100.0);
                }
                actions.push((entity, Act::Sleeping, name));
            }
            Act::Idle => {
                let (px, py) = match app.world.get::<&Position>(entity) {
                    Ok(pos) => (pos.x, pos.y),
                    Err(_) => continue,
                };
                let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1), (0, 0)];
                let (dx, dy) = dirs[rng.gen_range(0..dirs.len())];
                let nx = px + dx;
                let ny = py + dy;
                if (dx != 0 || dy != 0)
                    && app.map.is_walkable(nx, ny)
                    && !app.is_blocked(nx, ny)
                    && app.occupied(nx, ny).map(|e| {
                        app.world.get::<&Name>(e).is_err()
                    }).unwrap_or(true)
                {
                    moves.push((entity, nx, ny));
                }
            }
        }
    }

    for (entity, nx, ny) in moves {
        if let Ok(mut pos) = app.world.get::<&mut Position>(entity) {
            pos.x = nx;
            pos.y = ny;
        }
    }

    for (entity, act, _name) in actions {
        match act {
            Act::Eating => app.events.push(GameEvent::Ate { entity }),
            Act::Sleeping => app.events.push(GameEvent::Slept { entity }),
            Act::Idle => {}
        }
    }
}

fn decide(hunger: f32, thirst: f32, energy: f32) -> Act {
    if energy <= 0.0 {
        return Act::Sleeping;
    }
    if thirst < 15.0 || hunger < 20.0 {
        return Act::Eating;
    }
    if energy < 25.0 {
        return Act::Sleeping;
    }
    if thirst < 40.0 || hunger < 45.0 {
        return Act::Eating;
    }
    // Eating/Sleeping or anything else → both resolve to Idle
    Act::Idle
}
