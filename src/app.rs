use hecs::{Entity, World};
use rand::seq::SliceRandom;
use rand::Rng;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::components::*;
use crate::data::{ActorsConfig, DataError, FoodMap, TerrainMap};
use crate::events::EventQueue;
use crate::village::{self, VillageSize};
use crate::world::{GameMap, PropKind, CAMP_CX, CAMP_CY};


pub use crate::app_types::*;

pub struct App {
    pub screen: Screen,
    pub menu: MainMenuState,
    pub build_menu: Option<BuildMenuState>,
    /// 当前建造中的目标（x, y, 产物类型）；建造完成或中断后清空
    pub build_target: Option<(i32, i32, crate::systems::building::BuildTarget)>,
    /// 建造前速度，建完恢复
    pub pre_build_speed: Option<Speed>,
    pub world: World,
    pub map: GameMap,
    pub spatial: SpatialIndex,
    /// 空间索引是否过期（spawn/despawn/移动后设 true，rebuild 后设 false）。
    /// 静止 tick 跳过重建，省 50-75% rebuild 开销。
    pub spatial_dirty: bool,
    /// 火光照亮缓存：(x,y) → 是否被火照亮。跨帧有效，火源变化时清空。
    lit_cache: RefCell<HashMap<(i32, i32), bool>>,
    /// 视线缓存：to 坐标 → 玩家能否看见。vis_anchor 为 from 锚点，
    /// 玩家移动后 anchor 变化 → 自动清空。
    vis_cache: RefCell<HashMap<(i32, i32), bool>>,
    vis_anchor: Cell<Option<(i32, i32)>>,
    pub camera: Camera,
    pub events: EventQueue,
    pub log: Vec<String>,
    pub tick: u64,
    pub day: u64,
    pub ticks_per_day: u64,
    pub reputation: i32,
    pub speed: Speed,
    pub selected: Option<Entity>,
    pub player: Entity,
    pub should_quit: bool,
    pub quit_menu: bool,
    pub next_uid: u64,
    /// 玩家死了 → 暂停所有自动 tick，等玩家读日志决定退出
    pub player_dead: bool,
    pub pending_move: Option<(i32, i32)>,
    pub pending_torture: bool,
    pub pending_grab: bool,
    pub pending_drop: bool,
    pub pending_chop: bool,
    pub pending_mine: bool,
    pub pending_eat: bool,
    pub pending_break_wall: bool,
    pub force_step: bool,
    pub food_data: crate::data::FoodMap,
    /// 0=角色 1=双手 2=营地
    pub side_panel_tab: u8,
    pub game_mode: GameMode,
    /// 面朝方向（移动时更新，默认朝南）
    pub facing: (i32, i32),
    pub examine: Option<ExamineState>,
    /// 动作锁定：(目标x, 目标y, 动作, 面朝该目标的方向dxdy)
    pub action_lock: Option<(i32, i32, ExamineAction, i32, i32)>,
    /// F6 调试弹窗
    pub debug_popup: Option<DebugPopup>,
    /// e 键：等待方向输入
    pub examine_dir_prompt: bool,
    /// X 聚焦目标格
    pub focused_tile: Option<(i32, i32)>,
    /// 观察面板滚动偏移（[ ] 键控制）
    pub observe_scroll: usize,
    /// 上次 narrator 输出日志时 actor 所处的地形（防止丘陵等每步刷 L1 日志；切角色时重置为 None）
    pub last_actor_terrain: Option<TerrainKind>,
    /// 制作菜单状态
    pub craft_menu: Option<CraftMenuState>,
    /// 当前天气
    pub weather: Weather,
    /// 当前天气剩余 tick（到期掷骰转移）
    pub weather_timer: u64,
    /// 闪电白闪剩余帧数（weather system 设 3，map_view 消耗）。
    /// 帧数制保证步进模式下也有短暂闪烁——不会被"停住"。
    pub lightning_flash: u8,
    /// 天气+潮湿心情 debuff 追踪：entity → 当前已应用的综合 mood 惩罚
    /// 只在状态变化时调整差额，不重复累积
    pub weather_mood_tracker: std::collections::HashMap<hecs::Entity, f32>,
    /// 雨滴粒子（持续下落动画）
    pub rain_particles: Vec<RainDrop>,
    /// 加载界面 tick 计数器
    pub loading_tick: u8,
}

/// 一个雨滴粒子：世界坐标 + 子格偏移，渲染时往下落
#[derive(Debug, Clone, Copy)]
pub struct RainDrop {
    pub wx: i32,
    pub wy: f32,   // 浮点 y，子格平滑下落
    pub speed: f32, // 每帧下落速度，随机化避免统一节奏
    pub glyph: char,
}

