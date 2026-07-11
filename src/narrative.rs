use crate::app::App;
use crate::events::GameEvent;

pub fn format_event(app: &App, event: &GameEvent) -> Option<String> {
    match event {
        GameEvent::DayElapsed(day) => Some(format!("—— 第 {} 天 ——", day)),
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
        // Torture / Mood / Move 已在对应 system 里直接打日志，避免重复
        _ => None,
    }
}

fn entity_name(app: &App, entity: hecs::Entity) -> String {
    app.entity_label(entity)
}
