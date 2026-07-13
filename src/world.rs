use crate::components::TerrainKind;
use crate::data::{DataError, TerrainDef, TerrainMap};
use noise::NoiseFn;
use noise::Value;
use rand::Rng;

pub const MAP_WIDTH: i32 = 500;
pub const MAP_HEIGHT: i32 = 500;

pub const CAMP_CX: i32 = 250;
pub const CAMP_CY: i32 = 250;
pub const CAMP_HALF: i32 = 6;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Tile {
    pub terrain_id: String,
    pub terrain_kind: TerrainKind,
    pub is_walkable: bool,
    pub blocks_vision: bool,
    pub symbol: char,
    pub color_fg: (u8, u8, u8),
    pub color_bg: (u8, u8, u8),
}

#[derive(Debug)]
pub struct GameMap {
    pub width: i32,
    pub height: i32,
    tiles: Vec<Tile>,
    pub roof: Vec<bool>,
    pub window_light: Vec<u8>,
    revealed: Vec<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    Tree,
    Boulder,
    Bush,
    Stick,
    SmallStone,
    Reed,
    PoisonMush,
    MetalVein,
    WolfDen,
}

#[derive(Debug, Clone, Copy)]
pub struct PropSpawn {
    pub x: i32,
    pub y: i32,
    pub kind: PropKind,
}

pub struct MapGenResult {
    pub map: GameMap,
    pub props: Vec<PropSpawn>,
}

/// Value 噪声 → TerrainKind 映射阈值
fn terrain_from_noise(n: f64) -> TerrainKind {
    // n ∈ [0, 1]
    if n < 0.40 {
        TerrainKind::Grass
    } else if n < 0.55 {
        TerrainKind::LightForest
    } else if n < 0.65 {
        TerrainKind::DenseForest
    } else if n < 0.75 {
        TerrainKind::Sand
    } else if n < 0.85 {
        TerrainKind::Hill
    } else if n < 0.93 {
        TerrainKind::ShallowMarsh
    } else {
        TerrainKind::ShallowWater
    }
}

/// 每种地形的 spawn table：(PropKind, 概率)
fn spawn_table(terrain: TerrainKind) -> &'static [(PropKind, f32)] {
    use PropKind::*;
    static GRASS: &[(PropKind, f32)] = &[(Bush, 0.08)];
    static LIGHT_FOREST: &[(PropKind, f32)] = &[(Tree, 0.12), (Bush, 0.05)];
    static DENSE_FOREST: &[(PropKind, f32)] = &[(Tree, 0.20), (Bush, 0.08), (WolfDen, 0.01), (PoisonMush, 0.02)];
    static HILL: &[(PropKind, f32)] = &[(Boulder, 0.10), (MetalVein, 0.03)];
    static SHALLOW_MARSH: &[(PropKind, f32)] = &[(Reed, 0.10), (PoisonMush, 0.03)];
    static SHALLOW_WATER: &[(PropKind, f32)] = &[];
    static SAND: &[(PropKind, f32)] = &[(Boulder, 0.03)];
    static WATER: &[(PropKind, f32)] = &[];
    static DIRT: &[(PropKind, f32)] = &[];
    match terrain {
        TerrainKind::Grass => GRASS,
        TerrainKind::LightForest => LIGHT_FOREST,
        TerrainKind::DenseForest => DENSE_FOREST,
        TerrainKind::Hill => HILL,
        TerrainKind::ShallowMarsh => SHALLOW_MARSH,
        TerrainKind::ShallowWater => SHALLOW_WATER,
        TerrainKind::Sand => SAND,
        TerrainKind::Water => WATER,
        TerrainKind::Dirt => DIRT,
    }
}