pub const SIDE_TAB_COUNT: u8 = 3;

impl App {
    pub fn new(terrain: &TerrainMap, actors: &ActorsConfig, food_data: FoodMap, rng: &mut impl Rng) -> Result<Self, DataError> {
        village::validate_templates();

        let gen = GameMap::generate(terrain, rng)?;
        let map = gen.map;
        let mut world = World::new();

        spawn_props(&mut world, &gen.props, rng);
        let mut spatial_init = SpatialIndex::default();
        for (e, pos) in world.query::<&Position>().iter() {
            spatial_init
                .by_tile
                .entry((pos.x, pos.y))
                .or_default()
                .push(e);
        }
        for (_e, (pos, _)) in world.query::<(&Position, &BlocksMovement)>().iter() {
            spatial_init.blockers.insert((pos.x, pos.y), true);
        }
        spawn_wolves(&mut world, &map, &spatial_init, rng);

        let mut names = actors.names.clone();
        names.shuffle(rng);

        let player_name = names.pop().unwrap_or_else(|| "流亡者".to_string());
        let c1_name = names.pop().unwrap_or_else(|| "殖民者甲".to_string());
        let c2_name = names.pop().unwrap_or_else(|| "殖民者乙".to_string());
        let captive_name = actors
            .captive_names
            .choose(rng)
            .cloned()
            .unwrap_or_else(|| "俘虏".to_string());

        let trait_tag = actors
            .traits
            .choose(rng)
            .cloned()
            .unwrap_or_else(|| "冷静".to_string());

        // 出生在营区中心
        let px = CAMP_CX;
        let py = CAMP_CY;

        let player = world.spawn((
            Position { x: px, y: py },
            Name(player_name),
            Player,
            Hands::default(),
            Health {
                hp: 100.0,
                max_hp: 100.0,
            },
            Hunger {
                value: rng_range(rng, actors.hunger_range),
            },
            Thirst {
                value: rng_range(rng, actors.thirst_range),
            },
            Energy {
                value: rng_range(rng, actors.energy_range),
            },
            Mood {
                value: rng_range(rng, actors.mood_range),
            },
            TraitTag(trait_tag),
            Wet { value: 0.0 },
            MoveCooldown { ticks: 0 },
            BodyTemp { value: 60.0 },
            Vec::<StatusEffect>::new(),
        ));

        let _c1 = world.spawn((
            Position {
                x: px - 2,
                y: py - 1,
            },
            Name(c1_name),
            Colonist,
            Health {
                hp: 100.0,
                max_hp: 100.0,
            },
            Hunger {
                value: rng_range(rng, actors.hunger_range),
            },
            Thirst {
                value: rng_range(rng, actors.thirst_range),
            },
            Energy {
                value: rng_range(rng, actors.energy_range),
            },
            Mood {
                value: rng_range(rng, actors.mood_range),
            },
            AiState {
                current: Act::Idle,
            },
            TraitTag(
                actors
                    .traits
                    .choose(rng)
                    .cloned()
                    .unwrap_or_else(|| "敏感".to_string()),
            ),
            Wet { value: 0.0 },
            MoveCooldown { ticks: 0 },
            BodyTemp { value: 60.0 },
            Vec::<StatusEffect>::new(),
        ));

        let _c2 = world.spawn((
            Position {
                x: px + 2,
                y: py + 1,
            },
            Name(c2_name),
            Colonist,
            Health {
                hp: 100.0,
                max_hp: 100.0,
            },
            Hunger {
                value: rng_range(rng, actors.hunger_range),
            },
            Thirst {
                value: rng_range(rng, actors.thirst_range),
            },
            Energy {
                value: rng_range(rng, actors.energy_range),
            },
            Mood {
                value: rng_range(rng, actors.mood_range),
            },
            AiState {
                current: Act::Idle,
            },
            TraitTag(
                actors
                    .traits
                    .choose(rng)
                    .cloned()
                    .unwrap_or_else(|| "冲动".to_string()),
            ),
            Wet { value: 0.0 },
            MoveCooldown { ticks: 0 },
            BodyTemp { value: 60.0 },
            Vec::<StatusEffect>::new(),
        ));

        let _captive = world.spawn((
            Position {
                x: px + 1,
                y: py - 1,
            },
            Name(captive_name),
            Captive { will: 80.0 },
            Health {
                hp: 70.0,
                max_hp: 100.0,
            },
            Hunger { value: 50.0 },
            Thirst { value: 50.0 },
            Energy { value: 50.0 },
            Mood { value: 20.0 },
            Wet { value: 0.0 },
            MoveCooldown { ticks: 0 },
            BodyTemp { value: 60.0 },
            Vec::<StatusEffect>::new(),
        ));

        // 营区篝火：夜晚的家
        world.spawn((
            Position {
                x: px - 1,
                y: py + 1,
            },
            Campfire,
            LightSource { radius: 15, brightness: 2 },
            BlocksMovement,
        ));

        let mut camera = Camera {
            x: px.saturating_sub(20),
            y: py.saturating_sub(10),
        };
        camera.follow((px, py), map.width, map.height, 48, 20);

        let mut app = Self {
            screen: Screen::MainMenu,
            menu: MainMenuState { cursor: 0 },
            build_menu: None,
            build_target: None,
            pre_build_speed: None,
            next_uid: 0,
            world,
            map,
            spatial: SpatialIndex::default(),
            spatial_dirty: true, // 开局必须重建一次
            lit_cache: RefCell::new(HashMap::new()),
            vis_cache: RefCell::new(HashMap::new()),
            vis_anchor: Cell::new(None),
            camera,
            events: EventQueue::default(),
            log: Vec::new(),
            tick: 4_000, // 4000/12000 = 33.3% = 早上8点
            day: 1,
            ticks_per_day: 12_000,
            reputation: 0,
            speed: Speed::Step,
            selected: Some(player),
            player,
            should_quit: false,
            quit_menu: false,
            player_dead: false,
            pending_move: None,
            pending_torture: false,
            pending_grab: false,
            pending_drop: false,
            pending_chop: false,
            pending_mine: false,
            pending_eat: false,
            pending_break_wall: false,
            force_step: false,
            food_data,
            side_panel_tab: 0,
            game_mode: GameMode::Adventure,
            facing: (0, 1),
            examine: None,
            action_lock: None,
            debug_popup: None,
            examine_dir_prompt: false,
            focused_tile: None,
            observe_scroll: 0,
            last_actor_terrain: None,
            craft_menu: None,
            weather: Weather::Clear,
            weather_timer: 0,
            lightning_flash: 0,
            weather_mood_tracker: std::collections::HashMap::new(),
            rain_particles: Vec::new(),
            loading_tick: 0,
        };
        // 开局随机天气
        let start_weather = Weather::random(rng);
        let dur = rng.gen_range(start_weather.duration_range().0..=start_weather.duration_range().1);
        app.weather = start_weather;
        app.weather_timer = dur;
        app.push_log(format!("当前天气：{}。", start_weather.label()));
        // 批量分配 EntityUID（App 构造前 spawn 的实体）
        {
            let mut uid = 0u64;
            let entities: Vec<hecs::Entity> = app.world.query::<&Position>().iter().map(|(e, _)| e).collect();
            for e in entities {
                let _ = app.world.insert_one(e, crate::components::EntityUID(uid));
                uid += 1;
            }
            app.next_uid = uid;
        }

        app.push_log("你醒了过来。营火旁的空地很小——外面是一整片血壤。".into());
        app.rebuild_spatial_index();
        app.map.reveal_radius(CAMP_CX, CAMP_CY, 30);

        let centers = village::village_centers(rng);
        for (cx, cy) in centers {
            let size_roll: f64 = rng.gen();
            let size = if size_roll < 0.50 {
                VillageSize::Small
            } else if size_roll < 0.85 {
                VillageSize::Medium
            } else {
                VillageSize::Large
            };
            village::spawn_village(&mut app.world, &mut app.map, &app.spatial, cx, cy, size, rng);
            app.rebuild_spatial_index();
        }

        app.rebuild_spatial_index();
        Ok(app)
    }

