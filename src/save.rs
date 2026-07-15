use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use hecs::{Entity, World};
use serde::{Deserialize, Serialize};

use crate::app::{App, Weather};
use crate::components as comp;
use crate::components::{ComponentSnapshot, EntityUID};
use crate::components::Pile as CompPile;
use crate::components::StatusEffect as CompStatusEffect;
use crate::world::Chunk;

const SAVE_PATH: &str = "saves/slot_01.ron.gz";
const SAVE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveData {
    pub version: u32,
    pub tick: u64,
    pub day: u64,
    pub weather: Weather,
    pub weather_timer: u64,
    pub reputation: i32,
    pub player_uid: u64,
    pub selected_uid: u64,
    pub next_uid: u64,
    pub entities: Vec<EntitySnapshot>,
    pub dirty_chunks: Vec<Chunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub uid: u64,
    pub components: Vec<ComponentSnapshot>,
}

pub fn save_game(app: &mut App) -> Result<(), String> {
    let uid_map: HashMap<Entity, u64> = app.world.query::<&EntityUID>().iter()
        .map(|(e, u)| (e, u.0)).collect();

    let mut entities = Vec::new();
    for (_, uid) in app.world.query::<&EntityUID>().iter() {
        let mut snap = EntitySnapshot { uid: uid.0, components: Vec::new() };
        collect_components(&app.world, uid.0, &uid_map, &mut snap.components);
        entities.push(snap);
    }

    let data = SaveData {
        version: SAVE_VERSION,
        tick: app.tick, day: app.day,
        weather: app.weather, weather_timer: app.weather_timer,
        reputation: app.reputation,
        player_uid: uid_map.get(&app.player).copied().unwrap_or(0),
        selected_uid: app.selected.and_then(|e| uid_map.get(&e).copied()).unwrap_or(0),
        next_uid: app.next_uid,
        entities,
        dirty_chunks: app.map.all_chunks_cloned(),
    };

    // v1 全量存档，不清 dirty（后续增量时再加）

    let ron_text = ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("序列化失败: {}", e))?;

    let dir = Path::new(SAVE_PATH).parent().unwrap();
    fs::create_dir_all(dir).map_err(|e| format!("无法创建目录: {}", e))?;

    let tmp = format!("{}.tmp", SAVE_PATH);
    {
        let f = fs::File::create(&tmp).map_err(|e| format!("创建临时文件失败: {}", e))?;
        let mut enc = GzEncoder::new(f, Compression::default());
        enc.write_all(ron_text.as_bytes()).map_err(|e| format!("写入失败: {}", e))?;
        enc.finish().map_err(|e| format!("压缩失败: {}", e))?;
    }
    fs::rename(&tmp, SAVE_PATH).map_err(|e| format!("原子写入失败: {}", e))?;
    Ok(())
}

pub fn load_game() -> Result<(SaveData, World, HashMap<u64, Entity>), String> {
    let f = fs::File::open(SAVE_PATH).map_err(|e| format!("无法打开存档: {}", e))?;
    let mut dec = GzDecoder::new(f);
    let mut text = String::new();
    dec.read_to_string(&mut text).map_err(|e| format!("解压失败: {}", e))?;
    let data: SaveData = ron::from_str(&text).map_err(|e| format!("解析失败: {}", e))?;
    if data.version != SAVE_VERSION { return Err(format!("版本不兼容: {} (当前 {})", data.version, SAVE_VERSION)); }

    let mut world = World::new();
    let mut uid_map: HashMap<u64, Entity> = HashMap::new();
    for snap in &data.entities {
        let e = world.reserve_entity();
        uid_map.insert(snap.uid, e);
    }
    for snap in &data.entities {
        let e = uid_map[&snap.uid];
        apply_components(&mut world, e, snap, &uid_map);
    }
    Ok((data, world, uid_map))
}

