//! 猎物生态：DF 模式种群管理 + 刷新 + 繁殖

use rand::Rng;
use std::collections::HashMap;

use crate::app::App;
use crate::components::*;
use crate::world::{CAMP_CX, CAMP_CY, MAP_HEIGHT, MAP_WIDTH};

/// 每生态区每种猎物的种群上限
fn max_population(kind: AnimalKind, _terrain: TerrainKind) -> u32 {
    match kind {
        AnimalKind::Deer => 8,
        AnimalKind::Boar => 5,
        AnimalKind::Rabbit => 12,
    }
}

/// 每种猎物的生成生态区
fn valid_terrains(kind: AnimalKind) -> &'static [TerrainKind] {
    match kind {
        AnimalKind::Deer => &[TerrainKind::Grass, TerrainKind::LightForest],
        AnimalKind::Boar => &[TerrainKind::DenseForest, TerrainKind::Hill],
        AnimalKind::Rabbit => &[TerrainKind::Grass, TerrainKind::LightForest, TerrainKind::Sand],
    }
}

/// 每 50 tick 调用：种群维护 + 刷新 + 繁殖
pub fn update_ecology(app: &mut App, rng: &mut impl Rng) {
    if !app.tick.is_multiple_of(50) { return; }

    let mut pop_map: HashMap<(AnimalKind, TerrainKind), u32> = HashMap::new();
    for (_, (animal, pos)) in app.world.query::<(&Animal, &Position)>().iter() {
        let terrain = app.map.terrain(pos.x, pos.y);
        *pop_map.entry((animal.kind, terrain)).or_default() += 1;
    }

    for kind in &[AnimalKind::Deer, AnimalKind::Boar, AnimalKind::Rabbit] {
        for &terrain in valid_terrains(*kind) {
            let current = pop_map.get(&(*kind, terrain)).copied().unwrap_or(0);
            let max = max_population(*kind, terrain);

            // 刷新：低于上限时 5% 概率在边缘刷
            if current < max && rng.gen_bool(0.05) {
                spawn_at_edge(app, *kind, rng);
            }

            // 繁殖：低于 max 时极低概率
            if current < max && current >= 1 && rng.gen_bool(0.005) {
                let entities: Vec<hecs::Entity> = app.world.query::<&Animal>().iter()
                    .filter(|(_, a)| a.kind == *kind && a.adult)
                    .map(|(e, _)| e)
                    .collect();
                if !entities.is_empty() {
                    let parent = entities[rng.gen_range(0..entities.len())];
                    let ppos = app.world.get::<&Position>(parent).ok().map(|p| (p.x, p.y));
                    if let Some((px, py)) = ppos {
                        let (bx, by) = (
                            (px + rng.gen_range(-3..=3)).clamp(0, MAP_WIDTH - 1),
                            (py + rng.gen_range(-3..=3)).clamp(0, MAP_HEIGHT - 1),
                        );
                        if app.map.is_walkable(bx, by) && !app.is_blocked(bx, by) {
                            let uid = app.next_uid; app.next_uid += 1;
                            app.world.spawn((
                                Position { x: bx, y: by },
                                EntityUID(uid),
                                Animal { kind: *kind, adult: false },
                                Name(animal_name(*kind)),
                                Health { hp: young_hp(*kind), max_hp: adult_hp(*kind) },
                                MoveCooldown { ticks: 0 },
                            ));
                        }
                    }
                }
            }
        }
    }

    // 幼崽长大
    for (_, animal) in app.world.query::<&mut Animal>().iter() {
        if !animal.adult && app.tick.is_multiple_of(5000) {
            animal.adult = true;
        }
    }
}

fn spawn_at_edge(app: &mut App, kind: AnimalKind, rng: &mut impl Rng) {
    let terrains = valid_terrains(kind);
    for _ in 0..50 {
        let edge_side = rng.gen_range(0..4);
        let (x, y) = match edge_side {
            0 => (rng.gen_range(0..MAP_WIDTH), 0),
            1 => (rng.gen_range(0..MAP_WIDTH), MAP_HEIGHT - 1),
            2 => (0, rng.gen_range(0..MAP_HEIGHT)),
            _ => (MAP_WIDTH - 1, rng.gen_range(0..MAP_HEIGHT)),
        };
        let terrain = app.map.terrain(x, y);
        if !terrains.contains(&terrain) { continue; }
        if !app.map.is_walkable(x, y) || app.is_blocked(x, y) { continue; }
        if (x - CAMP_CX).abs() < 10 && (y - CAMP_CY).abs() < 10 { continue; }
        let uid = app.next_uid; app.next_uid += 1;
        app.world.spawn((
            Position { x, y },
            EntityUID(uid),
            Animal { kind, adult: true },
            Name(animal_name(kind)),
            Health { hp: adult_hp(kind), max_hp: adult_hp(kind) },
            MoveCooldown { ticks: 0 },
        ));
        app.mark_spatial_dirty();
        return;
    }
}

fn animal_name(kind: AnimalKind) -> String {
    match kind { AnimalKind::Deer => "鹿".into(), AnimalKind::Boar => "野猪".into(), AnimalKind::Rabbit => "兔子".into() }
}

fn adult_hp(kind: AnimalKind) -> f32 {
    match kind { AnimalKind::Deer => 80.0, AnimalKind::Boar => 120.0, AnimalKind::Rabbit => 20.0 }
}

fn young_hp(kind: AnimalKind) -> f32 { adult_hp(kind) * 0.4 }
