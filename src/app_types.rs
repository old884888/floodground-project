use hecs::Entity;
use rand::Rng;
use std::collections::HashMap;
use crate::village::VillageSize;

/// 制作菜单状态
#[derive(Debug, Clone)]
pub enum CraftMenuState {
    /// 正在浏览配方列表
    Browsing { cursor: usize, scroll: usize },
    /// 制作进行中
    Crafting { spinner_frame: u32 },
}

/// 空间索引：每格存同位置的实体列表 + 是否有 BlocksMovement 实体。
/// 由 `rebuild_spatial_index` 重建（每个 tick 一次）。所有 occupied/is_blocked
/// 查询都走它，避免 O(N) 全表扫描。
#[derive(Debug, Default)]
pub struct SpatialIndex {
    pub by_tile: HashMap<(i32, i32), Vec<Entity>>,
    pub blockers: HashMap<(i32, i32), bool>,
    pub vision_blockers: std::collections::HashSet<(i32, i32)>,
}

pub const DEBUG_ITEMS: &[&str] = &[
    "脚下刷5莓果",
    "脚下刷10木头",
    "脚下刷小石头×5",
    "脚下刷木棍×5",
    "脚下刷大石头×2",
    "饥渴全满",
    "面前生树",
    "面前生莓果丛",
    "刷石器工具 ▶",
    "生成生物 ▶",
    "生成聚落 ▶",
    "时间 +2h",
    "时间/日夜 ▶",
    "天气 ▶",
    "刷地形物品 ▶",
];

pub const SPAWN_ITEMS: &[&str] = &["狼", "殖民者", "俘虏"];

pub const TOOL_ITEMS: &[&str] = &["石刀", "削尖棍", "矛", "石斧", "火把"];

pub const SETTLEMENT_SIZE_ITEMS: &[(&str, VillageSize)] = &[
    ("小村 (5-8间)", VillageSize::Small),
    ("中村 (8-12间)", VillageSize::Medium),
    ("大村 (12-18间)", VillageSize::Large),
];

pub const DEBUG_TIME_ITEMS: &[&str] = &["黎明", "白天", "黄昏", "夜晚"];
pub const DEBUG_WEATHER_ITEMS: &[&str] = &["晴", "阴", "毛毛雨", "中雨", "暴雨", "雷阵雨"];
pub const DEBUG_TERRAIN_ITEMS: &[&str] = &[
    "草药", "黏土", "金属矿", "毒蘑菇", "草药植株", "狼巢穴",
    "树枝", "树叶", "长木棍", "藤条", "绳子", "石片", "大石片", "骨头",
];

pub const DEBUG_ITEM_COUNT: usize = DEBUG_ITEMS.len();

/// 子菜单索引常量，debug_execute 和 input.rs 共用
pub const DEBUG_SUB_TOOLS: usize = 8;
pub const DEBUG_SUB_CREATURES: usize = 9;
pub const DEBUG_SUB_SETTLEMENTS: usize = 10;
pub const DEBUG_SUB_TIME: usize = 12;
pub const DEBUG_SUB_WEATHER: usize = 13;
pub const DEBUG_SUB_TERRAIN: usize = 14;

// ── 天气 ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Weather {
    Clear,
    Overcast,
    Drizzle,
    Rain,
    Heavy,
    Thunder,
}

impl Weather {
    pub fn random(rng: &mut impl Rng) -> Self {
        match rng.gen_range(0u8..6) {
            0 => Weather::Clear,
            1 => Weather::Overcast,
            2 => Weather::Drizzle,
            3 => Weather::Rain,
            4 => Weather::Heavy,
            _ => Weather::Thunder,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Weather::Clear => "晴",
            Weather::Overcast => "阴",
            Weather::Drizzle => "毛毛雨",
            Weather::Rain => "中雨",
            Weather::Heavy => "暴雨",
            Weather::Thunder => "雷阵雨",
        }
    }

    /// 潮湿增长速率 (/tick)
    pub fn wet_rate(self) -> f32 {
        match self {
            Weather::Clear | Weather::Overcast => 0.0,
            Weather::Drizzle => 0.15,
            Weather::Rain => 0.4,
            Weather::Heavy => 1.0,
            Weather::Thunder => 1.5,
        }
    }

    /// 视野乘数
    pub fn visibility_multiplier(self) -> f32 {
        match self {
            Weather::Clear | Weather::Overcast => 1.0,
            Weather::Drizzle => 0.85,
            Weather::Rain => 0.70,
            Weather::Heavy => 0.50,
            Weather::Thunder => 0.45,
        }
    }

    /// 直接心情惩罚
    pub fn mood_penalty(self) -> f32 {
        match self {
            Weather::Clear => 0.0,
            Weather::Overcast => 2.0,
            Weather::Drizzle => 2.0,
            Weather::Rain => 5.0,
            Weather::Heavy => 10.0,
            Weather::Thunder => 12.0,
        }
    }

    /// 户外火源熄灭概率 (/tick)
    pub fn fire_extinguish_chance(self) -> f64 {
        match self {
            Weather::Clear | Weather::Overcast | Weather::Drizzle => 0.0,
            Weather::Rain => 0.001,    // ~10%/100tick — 小雨浇不灭
            Weather::Heavy => 0.005,   // ~40%/100tick — 暴雨有概率
            Weather::Thunder => 0.01,  // ~63%/100tick — 雷阵雨危险
        }
    }

    /// 雷阵雨闪电概率 (/帧)
    pub fn lightning_chance(self) -> f64 {
        match self {
            Weather::Thunder => 0.12,
            _ => 0.0,
        }
    }

