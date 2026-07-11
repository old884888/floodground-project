use crate::app::App;
use crate::components::{Dead, Energy, Health, Hunger, Mood, Thirst, Wet};

/// 按「每天」速率：饥饿 -30/天，口渴 -35/天，精力 -40/天
pub fn update_needs(app: &mut App) {
    let tpd = app.ticks_per_day.max(1) as f32;
    let hunger_per = 30.0 / tpd;
    let thirst_per = 35.0 / tpd;
    let energy_per = 40.0 / tpd;
    let hunger_mood = 8.0 / tpd;
    let thirst_mood = 8.0 / tpd;
    let energy_mood = 8.0 / tpd;
    let starve_hp = 20.0 / tpd;
    let dehydrate_hp = 25.0 / tpd;

    // 先收集死人集合和潮湿值，避免在 query_mut 持有借用时再调 world.get
    let dead: std::collections::HashSet<hecs::Entity> = app
        .world
        .query::<&Dead>()
        .iter()
        .map(|(e, _)| e)
        .collect();

    let wet_map: std::collections::HashMap<hecs::Entity, f32> = app
        .world
        .query::<&Wet>()
        .iter()
        .map(|(e, w)| (e, w.value))
        .collect();

    let weather_mood = app.weather.mood_penalty();

    let mut starving: Vec<hecs::Entity> = Vec::new();
    let mut dehydrating: Vec<hecs::Entity> = Vec::new();
    let mut to_kill: Vec<(hecs::Entity, &'static str)> = Vec::new();

    for (entity, (hunger, thirst, energy, mood, health, _name)) in app
        .world
        .query_mut::<(
            &mut Hunger,
            &mut Thirst,
            &mut Energy,
            &mut Mood,
            &mut Health,
            &crate::components::Name,
        )>()
    {
        // 死人不再衰减
        if dead.contains(&entity) {
            continue;
        }

        hunger.value -= hunger_per;
        thirst.value -= thirst_per;
        let wet_val = wet_map.get(&entity).copied().unwrap_or(0.0);
        let wet_energy_mult = if wet_val > 80.0 { 1.0 } else if wet_val > 50.0 { 0.5 } else { 0.0 };
        // 潮湿心情惩罚：按阈值分级，一旦湿透就不再累积
        let wet_mood = if wet_val > 80.0 { 15.0 } else if wet_val > 50.0 { 8.0 } else if wet_val > 20.0 { 3.0 } else { 0.0 };

        energy.value -= energy_per * (1.0 + wet_energy_mult);
        hunger.clamp();
        thirst.clamp();
        energy.clamp();

        if hunger.value < 40.0 {
            mood.value -= hunger_mood;
        }
        if thirst.value < 35.0 {
            mood.value -= thirst_mood;
        }
        if energy.value < 20.0 {
            mood.value -= energy_mood;
        }
        // 天气+潮湿心情 debuff：只在状态变化时调整差额，不重复累积
        // 淋到"湿透的"扣 8 心情，继续淋不再扣——除非进入更高档
        let target_debuff = weather_mood + wet_mood;
        let prev = app.weather_mood_tracker.get(&entity).copied().unwrap_or(0.0);
        let delta = target_debuff - prev;
        if delta != 0.0 {
            mood.value -= delta;
            app.weather_mood_tracker.insert(entity, target_debuff);
        }
        mood.clamp();

        if hunger.value <= 0.0 {
            health.hp -= starve_hp;
            starving.push(entity);
        }
        if thirst.value <= 0.0 {
            health.hp -= dehydrate_hp;
            dehydrating.push(entity);
        }

        if health.hp <= 0.0 {
            let cause = if hunger.value <= 0.0 && thirst.value <= 0.0 {
                "饥渴交迫"
            } else if hunger.value <= 0.0 {
                "饿死"
            } else {
                "渴死"
            };
            to_kill.push((entity, cause));
        }
    }

    // 统一调用 kill
    for (entity, cause) in to_kill {
        app.kill(entity, cause);
    }

    for &entity in &starving {
        if app.tick.is_multiple_of(100) && app.can_see_entity(entity) {
            let label = app.entity_label(entity);
            app.push_log(format!("{}饿得眼前发黑。", label));
        }
    }
    for &entity in &dehydrating {
        if app.tick.is_multiple_of(100) && app.can_see_entity(entity) {
            let label = app.entity_label(entity);
            app.push_log(format!("{}口干舌裂。", label));
        }
    }
}
