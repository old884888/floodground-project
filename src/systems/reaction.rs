use rand::Rng;

use crate::app::App;
use crate::components::{Colonist, Mood, Name, Position};
use crate::events::GameEvent;
use crate::world::has_line_of_sight;

const WITNESS_RANGE: i32 = 6;
const REPUTATION_WARN: i32 = -10;

pub fn react(app: &mut App, event: &GameEvent, rng: &mut impl Rng) {
    match event {
        GameEvent::TortureCommitted { victim: _, pos, .. } => {
            apply_torture_backlash(app, *pos, rng);
        }
        GameEvent::CaptiveBroke { .. } => {
            app.reputation -= 10;
            app.events.push(GameEvent::ReputationChanged {
                delta: -10,
                reason: "俘虏崩溃".into(),
            });
            if app.reputation <= REPUTATION_WARN {
                app.push_log("[警告] 你的声誉在下坠。这片土地开始记住你的名字。".into());
            }
        }
        _ => {}
    }
}

fn apply_torture_backlash(app: &mut App, torture_pos: (i32, i32), rng: &mut impl Rng) {
    app.reputation -= 5;
    app.events.push(GameEvent::ReputationChanged {
        delta: -5,
        reason: "刑讯".into(),
    });

    let mut witnesses: Vec<(hecs::Entity, String, f32)> = Vec::new();

    for (entity, (pos, _mood, name)) in app
        .world
        .query::<(&Position, &Mood, &Name)>()
        .with::<&Colonist>()
        .iter()
    {
        let dist = (pos.x - torture_pos.0).abs() + (pos.y - torture_pos.1).abs();
        if dist <= WITNESS_RANGE
            && has_line_of_sight(&app.map, (pos.x, pos.y), torture_pos)
        {
            let delta = rng.gen_range(8.0..15.0);
            witnesses.push((entity, name.0.clone(), delta));
        }
    }

    for (entity, _name, delta) in witnesses {
        let mut low = false;
        {
            if let Ok(mut mood) = app.world.get::<&mut Mood>(entity) {
                mood.value -= delta;
                mood.clamp();
                low = mood.value < 30.0;
            }
        }

        if app.can_see_entity(entity) {
            let label = app.entity_label(entity);
            app.push_log(format!(
                "{}看见了那一幕，胃里翻江倒海。（心情 -{:.0}）",
                label, delta
            ));

            if low {
                app.push_log(format!("{}情绪不稳——再逼下去会出事。", label));
            }
        }

        app.events.push(GameEvent::MoodChanged {
            entity,
            delta: -delta,
            reason: "目击刑讯".into(),
        });
    }

    if app.reputation <= REPUTATION_WARN {
        app.push_log("[警告] 你的声誉在下坠。".into());
    }
}
