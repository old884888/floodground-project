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
    pub color_fg: String,
    pub color_bg: String,
    pub is_walkable: bool,
    pub blocks_vision: bool,
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
