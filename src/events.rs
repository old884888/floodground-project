use hecs::Entity;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum GameEvent {
    Tick(u64),
    DayElapsed(u64),
    CharacterMoved {
        entity: Entity,
        from: (i32, i32),
        to: (i32, i32),
    },
    Ate {
        entity: Entity,
    },
    Slept {
        entity: Entity,
    },
    TortureCommitted {
        actor: Entity,
        victim: Entity,
        pos: (i32, i32),
        will_damage: f32,
    },
    CaptiveBroke {
        entity: Entity,
    },
    MoodChanged {
        entity: Entity,
        delta: f32,
        reason: String,
    },
    ReputationChanged {
        delta: i32,
        reason: String,
    },
    LogOnly(String),
    ItemPickedUp {
        item: crate::components::ItemKind,
    },
    ItemDropped {
        item: crate::components::ItemKind,
    },
    TreeChopped {
        damage: f32,
        hp_left: f32,
        max_hp: f32,
    },
    TreeFelled,
    BoulderMined {
        damage: f32,
        hp_left: f32,
        max_hp: f32,
    },
    BoulderDestroyed,
    BushHarvested {
        count: u32,
    },
    ActorDied {
        entity: Entity,
        cause: String,
    },
    WeatherChanged {
        from: Weather,
        to: Weather,
    },
    FireExtinguished {
        pos: (i32, i32),
    },
    LightningFlash,
}

/// 导入路径简洁：事件里引用 app 的 Weather
use crate::app::Weather;

#[derive(Debug, Default)]
pub struct EventQueue {
    events: Vec<GameEvent>,
}

impl EventQueue {
    pub fn push(&mut self, event: GameEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }
}