impl GameMap {
    pub fn generate(terrain: &TerrainMap, rng: &mut impl Rng) -> Result<MapGenResult, DataError> {
        // 校验所有地形 key 存在
        for kind in [
            TerrainKind::Grass,
            TerrainKind::LightForest,
            TerrainKind::DenseForest,
            TerrainKind::Hill,
            TerrainKind::ShallowMarsh,
            TerrainKind::ShallowWater,
            TerrainKind::Sand,
            TerrainKind::Water,
            TerrainKind::Dirt,
        ] {
            if !terrain.contains_key(kind.key()) {
                return Err(DataError::MissingKey {
                    path: "terrain.ron".into(),
                    key: kind.key().into(),
                });
            }
        }

        let tile_count = (MAP_WIDTH * MAP_HEIGHT) as usize;
        let mut tiles = Vec::with_capacity(tile_count);
        let mut terrain_grid: Vec<TerrainKind> = Vec::with_capacity(tile_count);

        // ── Value 噪声生成区域 ──
        let seed: u32 = rng.gen();
        let noise = Value::new(seed);
        const NOISE_FREQ: f64 = 0.02;

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let n = noise.get([x as f64 * NOISE_FREQ, y as f64 * NOISE_FREQ]);
                let normalized = (n + 1.0) / 2.0; // ~[0, 1]
                let kind = if in_camp(x, y) {
                    TerrainKind::Dirt
                } else {
                    terrain_from_noise(normalized)
                };
                terrain_grid.push(kind);
            }
        }

        // ── 浅水区中心变深水：四邻全是浅水的格子 → Water ──
        let mut water_deepened = 0u32;
        for y in 1..MAP_HEIGHT - 1 {
            for x in 1..MAP_WIDTH - 1 {
                let idx = (y * MAP_WIDTH + x) as usize;
                if terrain_grid[idx] != TerrainKind::ShallowWater {
                    continue;
                }
                // 四个方向都是浅水 → 中心转深水
                let neighbors = [
                    ((y - 1) * MAP_WIDTH + x) as usize,
                    ((y + 1) * MAP_WIDTH + x) as usize,
                    (y * MAP_WIDTH + (x - 1)) as usize,
                    (y * MAP_WIDTH + (x + 1)) as usize,
                ];
                if neighbors.iter().all(|&ni| terrain_grid[ni] == TerrainKind::ShallowWater)
                {
                    terrain_grid[idx] = TerrainKind::Water;
                    water_deepened += 1;
                }
            }
        }
        if water_deepened > 0 {
            // 日志在 App 初始化后 push，这里只做转换计数（调试用）
            let _ = water_deepened;
        }

        // ── 根据地形查表建 Tile ──
        for &kind in &terrain_grid {
            let def = terrain.get(kind.key()).unwrap();
            tiles.push(tile_from_def(def, kind));
        }

        let roof = vec![false; tile_count];
        let window_light = vec![0u8; tile_count];
        let revealed = vec![false; tile_count];

        let map = Self {
            width: MAP_WIDTH,
            height: MAP_HEIGHT,
            tiles,
            roof,
            window_light,
            revealed,
        };

        // ── spawn table 生成实体 ──
        let mut props = Vec::new();
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                if in_camp(x, y) {
                    continue;
                }
                let idx = (y * MAP_WIDTH + x) as usize;
                let kind = terrain_grid[idx];
                if !map.is_walkable(x, y) {
                    continue;
                }
                for &(prop, chance) in spawn_table(kind) {
                    if rng.gen_bool(chance as f64) {
                        props.push(PropSpawn { x, y, kind: prop });
                        break; // 一格只放一个
                    }
                }
            }
        }

        // ── 地面散落物（少量）──
        spawn_loose_items(&map, &mut props, rng, PropKind::Stick, 2, 4);
        spawn_loose_items(&map, &mut props, rng, PropKind::SmallStone, 2, 4);

        Ok(MapGenResult { map, props })
    }

    pub fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }

    /// 返回该格的地形类型
    pub fn terrain(&self, x: i32, y: i32) -> TerrainKind {
        self.tile(x, y)
            .map(|t| t.terrain_kind)
            .unwrap_or(TerrainKind::Grass)
    }

    pub fn has_roof(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) { return false; }
        self.roof[self.idx(x, y)]
    }

    pub fn set_roof(&mut self, x: i32, y: i32, value: bool) {
        if self.in_bounds(x, y) {
            let idx = self.idx(x, y);
            self.roof[idx] = value;
        }
    }

    pub fn window_light_at(&self, x: i32, y: i32) -> u8 {
        if !self.in_bounds(x, y) { return 0; }
        self.window_light[self.idx(x, y)]
    }

    pub fn set_window_light(&mut self, x: i32, y: i32, value: u8) {
        if self.in_bounds(x, y) {
            let idx = self.idx(x, y);
            if value > self.window_light[idx] {
                self.window_light[idx] = value;
            }
        }
    }

    pub fn reveal(&mut self, x: i32, y: i32) {
        if self.in_bounds(x, y) {
            let i = self.idx(x, y);
            self.revealed[i] = true;
        }
    }

    pub fn is_revealed(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let i = self.idx(x, y);
        self.revealed[i]
    }

    pub fn reveal_radius(&mut self, cx: i32, cy: i32, radius: i32) {
        let r2 = radius * radius;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let x = cx + dx;
                let y = cy + dy;
                if self.in_bounds(x, y) && dx * dx + dy * dy <= r2 {
                    let i = self.idx(x, y);
                    self.revealed[i] = true;
                }
            }
        }
    }

    fn coords_in_bounds(x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < MAP_WIDTH && y < MAP_HEIGHT
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        Self::coords_in_bounds(x, y)
    }

    pub fn tile(&self, x: i32, y: i32) -> Option<&Tile> {
        if !self.in_bounds(x, y) { return None; }
        Some(&self.tiles[(y * self.width + x) as usize])
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.tile(x, y).map(|t| t.is_walkable).unwrap_or(false)
    }

    pub fn blocks_vision(&self, x: i32, y: i32) -> bool {
        self.tile(x, y).map(|t| t.blocks_vision).unwrap_or(true)
    }
}

