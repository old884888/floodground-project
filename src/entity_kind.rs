//! 实体类型识别 + 单一映射来源。
//!
//! 之前 `desc.rs` 和 `ui/mod.rs` 各自维护一份 if-else 链来检测实体类型，
//! 新增组件时容易漏改其中一份。本模块提供 `classify()` 返回一个
//! `EntityKind` 枚举，所有地方（描述、glyph、priority、debug）都从它派生。
//!
//! 注意：枚举是穷尽优先的——`classify` 严格按优先级返回第一个匹配，
//! 与历史 UI 渲染顺序保持一致。

use hecs::Entity;

use crate::app::App;
use crate::components::*;

#[derive(Debug, Clone)]
pub enum EntityKind {
    Player,
    Colonist,
    Captive,
    Hostile,
    HostileFleeing,
    Door(bool),
    StoneWall,
    WoodWall,
    Window,
    Bed,
    Container,
    Floor,
    Campfire,
    Tree,
    Boulder,
    Bush(BushState),
    Pile(Option<ItemKind>),       // 主导物品
    CraftWip(usize, u32),         // 半成品（recipe_index, progress）
    Named(String),                // 兜底
}

impl EntityKind {
    pub fn classify(app: &App, e: Entity) -> Self {
        if app.world.get::<&Player>(e).is_ok() {
            return EntityKind::Player;
        }
        if app.world.get::<&Colonist>(e).is_ok() {
            return EntityKind::Colonist;
        }
        if app.world.get::<&Captive>(e).is_ok() {
            return EntityKind::Captive;
        }
        if app.world.get::<&Hostile>(e).is_ok() {
            if app.world.get::<&Fleeing>(e).is_ok() {
                return EntityKind::HostileFleeing;
            }
            return EntityKind::Hostile;
        }
        if let Ok(door) = app.world.get::<&Door>(e) {
            return EntityKind::Door(door.open);
        }
        if app.world.get::<&StoneWall>(e).is_ok() {
            return EntityKind::StoneWall;
        }
        if app.world.get::<&WoodWall>(e).is_ok() {
            return EntityKind::WoodWall;
        }
        if app.world.get::<&Window>(e).is_ok() {
            return EntityKind::Window;
        }
        if app.world.get::<&Bed>(e).is_ok() {
            return EntityKind::Bed;
        }
        if app.world.get::<&ContainerTag>(e).is_ok() {
            return EntityKind::Container;
        }
        if app.world.get::<&Campfire>(e).is_ok() {
            return EntityKind::Campfire;
        }
        if app.world.get::<&Tree>(e).is_ok() {
            return EntityKind::Tree;
        }
        if app.world.get::<&Boulder>(e).is_ok() {
            return EntityKind::Boulder;
        }
        if let Ok(bush) = app.world.get::<&Bush>(e) {
            return EntityKind::Bush(bush.state);
        }
        if let Ok(pile) = app.world.get::<&Pile>(e) {
            return EntityKind::Pile(pile.dominant().map(|(k, _)| k));
        }
        if let Ok(wip) = app.world.get::<&CraftWip>(e) {
            return EntityKind::CraftWip(wip.recipe_index, wip.progress);
        }
        if app.world.get::<&Floor>(e).is_ok() {
            return EntityKind::Floor;
        }
        if let Ok(name) = app.world.get::<&Name>(e) {
            return EntityKind::Named(name.0.clone());
        }
        EntityKind::Named(String::new())
    }

    /// 同格绘制优先级：越大越靠上。`@` 玩家最高，树/岩其次，
    /// 地面装饰（floor）最低。
    pub fn draw_priority(&self) -> u8 {
        match self {
            EntityKind::Player => 100,
            EntityKind::Colonist | EntityKind::Captive => 90,
            EntityKind::Hostile | EntityKind::HostileFleeing => 89,
            EntityKind::Campfire => 80,
            EntityKind::Door(_) | EntityKind::Window => 60,
            EntityKind::WoodWall | EntityKind::StoneWall => 55,
            EntityKind::Tree | EntityKind::Boulder => 50,
            EntityKind::Bed | EntityKind::Container => 40,
            EntityKind::Bush(_) => 30,
            EntityKind::Pile(_) => 15,
            EntityKind::CraftWip(_, _) => 14,
            EntityKind::Floor => 5,
            EntityKind::Named(_) => 1,
        }
    }

    /// ASCII 字形 + 主色
    pub fn glyph(&self) -> (char, ratatui::style::Color) {
        use ratatui::style::Color;
        match self {
            EntityKind::Player => ('@', Color::White),
            EntityKind::Colonist => ('C', Color::Cyan),
            EntityKind::Captive => ('p', Color::Magenta),
            EntityKind::Hostile | EntityKind::HostileFleeing => ('w', Color::Red),
            EntityKind::Door(open) => (
                if *open { '/' } else { '+' },
                Color::Yellow,
            ),
            EntityKind::StoneWall => ('#', Color::White),
            EntityKind::WoodWall => ('#', Color::Yellow),
            EntityKind::Window => ('\u{2592}', Color::Cyan), // ▒
            EntityKind::Bed => ('=', Color::Magenta),
            EntityKind::Container => ('[', Color::Yellow),
            EntityKind::Campfire => ('^', Color::Yellow),
            EntityKind::Tree => ('T', Color::Green),
            EntityKind::Boulder => ('A', Color::Gray),
            EntityKind::Bush(state) => match state {
                BushState::Fruiting => ('%', Color::Red),
                BushState::Growing => ('*', Color::Green),
                BushState::None => ('"', Color::DarkGray),
            },
            EntityKind::Floor => ('.', Color::Rgb(60, 40, 20)),
            EntityKind::Pile(Some(item)) => crate::ui::item_glyph(*item),
            EntityKind::Pile(None) => ('.', Color::DarkGray),
            EntityKind::CraftWip(_, _) => ('…', Color::Yellow),
            EntityKind::Named(_) => ('?', Color::Gray),
        }
    }
}
