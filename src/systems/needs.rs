use crate::app::App;
use crate::components::{BodyTemp, Dead, EffectKind, Energy, Health, Hunger, Mood, Position, StatusEffect, Thirst, Wet};

/// 需求衰减：饥饿/口渴/精力/体温/效果
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
    let diarrhea_mood = 10.0 / tpd;
    let poison_mood = 5.0 / tpd;

    let dead: std::collections::HashSet<hecs::Entity> = app
        .world.query::<&Dead>().iter().map(|(e, _)| e).collect();

    let wet_map: std::collections::HashMap<hecs::Entity, f32> = app
        .world.query::<&Wet>().iter().map(|(e, w)| (e, w.value)).collect();

    // BodyTemp map
    let temp_map: std::collections::HashMap<hecs::Entity, f32> = app
        .world.query::<&BodyTemp>().iter().map(|(e, t)| (e, t.value)).collect();

    // Effect 查询：只对有关键效果（腹泻）的实体做快速判断
    let diarrhea_set: std::collections::HashSet<hecs::Entity> = app
        .world.query::<&Vec<StatusEffect>>().iter()
        .filter(|(_, v)| v.iter().any(|s| s.kind == EffectKind::Diarrhea))
        .map(|(e, _)| e)
        .collect();

    let poison_set: std::collections::HashSet<hecs::Entity> = app
        .world.query::<&Vec<StatusEffect>>().iter()
        .filter(|(_, v)| v.iter().any(|s| s.kind == EffectKind::Poison))
        .map(|(e, _)| e)
        .collect();

    let weather_mood = app.weather.mood_penalty();

    let mut starving: Vec<hecs::Entity> = Vec::new();
    let mut dehydrating: Vec<hecs::Entity> = Vec::new();
    let mut to_kill: Vec<(hecs::Entity, &'static str)> = Vec::new();

    for (entity, (hunger, thirst, energy, mood, health, _name)) in app.world.query_mut::<(
        &mut Hunger, &mut Thirst, &mut Energy, &mut Mood, &mut Health, &crate::components::Name,
    )>() {
        if dead.contains(&entity) { continue; }

        // ── 基础衰减 ──
        hunger.value -= hunger_per;
        let mut thirst_mult: f32 = 1.0;
        if diarrhea_set.contains(&entity) {
            thirst_mult = 1.5;
        }
        if temp_map.get(&entity).copied().unwrap_or(60.0) > 60.0 {
            thirst_mult = thirst_mult.max(1.5);
        }
        thirst.value -= thirst_per * thirst_mult;

        let wet_val = wet_map.get(&entity).copied().unwrap_or(0.0);
        let wet = Wet { value: wet_val };
        let wet_energy_mult = wet.energy_penalty();
        let wet_mood = wet.mood_penalty();

        // ── 温度惩罚 ──
        let temp = temp_map.get(&entity).copied().unwrap_or(60.0);
        let (temp_energy_mult, temp_move_penalty) = if temp < 15.0 {
            (1.0, true) // 失温：精力+100%
        } else if temp < 30.0 {
            (0.5, false) // 很冷：精力+50%
        } else if temp < 45.0 {
            (0.3, false) // 冷：精力+30%
        } else {
            (0.0, false) // 舒服/热：无精力惩罚
        };

        energy.value -= energy_per * (1.0 + wet_energy_mult + temp_energy_mult);
        hunger.clamp(); thirst.clamp(); energy.clamp();

        // ── 心情综合 ──
        if hunger.value < 40.0 { mood.value -= hunger_mood; }
        if thirst.value < 35.0 { mood.value -= thirst_mood; }
        if energy.value < 20.0 { mood.value -= energy_mood; }

        let target_debuff = weather_mood + wet_mood
            + if diarrhea_set.contains(&entity) { diarrhea_mood } else { 0.0 }
            + if poison_set.contains(&entity) { poison_mood } else { 0.0 };
        let prev = app.weather_mood_tracker.get(&entity).copied().unwrap_or(0.0);
        let delta = target_debuff - prev;
        if delta != 0.0 {
            mood.value -= delta;
            app.weather_mood_tracker.insert(entity, target_debuff);
        }
        mood.clamp();

        // ── 失温扣血 ──
        if temp < 15.0 && app.tick.is_multiple_of(50) {
            health.hp -= 1.0;
        }
        if temp <= 0.0 && app.tick.is_multiple_of(50) {
            health.hp -= 2.0;
        }

        // ── 蛇毒扣血：每 80 tick −1 HP ──
        if poison_set.contains(&entity) && app.tick.is_multiple_of(80) {
            health.hp -= 1.0;
        }

        if hunger.value <= 0.0 {
            health.hp -= starve_hp; starving.push(entity);
        }
        if thirst.value <= 0.0 {
            health.hp -= dehydrate_hp; dehydrating.push(entity);
        }

        if health.hp <= 0.0 {
            let cause = if temp <= 0.0 { "冻死" }
            else if hunger.value <= 0.0 && thirst.value <= 0.0 { "饥渴交迫" }
            else if hunger.value <= 0.0 { "饿死" }
            else { "渴死" };
            to_kill.push((entity, cause));
        }

        let _ = temp_move_penalty; // 移速惩罚在 movement.rs 读 BodyTemp
    }

    // ── BodyTemp 趋近 ──
    for (e, (pos, temp)) in app.world.query::<(&Position, &mut BodyTemp)>().iter() {
        if dead.contains(&e) { continue; }
        let wet = wet_map.get(&e).copied().unwrap_or(0.0);
        let mut env = app.env_temperature(pos.x, pos.y, wet);
        // 衣物保暖：穿皮甲让环境温度感觉更高
        if let Ok(c) = app.world.get::<&crate::components::Clothing>(e) {
            env += c.warmth;
        }
        let diff = env - temp.value;
        temp.value = (temp.value + diff * 0.3).clamp(0.0, 100.0);
    }

    // ── StatusEffect 递减 ──
    let mut expired: Vec<(hecs::Entity, EffectKind)> = Vec::new();
    for (e, effects) in app.world.query::<&mut Vec<StatusEffect>>().iter() {
        effects.retain_mut(|eff| {
            eff.remaining = eff.remaining.saturating_sub(1);
            if eff.remaining == 0 {
                expired.push((e, eff.kind));
                false
            } else {
                true
            }
        });
    }
    for (entity, kind) in expired {
        app.events.push(crate::events::GameEvent::StatusEffectRemoved { entity, kind });
    }

    // ── kill ──
    for (entity, cause) in to_kill { app.kill(entity, cause); }
    for &entity in &starving {
        if app.tick.is_multiple_of(100) && app.can_see_entity(entity) {
            app.push_log(format!("{}饿得眼前发黑。", app.entity_label(entity)));
        }
    }
    for &entity in &dehydrating {
        if app.tick.is_multiple_of(100) && app.can_see_entity(entity) {
            app.push_log(format!("{}口干舌裂。", app.entity_label(entity)));
        }
    }
}