    pub fn push_log(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > 200 {
            let drain = self.log.len() - 200;
            self.log.drain(0..drain);
        }
    }

    /// 标记实体死亡。HP≤0 时由各系统调用。
    ///
    /// - 插入 `Dead` 组件
    /// - 移除 `BlocksMovement`（尸体不挡路）
    /// - 移除 `Fleeing`（如本来是狼）
    /// - 派发 `ActorDied` 事件供反应系统处理
    /// - 是玩家则设 `player_dead`
    ///
    /// 同一 tick 多次调用是安全的。
    pub fn kill(&mut self, entity: hecs::Entity, cause: impl Into<String>) {
        if self.world.get::<&crate::components::Dead>(entity).is_ok() {
            return;
        }
        let _ = self.world.insert_one(entity, crate::components::Dead);
        let _ = self.world.remove_one::<crate::components::BlocksMovement>(entity);
        let _ = self.world.remove_one::<crate::components::Fleeing>(entity);
        self.mark_spatial_dirty();
        let cause = cause.into();
        let name = self.entity_label(entity);
        self.events
            .push(crate::events::GameEvent::ActorDied { entity, cause: cause.clone() });
        self.push_log(format!("{}倒下了（{}）。", name, cause));
        if entity == self.player {
            self.player_dead = true;
            self.speed = crate::app::Speed::Paused;
        }
    }