fn spawn_loose_items(map: &GameMap, props: &mut Vec<PropSpawn>, rng: &mut impl Rng, kind: PropKind, min: u32, max: u32) {
    let count = rng.gen_range(min..=max);
    let mut placed = 0u32;
    let mut attempts = 0u32;
    while placed < count && attempts < 5000 {
        attempts += 1;
        let x = rng.gen_range(0..MAP_WIDTH);
        let y = rng.gen_range(0..MAP_HEIGHT);
        if in_camp(x, y) || !map.is_walkable(x, y) { continue; }
        if props.iter().any(|p| p.x == x && p.y == y) { continue; }
        props.push(PropSpawn { x, y, kind });
        placed += 1;
    }
}

fn tile_from_def(def: &TerrainDef, kind: TerrainKind) -> Tile {
    let symbol = def.symbol.chars().next().unwrap_or('?');
    Tile {
        terrain_id: def.display_name.clone(),
        terrain_kind: kind,
        is_walkable: def.is_walkable,
        blocks_vision: def.blocks_vision,
        symbol,
        color_fg: def.color_fg,
        color_bg: def.color_bg,
    }
}

pub fn in_camp(x: i32, y: i32) -> bool {
    (x - CAMP_CX).abs() < CAMP_HALF && (y - CAMP_CY).abs() < CAMP_HALF
}

pub fn within_radius(from: (i32, i32), to: (i32, i32), radius: i32) -> bool {
    if radius <= 0 { return from == to; }
    let dx = from.0 - to.0;
    let dy = from.1 - to.1;
    dx * dx + dy * dy <= radius * radius
}

pub fn has_line_of_sight(map: &GameMap, from: (i32, i32), to: (i32, i32)) -> bool {
    let (x0, y0) = from;
    let (x1, y1) = to;
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        if (x, y) != from && (x, y) != to && map.blocks_vision(x, y) { return false; }
        if x == x1 && y == y1 { return true; }
        let e2 = 2 * err;
        if e2 > -dy { err -= dy; x += sx; }
        if e2 < dx { err += dx; y += sy; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_from_noise_thresholds() {
        assert_eq!(terrain_from_noise(0.0), TerrainKind::Grass);
        assert_eq!(terrain_from_noise(0.39), TerrainKind::Grass);
        assert_eq!(terrain_from_noise(0.40), TerrainKind::LightForest);
        assert_eq!(terrain_from_noise(0.54), TerrainKind::LightForest);
        assert_eq!(terrain_from_noise(0.55), TerrainKind::DenseForest);
        assert_eq!(terrain_from_noise(0.64), TerrainKind::DenseForest);
        assert_eq!(terrain_from_noise(0.65), TerrainKind::Sand);
        assert_eq!(terrain_from_noise(0.74), TerrainKind::Sand);
        assert_eq!(terrain_from_noise(0.75), TerrainKind::Hill);
        assert_eq!(terrain_from_noise(0.84), TerrainKind::Hill);
        assert_eq!(terrain_from_noise(0.85), TerrainKind::ShallowMarsh);
        assert_eq!(terrain_from_noise(0.92), TerrainKind::ShallowMarsh);
        assert_eq!(terrain_from_noise(0.93), TerrainKind::ShallowWater);
        assert_eq!(terrain_from_noise(1.0), TerrainKind::ShallowWater);
    }

    #[test]
    fn terrain_kind_key_roundtrip() {
        for kind in [
            TerrainKind::Grass,
            TerrainKind::LightForest,
            TerrainKind::DenseForest,
            TerrainKind::Hill,
            TerrainKind::ShallowMarsh,
            TerrainKind::ShallowWater,
            TerrainKind::Sand,
            TerrainKind::Water,
            TerrainKind::Dirt,
        ] {
            let key = kind.key();
            assert_eq!(TerrainKind::from_key(key), Some(kind));
        }
        assert_eq!(TerrainKind::from_key("nonexistent"), None);
    }

    #[test]
    fn spawn_table_returns_correct_entries() {
        // 密林应该有树和狼巢穴
        let dense = spawn_table(TerrainKind::DenseForest);
        assert!(dense.iter().any(|(p, _)| *p == PropKind::Tree));
        assert!(dense.iter().any(|(p, _)| *p == PropKind::WolfDen));

        // 浅水应该为空
        let water = spawn_table(TerrainKind::ShallowWater);
        assert!(water.is_empty());

        // 丘陵应该有岩石和金属矿
        let hill = spawn_table(TerrainKind::Hill);
        assert!(hill.iter().any(|(p, _)| *p == PropKind::Boulder));
        assert!(hill.iter().any(|(p, _)| *p == PropKind::MetalVein));
    }
}
