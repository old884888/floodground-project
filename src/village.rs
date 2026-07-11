use std::collections::{HashMap, HashSet};

use hecs::World;
use rand::Rng;

use crate::app::SpatialIndex;
use crate::components::*;
use crate::world::{GameMap, MAP_HEIGHT, MAP_WIDTH, CAMP_CX, CAMP_CY};

#[derive(Debug, Clone)]
pub struct BuildingTemplate {
    pub name: &'static str,
    pub palette: HashMap<char, &'static str>,
    pub layouts: &'static [&'static [&'static str]],
    pub roofed: bool,
}

fn palette() -> HashMap<char, &'static str> {
    let mut p = HashMap::new();
    p.insert('#', "WoodWall");
    p.insert('|', "StoneWall");
    p.insert('+', "Door");
    p.insert('W', "Window");
    p.insert('B', "Bed");
    p.insert('T', "Table");
    p.insert('C', "Chair");
    p.insert('X', "Chest");
    p.insert('^', "Campfire");
    p.insert('.', "WoodFloor");
    p.insert(',', "StoneFloor");
    p.insert(' ', "None");
    p.insert('@', "Workbench");
    p.insert('$', "Furnace");
    p
}

fn all_templates() -> Vec<BuildingTemplate> {
    vec![
        BuildingTemplate {
            name: "小屋",
            palette: palette(),
            layouts: &[
                &["##W##",
                  "#.B.#",
                  "#.T.#",
                  "##+##"],
                &["#W###",
                  "#.B.#",
                  "#.T.W",
                  "##+##"],
            ],
            roofed: true,
        },
        BuildingTemplate {
            name: "中屋",
            palette: palette(),
            layouts: &[
                &["#######",
                  "#..B..#",
                  "#..T..W",
                  "#..C..#",
                  "###+###"],
                &["##W####",
                  "#..B..#",
                  "#..T..#",
                  "#..C..W",
                  "###+###"],
            ],
            roofed: true,
        },
        BuildingTemplate {
            name: "大屋",
            palette: palette(),
            layouts: &[
                &["####+###",
                  "#..B..#W",
                  "#..T...#",
                  "#..C..#W",
                  "#..B...#",
                  "###+####"],
                &["##W#####",
                  "#..B..#W",
                  "#..T...#",
                  "#..C...#",
                  "#..B..#+",
                  "###+####"],
            ],
            roofed: true,
        },
        BuildingTemplate {
            name: "工坊",
            palette: palette(),
            layouts: &[
                &["##W##",
                  "#.@.#",
                  "#.T.#",
                  "##$+#"],
            ],
            roofed: true,
        },
        BuildingTemplate {
            name: "仓库",
            palette: palette(),
            layouts: &[
                &["#####",
                  "#.X.W",
                  "#.X.#",
                  "##+##"],
            ],
            roofed: true,
        },
    ]
}

/// 死硬约束：每个 layout 在所有 4 个旋转方向下，门窗必须贴在 perimeter 上
/// 且不能跑进四个角落。敢违反就在启动时当场爆炸，别等到生成出一堆 sb 房子。
pub fn validate_templates() {
    let templates = all_templates();
    for tpl in &templates {
        for (li, layout) in tpl.layouts.iter().enumerate() {
            let rotated_versions: Vec<Vec<String>> = (0..4)
                .map(|r| {
                    let mut v: Vec<String> = layout.iter().map(|s| s.to_string()).collect();
                    for _ in 0..r {
                        v = rotate_matrix_90(&v);
                    }
                    v
                })
                .collect();
            for (rot, mat) in rotated_versions.iter().enumerate() {
                check_layout(tpl.name, li, rot, mat);
            }
        }
    }
}

