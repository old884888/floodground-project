use crate::app::App;
use crate::components::{EffectKind, TerrainKind};
use crate::events::GameEvent;

pub fn format_event(app: &mut App, event: &GameEvent) -> Option<String> {
    match event {
        GameEvent::DayElapsed(day) => Some(format!("—— 第 {} 天 ——", day)),
        GameEvent::CharacterMoved { entity, from: _, to } => {
            // 进入特殊地形 → L1 日志（仅在 actor 跨入新地形时触发一次）
            if Some(*entity) != app.actor() {
                return None;
            }
            let current = app.map.terrain(to.0, to.1);
            let prev = app.last_actor_terrain;
            app.last_actor_terrain = Some(current);
            if prev == Some(current) {
                return None;
            }
            let msg = terrain_enter_text(current);
            if msg.is_empty() {
                None
            } else {
                Some(msg.into())
            }
        }
        GameEvent::Ate { entity } => {
            let name = entity_name(app, *entity);
            Some(format!("{}找了点东西填肚子。", name))
        }
        GameEvent::Slept { entity } => {
            let name = entity_name(app, *entity);
            Some(format!("{}倒头睡了会儿。", name))
        }
        GameEvent::CaptiveBroke { entity } => {
            let name = entity_name(app, *entity);
            Some(format!(
                "{}的意志碎了。眼神空了——你得到了你想要的，也失去了别的什么。",
                name
            ))
        }
        GameEvent::ReputationChanged { delta, reason } => {
            if *delta < 0 {
                Some(format!("声誉 {}（{}）", delta, reason))
            } else {
                Some(format!("声誉 +{}（{}）", delta, reason))
            }
        }
        GameEvent::LogOnly(s) => Some(s.clone()),
        GameEvent::WeatherChanged { from: _, to } => {
            Some(match to {
                crate::app::Weather::Clear => "天晴了。云层裂开，久违的光砸了下来。",
                crate::app::Weather::Overcast => "天色阴沉下来，云层像一块拧不干的抹布。",
                crate::app::Weather::Drizzle => "细密的雨丝飘洒而下——不痛不痒，但烦。",
                crate::app::Weather::Rain => "雨落下来了。不是什么浪漫的细雨，是能把你淋透的那种。",
                crate::app::Weather::Heavy => "暴雨倾盆！天空像被人撕开了口子。找地方躲。",
                crate::app::Weather::Thunder => "轰——雷声撕裂云层，闪电把世界照成惨白。",
            }.into())
        }
        GameEvent::FireExtinguished { .. } => {
            Some("篝火在雨中挣扎了几下，灭了。".into())
        }
        GameEvent::LightningFlash => None, // 纯视觉效果，不留日志
        GameEvent::ActorDied { entity, cause } => {
            let name = entity_name(app, *entity);
            Some(format!("{}死了——{}。", name, cause))
        }
        GameEvent::StatusEffectAdded { entity, kind } => {
            if Some(*entity) != app.actor() { return None; }
            match kind {
                EffectKind::Diarrhea => Some("肚子一阵绞痛——那水不对劲。".into()),
                EffectKind::Poison => Some("毒素在血管里蔓延——你觉得浑身发冷。".into()),
            }
        }
        GameEvent::StatusEffectRemoved { entity, kind } => {
            if Some(*entity) != app.actor() { return None; }
            match kind {
                EffectKind::Diarrhea => Some("肚子终于消停了。".into()),
                EffectKind::Poison => Some("蛇毒退了——命硬。".into()),
            }
        }
        // Torture / Mood / Move 已在对应 system 里直接打日志，避免重复
        _ => None,
    }
}

fn entity_name(app: &App, entity: hecs::Entity) -> String {
    app.entity_label(entity)
}

/// 进入特殊地形的 L1 日志（只对特殊地形返回非空）
fn terrain_enter_text(kind: TerrainKind) -> &'static str {
    match kind {
        TerrainKind::DenseForest => "树枝刮过你的脸——密林果然不好走。",
        TerrainKind::Hill => "脚下的坡度变了——你爬上了丘陵。",
        TerrainKind::ShallowMarsh => "泥水浸过你的鞋——浅沼，每一步都在吮吸。",
        TerrainKind::ShallowWater => "水没过脚踝。涉水前行，裤脚已经湿透。",
        TerrainKind::Sand => "脚踩上松软的沙地——脚印留不了多久。",
        _ => "",
    }
}