fn collect_components(world: &World, uid: u64, uid_map: &HashMap<Entity, u64>, out: &mut Vec<ComponentSnapshot>) {
    use ComponentSnapshot::*;
    // Find entity by UID
    let e = world.query::<&EntityUID>().iter()
        .find(|(_, u)| u.0 == uid).map(|(e, _)| e);
    let Some(e) = e else { return };
    let _ = usize::default(); // suppress unused

    if let Ok(p) = world.get::<&comp::Position>(e) { out.push(Position { x: p.x, y: p.y }); }
    if let Ok(n) = world.get::<&comp::Name>(e) { out.push(Name(n.0.clone())); }
    if world.get::<&comp::Player>(e).is_ok() { out.push(Player); }
    if world.get::<&comp::Colonist>(e).is_ok() { out.push(Colonist); }
    if let Ok(c) = world.get::<&comp::Captive>(e) { out.push(Captive { will: c.will }); }
    if world.get::<&comp::Hostile>(e).is_ok() { out.push(Hostile); }
    if world.get::<&comp::Dead>(e).is_ok() { out.push(Dead); }
    if let Ok(h) = world.get::<&comp::Health>(e) { out.push(Health { hp: h.hp, max_hp: h.max_hp }); }
    if let Ok(h) = world.get::<&comp::Hunger>(e) { out.push(Hunger { value: h.value }); }
    if let Ok(t) = world.get::<&comp::Thirst>(e) { out.push(Thirst { value: t.value }); }
    if let Ok(e2) = world.get::<&comp::Energy>(e) { out.push(Energy { value: e2.value }); }
    if let Ok(m) = world.get::<&comp::Mood>(e) { out.push(Mood { value: m.value }); }
    if let Ok(b) = world.get::<&comp::BodyTemp>(e) { out.push(BodyTemp { value: b.value }); }
    if let Ok(w) = world.get::<&comp::Wet>(e) { out.push(Wet { value: w.value }); }
    if let Ok(mc) = world.get::<&comp::MoveCooldown>(e) { out.push(MoveCooldown { ticks: mc.ticks }); }
    if world.get::<&comp::Fleeing>(e).is_ok() { out.push(Fleeing); }
    if let Ok(ai) = world.get::<&comp::AiState>(e) { out.push(AiState { current: ai.current }); }
    if let Ok(h) = world.get::<&comp::Hands>(e) { out.push(Hands { left: h.left, right: h.right }); }
    if let Ok(hv) = world.get::<&comp::Harvestable>(e) { out.push(Harvestable { hp: hv.hp, max_hp: hv.max_hp, yield_item: hv.yield_item, yield_hp_step: hv.yield_hp_step }); }
    if let Ok(p) = world.get::<&CompPile>(e) { out.push(Pile { slots: p.slots.iter().map(|s| (s.item, s.count)).collect() }); }
    if world.get::<&comp::Tree>(e).is_ok() { out.push(Tree); }
    if world.get::<&comp::Boulder>(e).is_ok() { out.push(Boulder); }
    if let Ok(b) = world.get::<&comp::Bush>(e) { out.push(Bush { state: b.state, growth_timer: b.growth_timer, yield_item: b.yield_item }); }
    if let Ok(st) = world.get::<&comp::StickTrap>(e) { out.push(StickTrap { builder_uid: uid_map.get(&st.builder).copied().unwrap_or(0) }); }
    if let Ok(d) = world.get::<&comp::Door>(e) { out.push(Door { open: d.open }); }
    if world.get::<&comp::Wall>(e).is_ok() { out.push(Wall); }
    if world.get::<&comp::WoodWall>(e).is_ok() { out.push(WoodWall); }
    if world.get::<&comp::StoneWall>(e).is_ok() { out.push(StoneWall); }
    if world.get::<&comp::Window>(e).is_ok() { out.push(Window); }
    if world.get::<&comp::Bed>(e).is_ok() { out.push(Bed); }
    if world.get::<&comp::ContainerTag>(e).is_ok() { out.push(ContainerTag); }
    if world.get::<&comp::Floor>(e).is_ok() { out.push(Floor); }
    if world.get::<&comp::DirtRoad>(e).is_ok() { out.push(DirtRoad); }
    if world.get::<&comp::StoneRoad>(e).is_ok() { out.push(StoneRoad); }
    if world.get::<&comp::Campfire>(e).is_ok() { out.push(Campfire); }
    if let Ok(ls) = world.get::<&comp::LightSource>(e) { out.push(LightSource { radius: ls.radius, brightness: ls.brightness }); }
    if world.get::<&comp::WolfDen>(e).is_ok() { out.push(WolfDen); }
    if world.get::<&comp::LeanTo>(e).is_ok() { out.push(LeanTo); }
    if world.get::<&comp::PitShelter>(e).is_ok() { out.push(PitShelter); }
    if world.get::<&comp::SmokingRack>(e).is_ok() { out.push(SmokingRack); }
    if world.get::<&comp::Puddle>(e).is_ok() { out.push(Puddle); }
    if let Ok(b) = world.get::<&comp::Building>(e) { out.push(Building { recipe_index: b.recipe_index, progress: b.progress, total: b.total }); }
    if let Ok(wip) = world.get::<&comp::CraftWip>(e) { out.push(CraftWip { recipe_index: wip.recipe_index, progress: wip.progress }); }
    if let Ok(effs) = world.get::<&Vec<CompStatusEffect>>(e) { for eff in effs.iter() { out.push(StatusEffect { kind: eff.kind, remaining: eff.remaining }); } }
    if let Ok(tt) = world.get::<&comp::TraitTag>(e) { out.push(TraitTag(tt.0.clone())); }
}

