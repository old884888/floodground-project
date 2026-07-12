use std::collections::{HashMap, HashSet};

use hecs::{Entity, World};
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
        BuildingTemplate {
            name: "长屋",
            palette: palette(),
            layouts: &[
                &["###W#####",
                  "#...B...W",
                  "#...T...#",
                  "#.......#",
                  "#...B...#",
                  "#...T...#",
                  "###+#####"],
                &["#W#######",
                  "#..B....W",
                  "#..T....#",
                  "#.......#",
                  "#..B....W",
                  "#..T....#",
                  "#####+###"],
            ],
            roofed: true,
        },
        BuildingTemplate {
            name: "大厅",
            palette: palette(),
            layouts: &[
                &["####W#####",
                  "#........#",
                  "#..B.T..#W",
                  "#........#",
                  "#..B.T...#",
                  "#........#",
                  "#....C..#W",
                  "####+#####"],
                &["###W######",
                  "#........#",
                  "#..B.T...W",
                  "#........#",
                  "#..B.T...W",
                  "#........#",
                  "#...C....W",
                  "######+###"],
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
            VillageSize::Small => "小村 (5-8)",
            VillageSize::Medium => "中村 (8-12)",
            VillageSize::Large => "大村 (12-18)",
        }
    }

    fn building_count(self) -> (usize, usize) {
        match self {
            VillageSize::Small => (5, 8),
            VillageSize::Medium => (8, 12),
            VillageSize::Large => (12, 18),
        }
    }

    fn template_weights(self) -> Vec<(&'static str, f64)> {
        match self {
            VillageSize::Small => vec![("小屋", 0.35), ("中屋", 0.30), ("大屋", 0.15), ("长屋", 0.10), ("工坊", 0.05), ("仓库", 0.05)],
            VillageSize::Medium => vec![("小屋", 0.20), ("中屋", 0.25), ("大屋", 0.20), ("长屋", 0.15), ("工坊", 0.10), ("仓库", 0.10)],
            VillageSize::Large => vec![("小屋", 0.05), ("中屋", 0.15), ("大屋", 0.15), ("长屋", 0.25), ("大厅", 0.15), ("工坊", 0.15), ("仓库", 0.10)],
        }
    }
}