fn check_layout(name: &str, layout_idx: usize, rotation: usize, layout: &[String]) {
    let h = layout.len();
    if h == 0 { return; }
    let w = layout[0].len();
    for (y, row) in layout.iter().enumerate() {
        if row.chars().count() != w {
            panic!(
                "模板 {} layout #{} rot={} 宽度不均：第 {} 行 {} 字符，但应该是 {}。补空格去你妈的。",
                name, layout_idx, rotation, y, row.chars().count(), w
            );
        }
        for (x, ch) in row.char_indices() {
            let on_perimeter = y == 0 || y == h - 1 || x == 0 || x == w - 1;
            let on_corner = (x == 0 || x == w - 1) && (y == 0 || y == h - 1);
            match ch {
                '+' | 'W' => {
                    if !on_perimeter {
                        panic!(
                            "模板 {} layout #{} rot={} 在 ({},{}) 出现 '{}'，但这玩意在墙里不是墙上。门窗必须贴在 perimeter。",
                            name, layout_idx, rotation, x, y, ch
                        );
                    }
                    if on_corner {
                        panic!(
                            "模板 {} layout #{} rot={} 在 ({},{}) 出现 '{}'，结果这他妈的正好在角上，你让玩家从哪儿走进去？",
                            name, layout_idx, rotation, x, y, ch
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VillageSize {
    Small,
    Medium,
    Large,
}

impl VillageSize {
    pub fn label(self) -> &'static str {
        match self {
            VillageSize::Small => "小村 (3-5)",
            VillageSize::Medium => "中村 (5-8)",
            VillageSize::Large => "大村 (8-12)",
        }
    }

    fn building_count(self) -> (usize, usize) {
        match self {
            VillageSize::Small => (3, 5),
            VillageSize::Medium => (5, 8),
            VillageSize::Large => (8, 12),
        }
    }

    fn template_weights(self) -> Vec<(&'static str, f64)> {
        match self {
            VillageSize::Small => vec![("小屋", 0.65), ("中屋", 0.35)],
            VillageSize::Medium => vec![("小屋", 0.40), ("中屋", 0.35), ("工坊", 0.15), ("仓库", 0.10)],
            VillageSize::Large => vec![("小屋", 0.30), ("中屋", 0.30), ("大屋", 0.15), ("工坊", 0.10), ("仓库", 0.15)],
        }
    }
}

pub fn spawn_village(
    world: &mut World,
    map: &mut GameMap,
    spatial: &SpatialIndex,
    origin_x: i32,
    origin_y: i32,
    size: VillageSize,
    rng: &mut impl Rng,
) {
    let (min_count, max_count) = size.building_count();
    let count = rng.gen_range(min_count..=max_count);
    let templates = all_templates();
    let weights = size.template_weights();
    let mut placed: Vec<(i32, i32, i32, i32)> = Vec::new();
    let mut occupied: HashSet<(i32, i32)> = spatial.by_tile.keys().copied().collect();

    let layout_style = match size {
        VillageSize::Small => "circle",
        VillageSize::Medium | VillageSize::Large => "street",
    };

    for _ in 0..count {
        for _attempt in 0..50 {
            let (bx, by) = match layout_style {
                "circle" => {
                    let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                    let dist = rng.gen_range(3.0..12.0);
                    (origin_x + (angle.cos() * dist) as i32,
                     origin_y + (angle.sin() * dist) as i32)
                }
                _ => {
                    let side = rng.gen_range(-12..=12);
                    let along = rng.gen_range(-15..=15);
                    if rng.gen_bool(0.5) {
                        (origin_x + along, origin_y + side)
                    } else {
                        (origin_x + side, origin_y + along)
                    }
                }
            };

            let tpl = pick_template(&templates, &weights, rng);
            let rotated = rotate_layout(tpl.layouts, rng);
            let (w, h) = (rotated[0].len() as i32, rotated.len() as i32);

            let overlap = placed.iter().any(|(px, py, pw, ph)| {
                bx < px + pw + 2 && bx + w > px - 2 &&
                by < py + ph + 2 && by + h > py - 2
            });
            if overlap { continue; }

            if !can_place_building_v2(map, &occupied, bx, by, &rotated) { continue; }

            place_building(world, map, tpl, bx, by, &rotated, rng);
            for (ly, row) in rotated.iter().enumerate() {
                for (lx, ch) in row.char_indices() {
                    if ch != ' ' {
                        occupied.insert((bx + lx as i32, by + ly as i32));
                    }
                }
            }
            placed.push((bx, by, w, h));
            break;
        }
    }

    if matches!(size, VillageSize::Large) {
        place_fence_around_v2(world, map, &occupied, origin_x, origin_y, rng);
    }
}

fn pick_template<'a>(templates: &'a [BuildingTemplate], weights: &[(&str, f64)], rng: &mut impl Rng) -> &'a BuildingTemplate {
    let roll: f64 = rng.gen();
    let mut acc = 0.0;
    for (name, weight) in weights {
        acc += weight;
        if roll <= acc {
            if let Some(t) = templates.iter().find(|t| t.name == *name) {
                return t;
            }
        }
    }
    &templates[0]
}

fn rotate_layout(layouts: &[&[&str]], rng: &mut impl Rng) -> Vec<String> {
    let layout_idx = rng.gen_range(0..layouts.len());
    let original: Vec<String> = layouts[layout_idx].iter().map(|s| s.to_string()).collect();
    let rotations: usize = rng.gen_range(0..4);
    let mut result = original;
    for _ in 0..rotations {
        result = rotate_matrix_90(&result);
    }
    result
}

fn rotate_matrix_90(layout: &[String]) -> Vec<String> {
    let h = layout.len();
    if h == 0 { return vec![]; }
    let w = layout[0].len();
    let mut rotated = vec![String::with_capacity(h); w];
    for (x, row) in rotated.iter_mut().enumerate().take(w) {
        for y in (0..h).rev() {
            let ch = layout[y].chars().nth(x).unwrap_or(' ');
            row.push(ch);
        }
    }
    rotated
}

fn can_place_building_v2(map: &GameMap, occupied: &HashSet<(i32, i32)>, bx: i32, by: i32, layout: &[String]) -> bool {
    let h = layout.len() as i32;
    if h == 0 { return false; }
    let w = layout[0].len() as i32;
    for y in 0..h {
        for x in 0..w {
            let wx = bx + x;
            let wy = by + y;
            if !map.in_bounds(wx, wy) { return false; }
            if &layout[y as usize][x as usize..x as usize + 1] != " " && !map.is_walkable(wx, wy) {
                return false;
            }
            if &layout[y as usize][x as usize..x as usize + 1] != " "
                && occupied.contains(&(wx, wy)) {
                    return false;
                }
        }
    }
    true
}

fn place_building(
    world: &mut World,
    map: &mut GameMap,
    tpl: &BuildingTemplate,
    bx: i32,
    by: i32,
    layout: &[String],
    rng: &mut impl Rng,
) {
    let mut windows: Vec<(i32, i32)> = Vec::new();

    for (ly, row) in layout.iter().enumerate() {
        for (lx, ch) in row.char_indices() {
            let wx = bx + lx as i32;
            let wy = by + ly as i32;
            let kind = tpl.palette.get(&ch).copied().unwrap_or("None");

            match kind {
                "WoodWall" | "StoneWall" => {
                    let is_stone = kind == "StoneWall" || (kind == "WoodWall" && rng.gen_bool(0.15));
                    spawn_wall(world, wx, wy, is_stone);
                    map.set_roof(wx, wy, tpl.roofed);
                }
                "Door" => {
                    world.spawn((
                        Position { x: wx, y: wy },
                        Door { open: false },
                        Wall,
                        BlocksMovement,
                        BlocksVision,
                        harvest_for("WoodDoor"),
                    ));
                    map.set_roof(wx, wy, tpl.roofed);
                }
                "Window" => {
                    world.spawn((
                        Position { x: wx, y: wy },
                        Window,
                        Wall,
                        BlocksMovement,
                        BlocksVision,
                        harvest_for("Window"),
                    ));
                    windows.push((wx, wy));
                    map.set_roof(wx, wy, false);
                }
                "Bed" => {
                    world.spawn((
                        Position { x: wx, y: wy },
                        Bed,
                        harvest_for("WoodBed"),
                    ));
                    map.set_roof(wx, wy, tpl.roofed);
                }
                "Table" => {
                    world.spawn((
                        Position { x: wx, y: wy },
                        Name("桌子".into()),
                        harvest_for("WoodTable"),
                    ));
                    map.set_roof(wx, wy, tpl.roofed);
                }
                "Chair" => {
                    world.spawn((
                        Position { x: wx, y: wy },
                        Name("椅子".into()),
                        harvest_for("WoodChair"),
                    ));
                    map.set_roof(wx, wy, tpl.roofed);
                }
                "Chest" => {
                    world.spawn((
                        Position { x: wx, y: wy },
                        ContainerTag,
                        Pile::default(),
                        Name("储物箱".into()),
                        harvest_for("StorageChest"),
                    ));
                    map.set_roof(wx, wy, tpl.roofed);
                }
                "Campfire" => {
                    world.spawn((
                        Position { x: wx, y: wy },
                        Campfire,
                        LightSource { radius: 8, brightness: 2 },
                        BlocksMovement,
                    ));
                    map.set_roof(wx, wy, tpl.roofed);
                }
                "WoodFloor" => {
                    world.spawn((
                        Position { x: wx, y: wy },
                        Floor,
                    ));
                    map.set_roof(wx, wy, tpl.roofed);
                }
                "StoneFloor" => {
                    world.spawn((
                        Position { x: wx, y: wy },
                        Floor,
                    ));
                    map.set_roof(wx, wy, tpl.roofed);
                }
                "Workbench" => {
                    world.spawn((
                        Position { x: wx, y: wy },
                        Name("工作台(占位)".into()),
                        harvest_for("Workbench"),
                    ));
                    map.set_roof(wx, wy, tpl.roofed);
                }
                "Furnace" => {
                    world.spawn((
                        Position { x: wx, y: wy },
                        Name("熔炉(占位)".into()),
                        harvest_for("Furnace"),
                    ));
                    map.set_roof(wx, wy, tpl.roofed);
                }
                _ => {}
            }
        }
    }

    for &(wx, wy) in &windows {
        propagate_window_light(map, wx, wy);
    }
}

fn spawn_wall(world: &mut World, x: i32, y: i32, is_stone: bool) {
    if is_stone {
        world.spawn((
            Position { x, y },
            StoneWall,
            Wall,
            BlocksMovement,
            BlocksVision,
            Harvestable {
                hp: 800.0,
                max_hp: 800.0,
                yield_item: ItemKind::BigStone,
                yield_hp_step: 100.0,
            },
        ));
    } else {
        world.spawn((
            Position { x, y },
            WoodWall,
            Wall,
            BlocksMovement,
            BlocksVision,
            Harvestable {
                hp: 300.0,
                max_hp: 300.0,
                yield_item: ItemKind::Wood,
                yield_hp_step: 100.0,
            },
        ));
    }
}

fn harvest_for(name: &str) -> Harvestable {
    match name {
        "WoodDoor" => Harvestable { hp: 300.0, max_hp: 300.0, yield_item: ItemKind::Wood, yield_hp_step: 100.0 },
        "Window" => Harvestable { hp: 100.0, max_hp: 100.0, yield_item: ItemKind::Wood, yield_hp_step: 100.0 },
        "WoodBed" => Harvestable { hp: 200.0, max_hp: 200.0, yield_item: ItemKind::Wood, yield_hp_step: 100.0 },
        "WoodTable" => Harvestable { hp: 200.0, max_hp: 200.0, yield_item: ItemKind::Wood, yield_hp_step: 100.0 },
        "WoodChair" => Harvestable { hp: 100.0, max_hp: 100.0, yield_item: ItemKind::Wood, yield_hp_step: 100.0 },
        "StorageChest" => Harvestable { hp: 250.0, max_hp: 250.0, yield_item: ItemKind::Wood, yield_hp_step: 100.0 },
        "Workbench" => Harvestable { hp: 300.0, max_hp: 300.0, yield_item: ItemKind::Wood, yield_hp_step: 100.0 },
        "Furnace" => Harvestable { hp: 500.0, max_hp: 500.0, yield_item: ItemKind::BigStone, yield_hp_step: 100.0 },
        _ => Harvestable { hp: 100.0, max_hp: 100.0, yield_item: ItemKind::Wood, yield_hp_step: 100.0 },
    }
}

fn propagate_window_light(map: &mut GameMap, wx: i32, wy: i32) {
    for dy in -3i32..=3 {
        for dx in -3i32..=3 {
            let dist = dx.abs().max(dy.abs());
            if dist == 0 || dist > 3 { continue; }
            let strength = 4u8.saturating_sub(dist as u8);
            let tx = wx + dx;
            let ty = wy + dy;
            if map.in_bounds(tx, ty) && map.has_roof(tx, ty) {
                map.set_window_light(tx, ty, strength);
            }
        }
    }
}

fn place_fence_around_v2(world: &mut World, map: &mut GameMap, occupied: &HashSet<(i32, i32)>, cx: i32, cy: i32, rng: &mut impl Rng) {
    let radius = rng.gen_range(12i32..=18);
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx.abs().max(dy.abs()) != radius { continue; }
            let wx = cx + dx;
            let wy = cy + dy;
            if !map.in_bounds(wx, wy) || !map.is_walkable(wx, wy) { continue; }
            if map.has_roof(wx, wy) { continue; }
            if occupied.contains(&(wx, wy)) { continue; }
            world.spawn((
                Position { x: wx, y: wy },
                WoodWall,
                Wall,
                BlocksMovement,
                BlocksVision,
                Harvestable { hp: 150.0, max_hp: 150.0, yield_item: ItemKind::Stick, yield_hp_step: 100.0 },
            ));
        }
    }
    add_fence_gate_v2(world, map, occupied, cx, cy, radius, rng);
}

fn add_fence_gate_v2(world: &mut World, map: &mut GameMap, occupied: &HashSet<(i32, i32)>, cx: i32, cy: i32, radius: i32, rng: &mut impl Rng) {
    let sides: [(i32, i32); 4] = [(0, -radius), (0, radius), (-radius, 0), (radius, 0)];
    let count = rng.gen_range(1..=3);
    for side in &sides[0..count.min(4)] {
        let wx = cx + side.0;
        let wy = cy + side.1;
        if !map.in_bounds(wx, wy) || map.has_roof(wx, wy) { continue; }
        if occupied.contains(&(wx, wy)) { continue; }
        world.spawn((
            Position { x: wx, y: wy },
            Door { open: false },
            Wall,
            BlocksMovement,
            BlocksVision,
            harvest_for("WoodDoor"),
        ));
    }
}

pub fn village_centers(rng: &mut impl Rng) -> Vec<(i32, i32)> {
    let candidate_count = rng.gen_range(3..=5);
    let min_camp_dist = 15;
    let min_center_gap = 30;
    let mut centers: Vec<(i32, i32)> = Vec::new();

    for _ in 0..500 {
        if centers.len() >= candidate_count { break; }
        let cx = rng.gen_range(10..MAP_WIDTH - 10);
        let cy = rng.gen_range(10..MAP_HEIGHT - 10);
        if (cx - CAMP_CX).abs().max((cy - CAMP_CY).abs()) < min_camp_dist { continue; }
        let too_close = centers.iter().any(|(px, py)| {
            (cx - px).abs().max((cy - py).abs()) < min_center_gap
        });
        if too_close { continue; }
        centers.push((cx, cy));
    }

    centers
}