    pub fn player_pos(&self) -> (i32, i32) {
        self.world
            .get::<&Position>(self.player)
            .map(|p| (p.x, p.y))
            .unwrap_or((CAMP_CX, CAMP_CY))
    }

    /// 当前行动者：来自 `selected`，若其已死或不存在则回退到活着的玩家，
    /// 再回退到活着的第一个殖民者。返回 None 表示全员死亡。
    /// 这就是行动系统（移动/吃/砍/挖等）实际操作的实体。
    pub fn actor(&self) -> Option<Entity> {
        use crate::components::{Colonist, Dead};
        let alive = |e: Entity| self.world.get::<&Dead>(e).is_err();
        if let Some(e) = self.selected {
            if alive(e) {
                return Some(e);
            }
        }
        if alive(self.player) {
            return Some(self.player);
        }
        self.world
            .query::<&Colonist>()
            .iter()
            .map(|(e, _)| e)
            .find(|&e| alive(e))
    }

    /// actor 的位置；若 actor 不存在则用 `player_pos()`
    pub fn actor_pos(&self) -> (i32, i32) {
        match self.actor() {
            Some(e) => self
                .world
                .get::<&Position>(e)
                .map(|p| (p.x, p.y))
                .unwrap_or_else(|_| self.player_pos()),
            None => self.player_pos(),
        }
    }

    /// 一天进度 0.0..1.0
    pub fn day_progress(&self) -> f32 {
        let tpd = self.ticks_per_day.max(1);
        (self.tick % tpd) as f32 / tpd as f32
    }

    /// HH:MM（一天按 24 小时显示，实际节奏是 20 现实分钟一天）
    pub fn clock_hm(&self) -> (u32, u32) {
        let p = self.day_progress();
        let total_mins = (p * 24.0 * 60.0) as u32;
        (total_mins / 60 % 24, total_mins % 60)
    }

