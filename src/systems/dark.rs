use rand::Rng;

use crate::app::App;
use crate::components::{Captive, Dead, Position};
use crate::events::GameEvent;

pub fn try_torture(app: &mut App, rng: &mut impl Rng) {
    let Some(actor) = app.actor() else {
        app.push_log("没有能动的人。".into());
        return;
    };
    let player_pos = {
        let Ok(pos) = app.world.get::<&Position>(actor) else {
            return;
        };
        (pos.x, pos.y)
    };

    // 找相邻俘虏
    let mut target: Option<(hecs::Entity, f32)> = None;
    for (entity, (pos, captive, _name)) in app
        .world
        .query::<(&Position, &Captive, &crate::components::Name)>()
        .iter()
    {
        // 死人/已崩溃者不作为目标
        if app.world.get::<&Dead>(entity).is_ok() {
            continue;
        }
        let dist = (pos.x - player_pos.0).abs() + (pos.y - player_pos.1).abs();
        if dist == 1 {
            target = Some((entity, captive.will));
            break;
        }
    }

    let Some((victim, _will)) = target else {
        app.push_log("旁边没有俘虏可以下手。走近点再按 T。".into());
        return;
    };

    let damage = rng.gen_range(15.0..25.0);
    let mut broke = false;
    let mut died = false;

    if let Ok(mut captive) = app.world.get::<&mut Captive>(victim) {
        captive.will -= damage;
        captive.clamp();
        if captive.will <= 0.0 {
            broke = true;
        }
    }

    // 刑讯也伤一点血
    if let Ok(mut hp) = app.world.get::<&mut crate::components::Health>(victim) {
        hp.hp -= damage * 0.3;
        if hp.hp <= 0.0 {
            died = true;
        }
    }

    app.events.push(GameEvent::TortureCommitted {
        actor,
        victim,
        pos: player_pos,
        will_damage: damage,
    });

    let actor_name = app.entity_label(actor);
    let victim_name = app.entity_label(victim);

    app.push_log(format!(
        "{}对{}动了手。意志动摇了 {:.0}。",
        actor_name, victim_name, damage
    ));

    if broke {
        app.events.push(GameEvent::CaptiveBroke { entity: victim });
    }

    if died {
        app.kill(victim, "被刑讯致死");
    }
}