fn apply_components(world: &mut World, e: Entity, snap: &EntitySnapshot, uid_map: &HashMap<u64, Entity>) {
    use ComponentSnapshot::*;
    for c in snap.components.clone() {
        match c {
            Position { x, y } => { let _ = world.insert_one(e, comp::Position { x, y }); }
            Name(s) => { let _ = world.insert_one(e, comp::Name(s)); }
            Player => { let _ = world.insert_one(e, comp::Player); }
            Colonist => { let _ = world.insert_one(e, comp::Colonist); }
            Captive { will } => { let _ = world.insert_one(e, comp::Captive { will }); }
            Hostile => { let _ = world.insert_one(e, comp::Hostile); }
            Dead => { let _ = world.insert_one(e, comp::Dead); }
            Health { hp, max_hp } => { let _ = world.insert_one(e, comp::Health { hp, max_hp }); }
            Hunger { value } => { let _ = world.insert_one(e, comp::Hunger { value }); }
            Thirst { value } => { let _ = world.insert_one(e, comp::Thirst { value }); }
            Energy { value } => { let _ = world.insert_one(e, comp::Energy { value }); }
            Mood { value } => { let _ = world.insert_one(e, comp::Mood { value }); }
            BodyTemp { value } => { let _ = world.insert_one(e, comp::BodyTemp { value }); }
            Wet { value } => { let _ = world.insert_one(e, comp::Wet { value }); }
            MoveCooldown { ticks } => { let _ = world.insert_one(e, comp::MoveCooldown { ticks }); }
            Fleeing => { let _ = world.insert_one(e, comp::Fleeing); }
            AiState { current } => { let _ = world.insert_one(e, comp::AiState { current }); }
            Hands { left, right } => { let _ = world.insert_one(e, comp::Hands { left, right }); }
            Harvestable { hp, max_hp, yield_item, yield_hp_step } => { let _ = world.insert_one(e, comp::Harvestable { hp, max_hp, yield_item, yield_hp_step }); }
            Pile { slots } => { let mut p = CompPile::default(); for (item, count) in slots { p.add(item, count); } let _ = world.insert_one(e, p); }
            Tree => { let _ = world.insert_one(e, comp::Tree); }
            Boulder => { let _ = world.insert_one(e, comp::Boulder); }
            Bush { state, growth_timer, yield_item } => { let _ = world.insert_one(e, comp::Bush { state, growth_timer, yield_item }); }
            StickTrap { builder_uid } => { let builder = uid_map.get(&builder_uid).copied().unwrap_or(e); let _ = world.insert_one(e, comp::StickTrap { builder }); }
            Door { open } => { let _ = world.insert_one(e, comp::Door { open }); }
            Wall => { let _ = world.insert_one(e, comp::Wall); }
            WoodWall => { let _ = world.insert_one(e, comp::WoodWall); }
            StoneWall => { let _ = world.insert_one(e, comp::StoneWall); }
            Window => { let _ = world.insert_one(e, comp::Window); }
            Bed => { let _ = world.insert_one(e, comp::Bed); }
            ContainerTag => { let _ = world.insert_one(e, comp::ContainerTag); }
            Floor => { let _ = world.insert_one(e, comp::Floor); }
            DirtRoad => { let _ = world.insert_one(e, comp::DirtRoad); }
            StoneRoad => { let _ = world.insert_one(e, comp::StoneRoad); }
            Campfire => { let _ = world.insert_one(e, comp::Campfire); }
            LightSource { radius, brightness } => { let _ = world.insert_one(e, comp::LightSource { radius, brightness }); }
            WolfDen => { let _ = world.insert_one(e, comp::WolfDen); }
            LeanTo => { let _ = world.insert_one(e, comp::LeanTo); }
            PitShelter => { let _ = world.insert_one(e, comp::PitShelter); }
            SmokingRack => { let _ = world.insert_one(e, comp::SmokingRack); }
            Puddle => { let _ = world.insert_one(e, comp::Puddle); }
            Building { recipe_index, progress, total } => { let _ = world.insert_one(e, comp::Building { recipe_index, progress, total }); }
            CraftWip { recipe_index, progress } => { let _ = world.insert_one(e, comp::CraftWip { recipe_index, progress }); }
            StatusEffect { kind, remaining } => {
                let has_effs = world.get::<&Vec<CompStatusEffect>>(e).is_ok();
                if has_effs {
                    if let Ok(mut effs) = world.get::<&mut Vec<CompStatusEffect>>(e) {
                        effs.push(CompStatusEffect { kind, remaining });
                    }
                } else {
                    let _ = world.insert_one(e, vec![CompStatusEffect { kind, remaining }]);
                }
            }
            TraitTag(s) => { let _ = world.insert_one(e, comp::TraitTag(s)); }
        }
    }
}
