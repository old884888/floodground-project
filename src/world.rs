use crate::data::{DataError, TerrainDef, TerrainMap};
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
    pub is_walkable: bool,
    pub blocks_vision: bool,
    pub symbol: char,
    pub color_fg: String,
    pub color_bg: String,
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

impl GameMap {
    pub fn generate(terrain: &TerrainMap, rng: &mut impl Rng) -> Result<MapGenResult, DataError> {
        let grass = terrain.get("grass").ok_or_else(|| DataError::MissingKey {
            path: "terrain.ron".into(),
            key: "grass".into(),
        })?;
        let water = terrain.get("water").ok_or_else(|| DataError::MissingKey {
            path: "terrain.ron".into(),
            key: "water".into(),
        })?;
        let dirt = terrain.get("dirt").ok_or_else(|| DataError::MissingKey {
            path: "terrain.ron".into(),
            key: "dirt".into(),
        })?;

        let tile_count = (MAP_WIDTH * MAP_HEIGHT) as usize;
        let mut tiles = Vec::with_capacity(tile_count);
        let mut props = Vec::new();

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let def = if in_camp(x, y) {
                    dirt
                } else if is_water(x, y) {
                    water
                } else if should_dirt_patch(x, y, rng) {
                    dirt
                } else {
                    grass
                };
                tiles.push(tile_from_def(def));
            }
        }

        for y in (CAMP_CY - CAMP_HALF)..(CAMP_CY + CAMP_HALF) {
            for x in (CAMP_CX - CAMP_HALF)..(CAMP_CX + CAMP_HALF) {
                if !Self::coords_in_bounds(x, y) {
                    continue;
                }
                let idx = (y * MAP_WIDTH + x) as usize;
                tiles[idx] = tile_from_def(dirt);
            }
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

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                if in_camp(x, y) || !map.is_walkable(x, y) {
                    continue;
                }
                let dist = (x - CAMP_CX).abs().max((y - CAMP_CY).abs());

                let tree_chance = if dist < 20 { 0.10 } else if dist < 80 { 0.16 } else { 0.25 };
                if rng.gen_bool(tree_chance) {
                    props.push(PropSpawn { x, y, kind: PropKind::Tree });
                    continue;
                }

                if rng.gen_bool(0.025) {
                    props.push(PropSpawn { x, y, kind: PropKind::Boulder });
                    continue;
                }

                if rng.gen_bool(0.015) {
                    props.push(PropSpawn { x, y, kind: PropKind::Bush });
                }
            }
        }

        spawn_loose_items(&map, &mut props, rng, PropKind::Stick, 2, 4);
        spawn_loose_items(&map, &mut props, rng, PropKind::SmallStone, 2, 4);

        Ok(MapGenResult { map, props })
    }

    pub fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
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

fn tile_from_def(def: &TerrainDef) -> Tile {
    let symbol = def.symbol.chars().next().unwrap_or('?');
    Tile {
        terrain_id: def.display_name.clone(),
        is_walkable: def.is_walkable,
        blocks_vision: def.blocks_vision,
        symbol,
        color_fg: def.color_fg.clone(),
        color_bg: def.color_bg.clone(),
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

fn is_water(x: i32, y: i32) -> bool {
    let fx = x as f32;
    let fy = y as f32;
    let river_x = 180.0 + 25.0 * (fy * 0.04).sin() + 12.0 * (fy * 0.11).sin();
    if (fx - river_x).abs() < 1.6 { return true; }

    let ponds = [
        (80.0_f32, 90.0_f32, 7.0_f32, 5.0_f32),
        (400.0, 120.0, 6.0, 6.0),
        (320.0, 380.0, 8.0, 5.0),
        (100.0, 350.0, 5.0, 7.0),
        (420.0, 420.0, 6.0, 4.0),
    ];
    for (cx, cy, rx, ry) in ponds {
        let dx = (fx - cx) / rx;
        let dy = (fy - cy) / ry;
        if dx * dx + dy * dy <= 1.0 { return true; }
    }

    false
}

fn should_dirt_patch(x: i32, y: i32, rng: &mut impl Rng) -> bool {
    if in_camp(x, y) { return false; }
    rng.gen_bool(0.04)
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