pub fn spawn_village(
    world: &mut World,
    map: &mut GameMap,
    _spatial: &SpatialIndex,
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
    // 只记录建筑结构占位（墙/门/窗/屋顶），不把树石灌木算进去——房子可以清场
    let mut occupied: HashSet<(i32, i32)> = {
        let mut set = HashSet::new();
        for (e, pos) in world.query::<&Position>().with::<&Wall>().iter() {
            set.insert((pos.x, pos.y));
            let _ = e;
        }
        for (_e, (pos, _)) in world.query::<(&Position, &Door)>().iter() {
            set.insert((pos.x, pos.y));
        }
        for (_e, (pos, _)) in world.query::<(&Position, &Window)>().iter() {
            set.insert((pos.x, pos.y));
        }
        set
    };

    let (layout_style, radius) = match size {
        VillageSize::Small => ("circle", (4.0, 14.0)),
        VillageSize::Medium => ("mixed", (6.0, 18.0)),
        VillageSize::Large => ("mixed", (8.0, 20.0)),
    };

    // 村中心篝火——所有路汇集到这里
    if map.in_bounds(origin_x, origin_y) && map.is_walkable(origin_x, origin_y) {
        world.spawn((
            Position { x: origin_x, y: origin_y },
            Campfire,
            LightSource { radius: 10, brightness: 2 },
            BlocksMovement,
        ));
        occupied.insert((origin_x, origin_y));
    }

    let mut door_positions: Vec<(i32, i32)> = Vec::new();

    for _ in 0..count {
        for _attempt in 0..100 {
            let (bx, by) = match layout_style {
                "circle" => {
                    let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                    let dist = rng.gen_range(radius.0..radius.1);
                    (origin_x + (angle.cos() * dist) as i32,
                     origin_y + (angle.sin() * dist) as i32)
                }
                _ => {
                    // mixed: 60% 环状，40% 沿街
                    if rng.gen_bool(0.6) {
                        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                        let dist = rng.gen_range(radius.0..radius.1);
                        (origin_x + (angle.cos() * dist) as i32,
                         origin_y + (angle.sin() * dist) as i32)
                    } else {
                        let side = rng.gen_range(-16..=16);
                        let along = rng.gen_range(-20..=20);
                        if rng.gen_bool(0.5) {
                            (origin_x + along, origin_y + side)
                        } else {
                            (origin_x + side, origin_y + along)
                        }
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

            let door = place_building(world, map, tpl, bx, by, &rotated, rng);
            for (ly, row) in rotated.iter().enumerate() {
                for (lx, ch) in row.char_indices() {
                    if ch != ' ' {
                        occupied.insert((bx + lx as i32, by + ly as i32));
                    }
                }
            }
            placed.push((bx, by, w, h));
            if let Some(dp) = door {
                door_positions.push(dp);
            }
            break;
        }
    }

    // ── 泥土路：从每扇门连到村中心 ──
    let road_kind: RoadKind = match size {
        VillageSize::Small | VillageSize::Medium => RoadKind::Dirt,
        VillageSize::Large => RoadKind::Stone,
    };
    for (dx, dy) in &door_positions {
        draw_road(world, map, &occupied, *dx, *dy, origin_x, origin_y, road_kind);
    }

    // ── 围墙：以最远房子 +3 为半径，确保全包 ──
    let mut max_dist = if matches!(size, VillageSize::Large) { 12i32 } else { 0 };
    for &(bx, by, w, h) in &placed {
        let corners = [(bx, by), (bx + w, by), (bx, by + h), (bx + w, by + h)];
        for (ex, ey) in &corners {
            let d = (ex - origin_x).abs().max((ey - origin_y).abs());
            max_dist = max_dist.max(d);
        }
    }
    let fence_radius = (max_dist + 3).max(8);
    place_fence_around_v2(world, map, &occupied, origin_x, origin_y, fence_radius, rng);
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

/// 铲掉 (x,y) 上的树、岩石、灌木——清场给建筑/路让位
fn clear_tile(world: &mut World, x: i32, y: i32) {
    let to_kill: Vec<Entity> = world
        .query::<&Position>()
        .with::<&Tree>()
        .iter()
        .filter(|(_, p)| p.x == x && p.y == y)
        .map(|(e, _)| e)
        .chain(
            world.query::<&Position>().with::<&Boulder>().iter()
                .filter(|(_, p)| p.x == x && p.y == y)
                .map(|(e, _)| e),
        )
        .chain(
            world.query::<&Position>().with::<&Bush>().iter()
                .filter(|(_, p)| p.x == x && p.y == y)
                .map(|(e, _)| e),
        )
        .collect();
    for e in to_kill {
        let _ = world.despawn(e);
    }
}

fn place_building(
    world: &mut World,
    map: &mut GameMap,
    tpl: &BuildingTemplate,
    bx: i32,
    by: i32,
    layout: &[String],
    rng: &mut impl Rng,
) -> Option<(i32, i32)> {
    let mut windows: Vec<(i32, i32)> = Vec::new();
    let mut door_pos: Option<(i32, i32)> = None;
    let w = layout[0].len() as i32;
    let h = layout.len() as i32;

    // 先清场：房子脚下如果有树/石/灌木，直接铲掉
    for (ly, row) in layout.iter().enumerate() {
        for (lx, _) in row.char_indices() {
            clear_tile(world, bx + lx as i32, by + ly as i32);
        }
    }

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
                    door_pos = Some((wx, wy));
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

    // 室内光源——油灯挂屋子正中间，半径覆盖大部分房间，避免全黑
    let center_x = bx + w / 2;
    let center_y = by + h / 2;
    world.spawn((
        Position { x: center_x, y: center_y },
        LightSource { radius: 5, brightness: 1 },
    ));

    door_pos
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
    for dy in -5i32..=5 {
        for dx in -5i32..=5 {
            let dist = dx.abs().max(dy.abs());
            if dist == 0 || dist > 5 { continue; }
            let strength = 6u8.saturating_sub(dist as u8);
            let tx = wx + dx;
            let ty = wy + dy;
            if map.in_bounds(tx, ty) && map.has_roof(tx, ty) {
                map.set_window_light(tx, ty, strength);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum RoadKind {
    Dirt,
    Stone,
}

/// 简单 Chebyshev 路径：从门走到中心，每步铺路
#[allow(clippy::too_many_arguments)]
fn draw_road(
    world: &mut World,
    map: &mut GameMap,
    occupied: &HashSet<(i32, i32)>,
    mut x: i32,
    mut y: i32,
    cx: i32,
    cy: i32,
    kind: RoadKind,
) {
    let mut steps = 0;
    let max_steps = 60;
    while (x != cx || y != cy) && steps < max_steps {
        steps += 1;
        let dx = (cx - x).signum();
        let dy = (cy - y).signum();
        // Chebyshev：优先走差距大的方向，对角线也可以
        if (cx - x).abs() > (cy - y).abs() {
            x += dx;
        } else if (cy - y).abs() > (cx - x).abs() {
            y += dy;
        } else {
            x += dx;
            y += dy;
        }
        if !map.in_bounds(x, y) { break; }
        if occupied.contains(&(x, y)) { break; } // 碰到建筑就停
        if !map.is_walkable(x, y) { break; }
        // 清掉路上的树石灌木，不然路会被盖在下面看不见
        clear_tile(world, x, y);
        match kind {
            RoadKind::Dirt => {
                world.spawn((Position { x, y }, DirtRoad));
            }
            RoadKind::Stone => {
                world.spawn((Position { x, y }, StoneRoad));
            }
        }
    }
}

fn place_fence_around_v2(world: &mut World, map: &mut GameMap, occupied: &HashSet<(i32, i32)>, cx: i32, cy: i32, radius: i32, rng: &mut impl Rng) {
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