    /// 调色板颜色乘数 (r, g, b)
    pub fn color_multiplier(self) -> (f32, f32, f32) {
        match self {
            Weather::Clear => (1.00, 1.00, 1.00),
            Weather::Overcast => (0.88, 0.88, 0.82),
            Weather::Drizzle => (0.85, 0.86, 0.82),
            Weather::Rain => (0.78, 0.80, 0.76),
            Weather::Heavy => (0.65, 0.68, 0.72),
            Weather::Thunder => (0.55, 0.58, 0.65),
        }
    }

    /// 粒子：None=无粒子，Some((glyph, 1/N格概率, color_name))
    pub fn particle(self) -> Option<(char, u32, &'static str)> {
        match self {
            Weather::Clear | Weather::Overcast => None,
            Weather::Drizzle => Some(('·', 80, "gray")),
            Weather::Rain => Some(('·', 40, "blue")),
            Weather::Heavy => Some(('│', 15, "blue")),
            Weather::Thunder => Some(('│', 10, "white")),
        }
    }

    /// 随机下一状态
    pub fn next(self, rng: &mut impl Rng) -> Self {
        let roll: f64 = rng.gen();
        match self {
            Weather::Clear => {
                if roll < 0.60 { Weather::Overcast }
                else if roll < 0.75 { Weather::Drizzle }
                else { Weather::Clear }
            }
            Weather::Overcast => {
                if roll < 0.50 { Weather::Drizzle }
                else if roll < 0.80 { Weather::Clear }
                else { Weather::Overcast }
            }
            Weather::Drizzle => {
                if roll < 0.40 { Weather::Rain }
                else if roll < 0.75 { Weather::Overcast }
                else { Weather::Drizzle }
            }
            Weather::Rain => {
                if roll < 0.30 { Weather::Heavy }
                else if roll < 0.50 { Weather::Thunder }
                else if roll < 0.80 { Weather::Drizzle }
                else { Weather::Rain }
            }
            Weather::Heavy => {
                if roll < 0.35 { Weather::Thunder }
                else if roll < 0.75 { Weather::Rain }
                else { Weather::Heavy }
            }
            Weather::Thunder => {
                if roll < 0.50 { Weather::Rain }
                else if roll < 0.80 { Weather::Heavy }
                else { Weather::Drizzle }
            }
        }
    }

    /// duration 范围 (ticks)
    pub fn duration_range(self) -> (u64, u64) {
        match self {
            Weather::Clear => (250, 750),
            Weather::Overcast => (125, 375),
            Weather::Drizzle => (125, 333),
            Weather::Rain => (83, 250),
            Weather::Heavy => (42, 167),
            Weather::Thunder => (25, 83),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    MainMenu,
    Loading,   // 新游戏加载动画
    Gameplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    Paused,
    Step,   // 按空格才走一 tick
    Normal, // 自动跑
    Fast,   // 快进 10x
    Turbo,  // 狂暴 50x
}

impl Speed {
    pub fn label(self) -> &'static str {
        match self {
            Speed::Paused => "暂停",
            Speed::Step => "步进",
            Speed::Normal => "正常",
            Speed::Fast => "快进",
            Speed::Turbo => "狂暴",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Adventure,
    Camp,
}

impl GameMode {
    pub fn label(self) -> &'static str {
        match self {
            GameMode::Adventure => "冒险",
            GameMode::Camp => "营地",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExamineAction {
    Chop,
    Mine,
    Harvest,
    Torture,
    OpenDoor,
    CloseDoor,
    SleepBed,
    SleepLeanTo,
    SleepPitShelter,
    BreakWall,
    Drink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExamineMenu {
    Pile,
    Action(ExamineAction),
    Empty,
}

#[derive(Debug, Clone)]
pub struct ExamineState {
    pub x: i32,
    pub y: i32,
    pub menu: ExamineMenu,
    pub cursor: usize,
    pub take_qty: u32, // 弹窗内捡取的抽取数量，←→调节
}

#[derive(Debug, Clone)]
pub struct DebugPopup {
    pub cursor: usize,
    pub sub: Option<DebugSubKind>,
    pub sub_cursor: usize,
}

#[derive(Debug, Clone)]
pub enum DebugSubKind {
    Tool,
    Creature,
    Settlement,
    TimePeriod,
    WeatherKind,
    TerrainItem,
}

/// 视口左上角世界坐标；跟随玩家，边缘 clamp
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub x: i32,
    pub y: i32,
}

impl Camera {
    pub fn follow(
        &mut self,
        target: (i32, i32),
        map_w: i32,
        map_h: i32,
        view_w: i32,
        view_h: i32,
    ) {
        if view_w <= 0 || view_h <= 0 {
            return;
        }
        // 地图比视口还小：钉在 (0,0)
        if map_w <= view_w && map_h <= view_h {
            self.x = 0;
            self.y = 0;
            return;
        }
        let mut cx = target.0 - view_w / 2;
        let mut cy = target.1 - view_h / 2;
        let max_x = (map_w - view_w).max(0);
        let max_y = (map_h - view_h).max(0);
        cx = cx.clamp(0, max_x);
        cy = cy.clamp(0, max_y);
        self.x = cx;
        self.y = cy;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MainMenuState {
    pub cursor: u8, // 0=开始游戏, 1=退出游戏
}

#[derive(Debug, Clone)]
pub enum BuildMenuState {
    Browsing { cursor: usize, scroll: usize },
    PickingDir { cursor: usize, scroll: usize },
    Building { recipe_index: usize }, // 进度从 Building 组件读
}

