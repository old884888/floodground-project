use crate::app::App;
use crate::components::{Bush, BushState, Position};

const HOT_RADIUS: i32 = 30;
const WARM_RADIUS: i32 = 60;
const NONE_TO_GROWING: u64 = 600; // 5 天 @ 120 tick/天旧标尺；现随 ticks 推进
const GROWING_TO_FRUIT: u64 = 360;

/// 按与玩家距离推进莓果丛生长（热区全速，温区每 5 tick 一次，冷区冻结）
pub fn update_bushes(app: &mut App) {
    let (px, py) = app.player_pos();
    let ticks_per_day = app.ticks_per_day.max(1);

    // 计划按「旧 120 tick/天」写的 600/360；换算到当前一天长度
    let none_need = NONE_TO_GROWING * ticks_per_day / 120;
    let grow_need = GROWING_TO_FRUIT * ticks_per_day / 120;

    let mut updates: Vec<(hecs::Entity, BushState, u64)> = Vec::new();

    for (e, (pos, bush)) in app.world.query::<(&Position, &Bush)>().iter() {
        // 玩家死掉时灌木也会停在最后一个状态（避免游戏继续空转）
        if app.player_dead {
            break;
        }
        let dist = (pos.x - px).abs() + (pos.y - py).abs();
        let advance = if dist <= HOT_RADIUS {
            1u64
        } else if dist <= WARM_RADIUS {
            if app.tick.is_multiple_of(5) {
                1
            } else {
                0
            }
        } else {
            0
        };
        if advance == 0 {
            continue;
        }
        if bush.state == BushState::Fruiting {
            continue;
        }

        let mut timer = bush.growth_timer + advance;
        let mut state = bush.state;
        match state {
            BushState::None => {
                if timer >= none_need {
                    state = BushState::Growing;
                    timer = 0;
                }
            }
            BushState::Growing => {
                if timer >= grow_need {
                    state = BushState::Fruiting;
                    timer = 0;
                }
            }
            BushState::Fruiting => {}
        }
        updates.push((e, state, timer));
    }

    for (e, state, timer) in updates {
        if let Ok(mut bush) = app.world.get::<&mut Bush>(e) {
            bush.state = state;
            bush.growth_timer = timer;
        }
    }
}
