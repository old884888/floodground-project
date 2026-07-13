use std::collections::HashMap;
use std::io;
use std::sync::OnceLock;
use thiserror::Error;

use serde::Deserialize;

#[derive(Debug, Error)]
pub enum DataError {
    #[error("无法读取数据文件 {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("数据文件 {path} 解析失败: {message}")]
    Parse { path: String, message: String },
    #[error("数据文件 {path} 校验失败: {message}")]
    Validation { path: String, message: String },
    #[error("数据文件 {path} 缺少必需键 {key}")]
    MissingKey { path: String, key: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerrainDef {
    pub display_name: String,
    pub symbol: String,
    pub color_fg: (u8, u8, u8),
    pub color_bg: (u8, u8, u8),
    pub is_walkable: bool,
    pub blocks_vision: bool,
    /// 移动 1 格消耗的 tick 倍率，1.0=正常，0=不可通行
    pub move_cost: f32,
    /// 视野半径乘数（密林 0.5 = 砍半）
    pub vis_mod: f32,
    /// 视野半径固定加成（丘陵 +3、沙地 +5）
    pub vis_flat: i32,
    /// 遮雨比例 0.0-1.0
    pub rain_shield: f32,
    /// 站在上面是否持续获得潮湿
    pub auto_wet: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActorsConfig {
    pub names: Vec<String>,
    pub captive_names: Vec<String>,
    pub hunger_range: (f32, f32),
    pub thirst_range: (f32, f32),
    pub energy_range: (f32, f32),
    pub mood_range: (f32, f32),
    pub traits: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FoodDef {
    pub name: String,
    pub hunger: f32,  // 正值=补饥饿
    pub thirst: f32,  // 正值=补口渴
}

pub type TerrainMap = std::collections::HashMap<String, TerrainDef>;
pub type FoodMap = std::collections::HashMap<String, FoodDef>;

pub fn load_terrain(path: &str) -> Result<TerrainMap, DataError> {
    let text = std::fs::read_to_string(path).map_err(|e| DataError::Io {
        path: path.into(),
        source: e,
    })?;
    ron::from_str(&text).map_err(|e| DataError::Parse {
        path: path.into(),
        message: e.to_string(),
    })
}

// ── 地形注册表（数据驱动，运行时查 move_cost/vis_mod 等）──

static TERRAIN_REGISTRY: OnceLock<TerrainMap> = OnceLock::new();

pub fn init_terrain_registry(map: TerrainMap) -> Result<(), DataError> {
    TERRAIN_REGISTRY.set(map).map_err(|_| DataError::Validation {
        path: "terrain.ron".into(),
        message: "TERRAIN_REGISTRY 已初始化过".into(),
    })
}

pub fn terrain_def(key: &str) -> &TerrainDef {
    TERRAIN_REGISTRY
        .get()
        .and_then(|m| m.get(key))
        .unwrap_or_else(|| {
            static FALLBACK: OnceLock<TerrainDef> = OnceLock::new();
            FALLBACK.get_or_init(|| TerrainDef {
                display_name: "???".into(),
                symbol: "?".into(),
                color_fg: (255, 255, 255),
                color_bg: (0, 0, 0),
                is_walkable: true,
                blocks_vision: false,
                move_cost: 1.0,
                vis_mod: 1.0,
                vis_flat: 0,
                rain_shield: 0.0,
                auto_wet: false,
            })
        })
}

pub fn load_actors(path: &str) -> Result<ActorsConfig, DataError> {
    let text = std::fs::read_to_string(path).map_err(|e| DataError::Io {
        path: path.into(),
        source: e,
    })?;
    let cfg: ActorsConfig = ron::from_str(&text).map_err(|e| DataError::Parse {
        path: path.into(),
        message: e.to_string(),
    })?;
    if cfg.names.is_empty() {
        return Err(DataError::Validation {
            path: path.into(),
            message: "names 不能为空".into(),
        });
    }
    if cfg.captive_names.is_empty() {
        return Err(DataError::Validation {
            path: path.into(),
            message: "captive_names 不能为空".into(),
        });
    }
    Ok(cfg)
}

// ── 物品注册表（数据驱动，替代 14 个文件里散落的 match 臂）──

#[derive(Debug, Clone, Deserialize)]
pub struct ItemDef {
    pub name: String,
    pub glyph: char,
    pub color: String,
    pub desc: String,
}

pub type ItemDefMap = HashMap<String, ItemDef>;

static ITEM_REGISTRY: OnceLock<ItemDefMap> = OnceLock::new();

pub fn init_item_registry(path: &str) -> Result<(), DataError> {
    let map = load_items(path)?;
    ITEM_REGISTRY.set(map).map_err(|_| DataError::Validation {
        path: path.into(),
        message: "ITEM_REGISTRY 已初始化过".into(),
    })
}

pub fn item_def(key: &str) -> &ItemDef {
    ITEM_REGISTRY
        .get()
        .and_then(|m| m.get(key))
        .unwrap_or_else(|| {
            static FALLBACK: OnceLock<ItemDef> = OnceLock::new();
            FALLBACK.get_or_init(|| ItemDef {
                name: "???".into(),
                glyph: '?',
                color: "white".into(),
                desc: "一件你完全认不出来的东西。".into(),
            })
        })
}

fn load_items(path: &str) -> Result<ItemDefMap, DataError> {
    let text = std::fs::read_to_string(path).map_err(|e| DataError::Io {
        path: path.into(),
        source: e,
    })?;
    ron::from_str(&text).map_err(|e| DataError::Parse {
        path: path.into(),
        message: e.to_string(),
    })
}

pub fn load_food(path: &str) -> Result<FoodMap, DataError> {
    let text = std::fs::read_to_string(path).map_err(|e| DataError::Io {
        path: path.into(),
        source: e,
    })?;
    ron::from_str(&text).map_err(|e| DataError::Parse {
        path: path.into(),
        message: e.to_string(),
    })
}
