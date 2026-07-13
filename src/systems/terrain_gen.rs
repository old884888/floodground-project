//! 地形系统：狼巢穴 spawner 逻辑
//!
//! 噪声区域生成在 world.rs 里（GameMap::generate）。
//! 本模块负责运行时基于地形的逻辑——目前只有狼巢穴定期刷狼。

use rand::Rng;

use crate::app::App;
use crate::components::{Hostile, MoveCooldown, Name, Position, WolfDen};
use crate::world::MAP_WIDTH;

const WOLF_DEN_SPAWN_RADIUS: i32 = 15;
const WOLF_DEN_SPAWN_CHANCE: f64 = 0.10;

/// 每 tick 检查所有 WolfDen：半径 15 格内没有狼 → 10% 概率刷一只
pub fn update_wolf_dens(app: &mut App, rng: &mut impl Rng) {
    let dens: Vec<(hecs::Entity, (i32, i32))> = app
        .world
        .query::<(&Position, &WolfDen)>()
        .iter()
        .map(|(e, (pos, _))| (e, (pos.x, pos.y)))
        .collect();

    for (_den, (dx, dy)) in dens {
        // 检查半径内是否有活狼
        let has_wolf_nearby = app
            .world
            .query::<&Position>()
            .with::<&Hostile>()
            .iter()
            .any(|(_e, pos)| {
                (pos.x - dx).abs() + (pos.y - dy).abs() <= WOLF_DEN_SPAWN_RADIUS
            });

        if has_wolf_nearby {
            continue;
        }

        if !rng.gen_bool(WOLF_DEN_SPAWN_CHANCE) {
            continue;
        }

        // 在巢穴附近找空格刷狼
        for _ in 0..20 {
            let ox = rng.gen_range(-3..=3);
            let oy = rng.gen_range(-3..=3);
            let x = (dx + ox).clamp(0, MAP_WIDTH - 1);
            let y = (dy + oy).clamp(0, MAP_WIDTH - 1);
            if !app.map.is_walkable(x, y) {
                continue;
            }
            if app.is_blocked(x, y) {
                continue;
            }
            app.world.spawn((
                Position { x, y },
                Name("狼".into()),
                Hostile,
                crate::components::Health {
                    hp: 50.0,
                    max_hp: 50.0,
                },
                crate::components::Wet { value: 0.0 },
                MoveCooldown { ticks: 0 },
            ));
            app.mark_spatial_dirty();
            break;
        }
    }
}