    pub fn period_label(&self) -> &'static str {
        let p = self.day_progress();
        if !(0.25..0.80).contains(&p) {
            "夜晚"
        } else if p < 0.30 {
            "黎明"
        } else if p < 0.75 {
            "白天"
        } else {
            "黄昏"
        }
    }

    pub fn visibility_radius(&self) -> i32 {
        let base = {
            let progress = self.day_progress();
            if progress < 0.25 {
                8.0 // 夜晚前半
            } else if progress < 0.30 {
                lerp(8.0, 50.0, (progress - 0.25) / 0.05) // 黎明过渡
            } else if progress < 0.75 {
                50.0 // 白天
            } else if progress < 0.80 {
                lerp(50.0, 8.0, (progress - 0.75) / 0.05) // 黄昏过渡
            } else {
                8.0 // 夜晚后半
            }
        };
        // ── 地形视野修正：actor 脚下地形的 vis_mod + vis_flat ──
        let (ax, ay) = self.actor_pos();
        let terrain = self.map.terrain(ax, ay);
        let def = crate::data::terrain_def(terrain.key());
        let multiplied = base * self.weather.visibility_multiplier() * def.vis_mod;
        (multiplied + def.vis_flat as f32).max(1.0) as i32
    }

    /// 环境温度：50 基准 + 天气 + 湿身 + 火源 + 室内
    pub fn env_temperature(&self, x: i32, y: i32, wet_value: f32) -> f32 {
        let day = (0.25..0.80).contains(&self.day_progress());
        let weather = self.weather;
        let base = match (weather, day) {
            (Weather::Clear, true) => 20.0, (Weather::Clear, false) => -8.0,
            (Weather::Overcast, true) => 10.0, (Weather::Overcast, false) => -12.0,
            (Weather::Drizzle, true) => 5.0, (Weather::Drizzle, false) => -10.0,
            (Weather::Rain, true) => -5.0, (Weather::Rain, false) => -15.0,
            (Weather::Heavy, true) => -10.0, (Weather::Heavy, false) => -18.0,
            (Weather::Thunder, true) => -8.0, (Weather::Thunder, false) => -18.0,
        };
        let wet_penalty = if wet_value > 80.0 { -10.0 } else if wet_value > 50.0 { -5.0 } else { 0.0 };
        let fire_bonus = if self.has_fire_adjacent(x, y) { 25.0 } else { 0.0 };
        let torch_bonus = self.actor().and_then(|a| self.world.get::<&crate::components::Hands>(a).ok())
            .map(|h| {
                let has = h.left.is_some_and(|(k,_)| k == crate::components::ItemKind::Torch)
                    || h.right.is_some_and(|(k,_)| k == crate::components::ItemKind::Torch);
                if has { 8.0 } else { 0.0 }
            }).unwrap_or(0.0);
        let roof_bonus = if self.map.has_roof(x, y) { 10.0 } else { 0.0 };
        ((50.0f64 + base + wet_penalty + fire_bonus + torch_bonus + roof_bonus).clamp(0.0, 100.0)) as f32
    }

    /// 天气颜色乘数暴露给 map_view
    pub fn weather_color_mult(&self) -> (f32, f32, f32) {
        self.weather.color_multiplier()
    }

    /// 是否被篝火照亮（圆形半径）。结果缓存，火源变化时调 clear_lit_cache。
    pub fn lit_by_fire(&self, x: i32, y: i32) -> bool {
        {
            let cache = self.lit_cache.borrow();
            if let Some(&v) = cache.get(&(x, y)) {
                return v;
            }
        }
        let result = self.compute_lit_by_fire(x, y);
        self.lit_cache.borrow_mut().insert((x, y), result);
        result
    }

    fn compute_lit_by_fire(&self, x: i32, y: i32) -> bool {
        for (_e, (pos, light)) in self.world.query::<(&Position, &LightSource)>().iter() {
            if crate::world::within_radius((pos.x, pos.y), (x, y), light.radius) {
                return true;
            }
        }
        false
    }

    /// 清空火光缓存（火源 spawn/despawn/移动后调用）。
    pub fn clear_lit_cache(&self) {
        self.lit_cache.borrow_mut().clear();
    }

    /// 统一五级光照计算：max(环境光, 火源光, 手持火把光, 窗口光)，上限4
    pub fn tile_light(&self, x: i32, y: i32) -> u8 {
        // 1. 环境光（日夜）
        let ambient = match self.period_label() {
            "黎明" => {
                let p = self.day_progress();
                let t = (p / 0.10).clamp(0.0, 1.0);
                (t * 2.0) as u8
            }
            "白天" => 2,
            "黄昏" => {
                let p = self.day_progress();
                let t = ((p - 0.60) / 0.10).clamp(0.0, 1.0);
                (2.0 - t * 2.0) as u8
            }
            _ => 0, // 夜晚
        };

        // 2. 火源光（篝火/火把实体 LightSource）
        let mut fire = 0u8;
        for (_e, (pos, light)) in self.world.query::<(&Position, &LightSource)>().iter() {
            let dist = (pos.x - x).abs() + (pos.y - y).abs(); // 曼哈顿
            if dist > light.radius {
                continue;
            }
            let level = if dist <= light.radius / 3 {
                light.brightness
            } else {
                light.brightness.saturating_sub(1).max(1)
            };
            fire = fire.max(level);
        }

        // 3. 手持火把的移动光源（曼哈顿半径5，亮度1级）
        let mut hand_fire = 0u8;
        const TORCH_RADIUS: i32 = 5;
        for (_e, (pos, hands)) in self.world.query::<(&Position, &Hands)>().iter() {
            let has_torch = hands.left.is_some_and(|(k, _)| k == ItemKind::Torch)
                || hands.right.is_some_and(|(k, _)| k == ItemKind::Torch);
            if !has_torch {
                continue;
            }
            let dist = (pos.x - x).abs() + (pos.y - y).abs();
            if dist <= TORCH_RADIUS {
                hand_fire = hand_fire.max(1);
            }
        }

        // 4. 窗口光
        let window = if self.map.window_light_at(x, y) > 0 { 1 } else { 0 };

        ambient.max(fire).max(hand_fire).max(window).min(4)
    }

    /// 检查 (x,y) 的曼哈顿邻格（dist=1）是否有 LightSource 实体（篝火等）
    pub fn has_fire_adjacent(&self, x: i32, y: i32) -> bool {
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x + dx;
                let ny = y + dy;
                if let Some(v) = self.spatial.by_tile.get(&(nx, ny)) {
                    for &e in v {
                        if self.world.get::<&LightSource>(e).is_ok() {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// actor 脚下的光照等级
    pub fn actor_light(&self) -> LightLevel {
        let (x, y) = self.actor_pos();
        LightLevel::from_u8(self.tile_light(x, y))
    }

    pub fn can_see_tile(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        // 视线缓存：from 不变时跨帧命中
        if self.vis_anchor.get() != Some(from) {
            self.vis_anchor.set(Some(from));
            self.vis_cache.borrow_mut().clear();
        }
        {
            let cache = self.vis_cache.borrow();
            if let Some(&v) = cache.get(&to) {
                return v;
            }
        }
        let result = self.compute_can_see(from, to);
        self.vis_cache.borrow_mut().insert(to, result);
        result
    }

    fn compute_can_see(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        if self.lit_by_fire(to.0, to.1) {
            return true;
        }
        if self.map.has_roof(to.0, to.1) {
            let is_daytime = (0.25..0.80).contains(&self.day_progress());
            if is_daytime && self.map.window_light_at(to.0, to.1) > 0 {
                return true;
            }
            return false;
        }
        // 距离检查
        if !crate::world::within_radius(from, to, self.visibility_radius()) {
            return false;
        }
        // 视线穿透：from→to 路径上不能有 BlocksVision 实体（墙/门）
        !self.line_blocked_by_vision(from, to)
    }

    pub fn can_see_entity(&self, entity: hecs::Entity) -> bool {
        let (ax, ay) = self.actor_pos();
        if let Ok(pos) = self.world.get::<&crate::components::Position>(entity) {
            self.can_see_tile((ax, ay), (pos.x, pos.y))
        } else {
            false
        }
    }

    /// 检查 from→to 直线路径上是否有 BlocksVision 实体阻挡视线
    fn line_blocked_by_vision(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        let (x0, y0) = from;
        let (x1, y1) = to;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut x = x0;
        let mut y = y0;
        loop {
            // 跳过起点和终点本身
            if (x != x0 || y != y0) && (x != x1 || y != y1)
                && self.spatial.vision_blockers.contains(&(x, y))
            {
                return true;
            }
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
        false
    }

    pub fn entity_label(&self, entity: hecs::Entity) -> String {
        if self.actor() == Some(entity) {
            return "你".into();
        }
        self.world
            .get::<&crate::components::Name>(entity)
            .map(|n| n.0.clone())
            .unwrap_or_else(|_| "?".into())
    }

    pub fn visible_or_generic(&self, entity: hecs::Entity, generic: &str) -> String {
        if self.can_see_entity(entity) {
            self.entity_label(entity)
        } else {
            generic.to_string()
        }
    }

    pub fn toggle_game_mode(&mut self) {
        self.game_mode = match self.game_mode {
            GameMode::Adventure => {
                self.push_log("（占位）营地模式尚未开放。".into());
                GameMode::Camp
            }
            GameMode::Camp => {
                self.push_log("回到冒险模式。".into());
                GameMode::Adventure
            }
        };
    }

    /// 可选角色：玩家 → 殖民者 → 俘虏
    pub fn selectable_characters(&self) -> Vec<Entity> {
        let mut out = vec![self.player];
        for (e, _) in self.world.query::<&Colonist>().iter() {
            if e != self.player {
                out.push(e);
            }
        }
        for (e, _) in self.world.query::<&Captive>().iter() {
            out.push(e);
        }
        out
    }

    pub fn cycle_character(&mut self) {
        let list = self.selectable_characters();
        if list.is_empty() {
            return;
        }
        let next = match self.selected {
            Some(cur) => {
                let idx = list.iter().position(|e| *e == cur).unwrap_or(0);
                list[(idx + 1) % list.len()]
            }
            None => list[0],
        };
        self.selected = Some(next);
        self.last_actor_terrain = None; // 切角色：重置地形记忆
        let label = self
            .world
            .get::<&Name>(next)
            .ok()
            .map(|n| n.0.clone());
        if let Some(label) = label {
            self.push_log(format!("选中：{}。", label));
        }
    }

    pub fn select_character_slot(&mut self, slot: u8) {
        if !(1..=4).contains(&slot) {
            return;
        }
        let list = self.selectable_characters();
        let idx = (slot - 1) as usize;
        if let Some(entity) = list.get(idx) {
            self.selected = Some(*entity);
            self.last_actor_terrain = None; // 切角色：重置地形记忆
            let label = self
                .world
                .get::<&Name>(*entity)
                .ok()
                .map(|n| n.0.clone());
            if let Some(label) = label {
                self.push_log(format!("选中：{}。", label));
            }
        }
    }

    pub fn occupied(&self, x: i32, y: i32) -> Option<Entity> {
        if let Some(v) = self.spatial.by_tile.get(&(x, y)) {
            // 优先返回有名字的角色（狼/殖民者/俘虏等），避免 Pile 遮盖角色导致碰撞/攻击漏检
            for &e in v {
                if self.world.get::<&crate::components::Name>(e).is_ok() {
                    return Some(e);
                }
            }
            v.first().copied()
        } else {
            None
        }
    }

    /// 地形不可走 / 有 BlocksMovement 的实体
    pub fn is_blocked(&self, x: i32, y: i32) -> bool {
        if !self.map.is_walkable(x, y) {
            return true;
        }
        self.spatial.blockers.get(&(x, y)).copied().unwrap_or(false)
    }

    /// 该格是否有会挡住「角色」的东西（BlocksMovement 或 有名字的角色）
    pub fn actor_or_blocker_at(&self, x: i32, y: i32) -> Option<Entity> {
        if let Some(e) = self.spatial.blockers.get(&(x, y)).copied() {
            if e {
                // 找一个 BlocksMovement 实体
                if let Some(v) = self.spatial.by_tile.get(&(x, y)) {
                    for &e in v {
                        if self.world.get::<&BlocksMovement>(e).is_ok() {
                            return Some(e);
                        }
                    }
                }
            }
        }
        if let Some(v) = self.spatial.by_tile.get(&(x, y)) {
            for &e in v {
                if self.world.get::<&Name>(e).is_ok() {
                    return Some(e);
                }
            }
        }
        None
    }

    /// 每 tick 调一次：把 world 当前位置 + 阻挡关系复制到空间索引。
    /// 取代原来 N 次全表扫描。
    pub fn rebuild_spatial_index(&mut self) {
        if !self.spatial_dirty {
            return;
        }
        self.spatial.by_tile.clear();
        self.spatial.blockers.clear();
        self.spatial.vision_blockers.clear();
        for (e, pos) in self.world.query::<&Position>().iter() {
            self.spatial
                .by_tile
                .entry((pos.x, pos.y))
                .or_default()
                .push(e);
        }
        for (_e, (pos, _)) in self.world.query::<(&Position, &BlocksMovement)>().iter() {
            self.spatial.blockers.insert((pos.x, pos.y), true);
        }
        for (_e, (pos, _)) in self.world.query::<(&Position, &BlocksVision)>().iter() {
            self.spatial.vision_blockers.insert((pos.x, pos.y));
        }
        self.spatial_dirty = false;
    }

    /// 标记空间索引已过期（spawn/despawn/移动后用）。
    /// 下一 tick 开头会按需重建。同时清空视线/火光缓存（可能改了墙/火源）。
    pub fn mark_spatial_dirty(&mut self) {
        self.spatial_dirty = true;
        self.vis_cache.borrow_mut().clear();
        self.lit_cache.borrow_mut().clear();
    }

    pub fn spawn_settlement(&mut self, size: VillageSize, rng: &mut impl Rng) {
        let (px, py) = self.player_pos();
        let cx = px + self.facing.0 * 15;
        let cy = py + self.facing.1 * 15;
        let cx = cx.clamp(5, crate::world::MAP_WIDTH - 5);
        let cy = cy.clamp(5, crate::world::MAP_HEIGHT - 5);
        village::spawn_village(&mut self.world, &mut self.map, &self.spatial, cx, cy, size, rng);
        self.rebuild_spatial_index();
        let label = size.label();
        self.push_log(format!("（调试）面前生成了{}。", label));
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn spawn_props(world: &mut World, props: &[crate::world::PropSpawn], rng: &mut impl Rng) {
    for p in props {
        match p.kind {
            PropKind::Tree => {
                world.spawn((
                    Position { x: p.x, y: p.y },
                    Tree,
                    BlocksMovement,
                    BlocksVision,
                    Harvestable {
                        hp: 1000.0,
                        max_hp: 1000.0,
                        yield_item: ItemKind::Wood,
                        yield_hp_step: 100.0,
                    },
                ));
            }
            PropKind::Boulder => {
                world.spawn((
                    Position { x: p.x, y: p.y },
                    Boulder,
                    BlocksMovement,
                    BlocksVision,
                    Harvestable {
                        hp: 2000.0,
                        max_hp: 2000.0,
                        yield_item: ItemKind::BigStone,
                        yield_hp_step: 100.0,
                    },
                ));
            }
            PropKind::Bramble => {
                // 荆棘藤：采摘产出藤条，25% 扎手
                world.spawn((
                    Position { x: p.x, y: p.y },
                    Bush {
                        state: BushState::Fruiting,
                        growth_timer: 0,
                        yield_item: ItemKind::Vine,
                    },
                ));
            }
            PropKind::HerbPlant => {
                // 草药植株，采摘直接得 Herb
                world.spawn((
                    Position { x: p.x, y: p.y },
                    Bush {
                        state: BushState::Fruiting,
                        growth_timer: 0,
                        yield_item: ItemKind::Herb,
                    },
                ));
            }
            PropKind::Bush => {
                let fruiting = rng.gen_bool(0.35);
                let (state, timer) = if fruiting {
                    (BushState::Fruiting, 0)
                } else if rng.gen_bool(0.5) {
                    (BushState::Growing, rng.gen_range(0..360))
                } else {
                    (BushState::None, rng.gen_range(0..600))
                };
                world.spawn((
                    Position { x: p.x, y: p.y },
                    Bush {
                        state,
                        growth_timer: timer,
                        yield_item: ItemKind::Berry,
                    },
                ));
            }
            PropKind::Stick => {
                let mut pile = Pile::default();
                pile.add(ItemKind::Stick, 1);
                world.spawn((Position { x: p.x, y: p.y }, pile));
            }
            PropKind::SmallStone => {
                let mut pile = Pile::default();
                pile.add(ItemKind::SmallStone, 1);
                world.spawn((Position { x: p.x, y: p.y }, pile));
            }
            PropKind::Reed => {
                // 芦苇：可采摘产出草药
                world.spawn((
                    Position { x: p.x, y: p.y },
                    Bush {
                        state: BushState::Fruiting,
                        growth_timer: 0,
                        yield_item: ItemKind::Herb,
                    },
                ));
            }
            PropKind::PoisonMush => {
                // 毒蘑菇：可采摘，v1 不可食用
                world.spawn((
                    Position { x: p.x, y: p.y },
                    Bush {
                        state: BushState::Fruiting,
                        growth_timer: 0,
                        yield_item: ItemKind::PoisonMush,
                    },
                ));
            }
            PropKind::MetalVein => {
                // 金属矿脉：可挖矿产出金属矿
                world.spawn((
                    Position { x: p.x, y: p.y },
                    Boulder,
                    BlocksMovement,
                    Harvestable {
                        hp: 2000.0,
                        max_hp: 2000.0,
                        yield_item: ItemKind::MetalOre,
                        yield_hp_step: 200.0,
                    },
                ));
            }
            PropKind::WolfDen => {
                world.spawn((
                    Position { x: p.x, y: p.y },
                    WolfDen,
                ));
            }
        }
    }
}

fn rng_range(rng: &mut impl Rng, range: (f32, f32)) -> f32 {
    if range.0 >= range.1 {
        range.0
    } else {
        rng.gen_range(range.0..range.1)
    }
}

fn spawn_wolves(world: &mut World, map: &GameMap, spatial: &SpatialIndex, rng: &mut impl Rng) {
    let pack_count = 4;
    let min_camp_dist: i32 = 30;
    let min_pack_gap: i32 = 40;
    let max_attempts = 500;

    let mut pack_centers: Vec<(i32, i32)> = Vec::new();

    for _ in 0..max_attempts {
        if pack_centers.len() >= pack_count {
            break;
        }
        let cx = rng.gen_range(0..crate::world::MAP_WIDTH);
        let cy = rng.gen_range(0..crate::world::MAP_HEIGHT);
        if (cx - CAMP_CX).abs().max((cy - CAMP_CY).abs()) < min_camp_dist {
            continue;
        }
        if !map.is_walkable(cx, cy) {
            continue;
        }
        let too_close = pack_centers.iter().any(|(px, py)| {
            (cx - px).abs().max((cy - py).abs()) < min_pack_gap
        });
        if too_close {
            continue;
        }
        pack_centers.push((cx, cy));
    }

    for (cx, cy) in pack_centers {
        let size: u32 = match () {
            _ if rng.gen_bool(0.50) => 1,
            _ if rng.gen_bool(0.35 / 0.50) => 2, // conditional probability
            _ => 3,
        };
        let mut spawned = 0u32;
        for _ in 0..50 {
            if spawned >= size {
                break;
            }
            let dx = rng.gen_range(-2..=2);
            let dy = rng.gen_range(-2..=2);
            let x = cx + dx;
            let y = cy + dy;
            if !map.in_bounds(x, y) || !map.is_walkable(x, y) {
                continue;
            }
            if spatial.by_tile.contains_key(&(x, y)) {
                continue;
            }
            world.spawn((
                Position { x, y },
                Name("狼".into()),
                Hostile,
                Health {
                    hp: 50.0,
                    max_hp: 50.0,
                },
                Wet { value: 0.0 },
                MoveCooldown { ticks: 0 },
            ));
            spawned += 1;
        }
    }
}
