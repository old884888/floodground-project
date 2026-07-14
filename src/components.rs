#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone)]
pub struct Name(pub String);

#[derive(Debug, Clone, Copy)]
pub struct Player;

#[derive(Debug, Clone, Copy)]
pub struct Colonist;

#[derive(Debug, Clone, Copy)]
pub struct Captive {
    pub will: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Health {
    pub hp: f32,
    pub max_hp: f32,
}

/// 死亡标记：HP ≤ 0 后由各系统插入。
/// 大多数系统应跳过此组件的实体；UI 仍可显示尸体。
#[derive(Debug, Clone, Copy)]
pub struct Dead;

#[derive(Debug, Clone, Copy)]
pub struct Hunger {
    /// 100 = 饱腹, 0 = 饿死
    pub value: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Thirst {
    /// 100 = 不渴, 0 = 渴死
    pub value: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Energy {
    /// 100 = 满, 0 = 强制睡
    pub value: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Mood {
    /// 0 = 崩溃边缘, 100 = 开心
    pub value: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Idle,
    Eating,
    Sleeping,
}

#[derive(Debug, Clone, Copy)]
pub struct AiState {
    pub current: Act,
}

#[derive(Debug, Clone)]
pub struct TraitTag(pub String);

// —— 世界实体（Planmini1）——

// —— 地形类型（Plan 07）——

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainKind {
    Grass,
    LightForest,
    DenseForest,
    Hill,
    ShallowMarsh,
    ShallowWater,
    Stream,
    Sand,
    Water,
    Dirt,
}

impl TerrainKind {
    /// 对应 terrain.ron 里的 key
    pub fn key(self) -> &'static str {
        match self {
            TerrainKind::Grass => "grass",
            TerrainKind::LightForest => "light_forest",
            TerrainKind::DenseForest => "dense_forest",
            TerrainKind::Hill => "hill",
            TerrainKind::ShallowMarsh => "shallow_marsh",
            TerrainKind::ShallowWater => "shallow_water",
            TerrainKind::Stream => "stream",
            TerrainKind::Sand => "sand",
            TerrainKind::Water => "water",
            TerrainKind::Dirt => "dirt",
        }
    }

    #[allow(dead_code)]
    pub fn from_key(s: &str) -> Option<Self> {
        Some(match s {
            "grass" => TerrainKind::Grass,
            "light_forest" => TerrainKind::LightForest,
            "dense_forest" => TerrainKind::DenseForest,
            "hill" => TerrainKind::Hill,
            "shallow_marsh" => TerrainKind::ShallowMarsh,
            "shallow_water" => TerrainKind::ShallowWater,
            "stream" => TerrainKind::Stream,
            "sand" => TerrainKind::Sand,
            "water" => TerrainKind::Water,
            "dirt" => TerrainKind::Dirt,
            _ => return None,
        })
    }

    /// 站在该地形上每 tick 自动潮湿增量
    pub fn auto_wet_rate(self) -> f32 {
        match self {
            TerrainKind::ShallowMarsh => 0.05,
            TerrainKind::ShallowWater => 0.08,
            TerrainKind::Stream => 0.06,
            TerrainKind::Water => 0.10,
            _ => 0.0,
        }
    }
}

use serde::{Deserialize, Serialize};

/// 移动冷却：移动后按地形 move_cost 设定，>0 时不响应移动
#[derive(Debug, Clone, Copy, Default)]
pub struct MoveCooldown {
    pub ticks: u32,
}

/// 狼巢穴：单格实体，周围定期刷狼
#[derive(Debug, Clone, Copy)]
pub struct WolfDen;

/// 受击闪烁：被攻击时插入，frames 每 tick 递减，归零后由 cleanup 移除
#[derive(Debug, Clone, Copy)]
pub struct HitFlash {
    pub frames: u8,
}

/// 浮动伤害数字：独立实体，挂在被攻击位置上方。frame 每 tick 递减，归零 despawn
#[derive(Debug, Clone)]
pub struct DamageNumber {
    pub text: String,
    pub frame: u8,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Tree;

#[derive(Debug, Clone, Copy)]
pub struct Boulder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BushState {
    None,
    Growing,
    Fruiting,
}

#[derive(Debug, Clone, Copy)]
pub struct Bush {
    pub state: BushState,
    pub growth_timer: u64,
    /// 采摘产出物品（普通灌木=Berry，芦苇=Herb，毒蘑菇=PoisonMush）
    pub yield_item: ItemKind,
}

#[derive(Debug, Clone, Copy)]
pub struct BlocksMovement;

#[derive(Debug, Clone, Copy)]
pub struct BlocksVision;

#[derive(Debug, Clone, Copy)]
pub struct Campfire;

#[derive(Debug, Clone, Copy)]
pub struct LightSource {
    pub radius: i32,
    /// 中心最大亮度（0-4），篝火=2，火把=1
    pub brightness: u8,
}

/// 制作进度（挂在正在制作的实体上）
#[derive(Debug, Clone)]
pub struct CraftingState {
    /// 配方在 RECIPES 数组中的索引
    pub recipe_index: usize,
    pub progress: u32,
}

/// 半成品：中断制作后掉落地面，可捡起继续
#[derive(Debug, Clone)]
pub struct CraftWip {
    pub recipe_index: usize,
    pub progress: u32,
}

/// 光照五级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LightLevel {
    /// 0 — 完全看不见
    PitchBlack = 0,
    /// 1 — 昏暗（火把/篝火边缘/黄昏/窗口余光）
    Dim = 1,
    /// 2 — 明亮（白天/篝火近旁）
    Bright = 2,
    /// 3 — 太亮了（预留）
    TooBright = 3,
    /// 4 — 亮瞎眼（预留）
    Blinding = 4,
}

impl LightLevel {
    pub fn from_u8(v: u8) -> Self {
        match v.min(4) {
            0 => LightLevel::PitchBlack,
            1 => LightLevel::Dim,
            2 => LightLevel::Bright,
            3 => LightLevel::TooBright,
            _ => LightLevel::Blinding,
        }
    }

    /// 制作速度倍率
    pub fn craft_speed_multiplier(self) -> f32 {
        match self {
            LightLevel::PitchBlack => 0.0,
            LightLevel::Dim => 0.5,
            LightLevel::Bright => 1.0,
            LightLevel::TooBright => 0.8,
            LightLevel::Blinding => 0.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LightLevel::PitchBlack => "完全看不见",
            LightLevel::Dim => "昏暗",
            LightLevel::Bright => "明亮",
            LightLevel::TooBright => "刺眼（镁光/探照灯）",
            LightLevel::Blinding => "亮瞎眼（原子弹级）",
        }
    }

    pub fn can_craft(self) -> bool {
        self.craft_speed_multiplier() > 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    // 基础资源
    Wood,
    BigStone,
    Stick,
    SmallStone,
    Berry,
    // Plan 08 基础材料
    Branch,      // 树枝
    Leaves,      // 树叶
    LongStick,   // 长木棍
    Vine,        // 藤条
    Rope,        // 绳子
    SmallFlake,  // 石片
    LargeFlake,  // 大石片
    Bone,        // 骨头
    // 旧石器工具链
    StoneKnife,
    SharpStick,
    Spear,       // 火烤矛（显示名）
    StoneAxe,
    Torch,
    // Plan 08 新工具
    StoneHammer, // 石锤
    StoneShovel, // 石铲
    StoneDrill,  // 石钻
    WoodKnife,   // 木刀
    WoodAxe,     // 木斧
    WoodShovel,  // 木铲
    WoodSpear,   // 削尖长棍
    BoneKnife,   // 骨刀
    BoneNeedle,  // 骨针
    // Plan 07：地形差异化产出
    Herb,
    Clay,
    MetalOre,
    PoisonMush,
}

impl ItemKind {
    pub fn key(self) -> &'static str {
        match self {
            ItemKind::Wood => "wood",
            ItemKind::BigStone => "big_stone",
            ItemKind::Stick => "stick",
            ItemKind::SmallStone => "small_stone",
            ItemKind::Berry => "berry",
            ItemKind::Branch => "branch",
            ItemKind::Leaves => "leaves",
            ItemKind::LongStick => "long_stick",
            ItemKind::Vine => "vine",
            ItemKind::Rope => "rope",
            ItemKind::SmallFlake => "small_flake",
            ItemKind::LargeFlake => "large_flake",
            ItemKind::Bone => "bone",
            ItemKind::StoneKnife => "stone_knife",
            ItemKind::SharpStick => "sharp_stick",
            ItemKind::Spear => "spear",
            ItemKind::StoneAxe => "stone_axe",
            ItemKind::Torch => "torch",
            ItemKind::StoneHammer => "stone_hammer",
            ItemKind::StoneShovel => "stone_shovel",
            ItemKind::StoneDrill => "stone_drill",
            ItemKind::WoodKnife => "wood_knife",
            ItemKind::WoodAxe => "wood_axe",
            ItemKind::WoodShovel => "wood_shovel",
            ItemKind::WoodSpear => "wood_spear",
            ItemKind::BoneKnife => "bone_knife",
            ItemKind::BoneNeedle => "bone_needle",
            ItemKind::Herb => "herb",
            ItemKind::Clay => "clay",
            ItemKind::MetalOre => "metal_ore",
            ItemKind::PoisonMush => "poison_mush",
        }
    }

    pub fn label(self) -> &'static str {
        &crate::data::item_def(self.key()).name
    }
}

/// 一格地面最多容纳 MAX_PILE_SLOTS 种不同物品，同种叠加
pub const MAX_PILE_SLOTS: usize = 128;

#[derive(Debug, Clone, Copy)]
pub struct PileSlot {
    pub item: ItemKind,
    pub count: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Pile {
    pub slots: Vec<PileSlot>,
}

impl Pile {
    pub fn add(&mut self, item: ItemKind, count: u32) -> bool {
        if count == 0 {
            return true;
        }
        if let Some(slot) = self.slots.iter_mut().find(|s| s.item == item) {
            slot.count = slot.count.saturating_add(count);
            return true;
        }
        if self.slots.len() >= MAX_PILE_SLOTS {
            return false;
        }
        self.slots.push(PileSlot { item, count });
        true
    }

    /// 从指定 slot 拿走 n 个；不够就全拿。返回实际数量。
    pub fn take_slot(&mut self, slot_index: usize, n: u32) -> Option<(ItemKind, u32)> {
        if slot_index >= self.slots.len() || n == 0 {
            return None;
        }
        let slot = &mut self.slots[slot_index];
        let item = slot.item;
        let took = n.min(slot.count);
        slot.count -= took;
        if slot.count == 0 {
            self.slots.swap_remove(slot_index);
        }
        Some((item, took))
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn dominant(&self) -> Option<(ItemKind, u32)> {
        self.slots
            .iter()
            .max_by_key(|s| s.count)
            .map(|s| (s.item, s.count))
    }
}

/// 两手持——没有背包；每手可叠同种
#[derive(Debug, Clone, Copy, Default)]
pub struct Hands {
    pub left: Option<(ItemKind, u32)>,
    pub right: Option<(ItemKind, u32)>,
}

impl Hands {
    pub fn is_empty(&self) -> bool {
        self.left.is_none() && self.right.is_none()
    }

    #[allow(dead_code)]
    pub fn is_full(&self) -> bool {
        self.left.is_some() && self.right.is_some()
    }

    /// 能否再塞这种物品（空手或同种可叠）
    pub fn can_take(&self, item: ItemKind) -> bool {
        match (self.left, self.right) {
            (None, _) | (_, None) => true,
            (Some((l, _)), Some((r, _))) => l == item || r == item,
        }
    }

    /// 塞进空手或叠到同种手；满且无同种返回 false
    #[allow(dead_code)]
    pub fn take(&mut self, item: ItemKind) -> bool {
        self.take_n(item, 1) == 1
    }

    pub fn take_n(&mut self, item: ItemKind, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        // 优先叠同种
        if let Some((kind, count)) = self.left.as_mut() {
            if *kind == item {
                *count = count.saturating_add(n);
                return n;
            }
        }
        if let Some((kind, count)) = self.right.as_mut() {
            if *kind == item {
                *count = count.saturating_add(n);
                return n;
            }
        }
        if self.left.is_none() {
            self.left = Some((item, n));
            return n;
        }
        if self.right.is_none() {
            self.right = Some((item, n));
            return n;
        }
        0
    }

    /// 丢右手优先，一次丢 1 个
    pub fn drop_one(&mut self) -> Option<ItemKind> {
        if let Some((kind, count)) = self.right.as_mut() {
            let item = *kind;
            *count -= 1;
            if *count == 0 {
                self.right = None;
            }
            return Some(item);
        }
        if let Some((kind, count)) = self.left.as_mut() {
            let item = *kind;
            *count -= 1;
            if *count == 0 {
                self.left = None;
            }
            return Some(item);
        }
        None
    }

    pub fn format_hand(slot: Option<(ItemKind, u32)>) -> String {
        match slot {
            None => "空".into(),
            Some((item, n)) => format!("{} ×{}", item.label(), n),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Harvestable {
    pub hp: f32,
    pub max_hp: f32,
    pub yield_item: ItemKind,
    pub yield_hp_step: f32,
}

// —— 敌人 ——

#[derive(Debug, Clone, Copy)]
pub struct Hostile;

/// 逃跑标记：狼正在逃离玩家
#[derive(Debug, Clone, Copy)]
pub struct Fleeing;

// —— 建筑（Plan 04）——

#[derive(Debug, Clone, Copy)]
pub struct Wall;

#[derive(Debug, Clone, Copy)]
pub struct WoodWall;

#[derive(Debug, Clone, Copy)]
pub struct StoneWall;

#[derive(Debug, Clone, Copy)]
pub struct Door {
    pub open: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Window;

#[derive(Debug, Clone, Copy)]
pub struct Bed;

#[derive(Debug, Clone, Copy)]
pub struct ContainerTag;

#[derive(Debug, Clone, Copy)]
pub struct Floor;

#[derive(Debug, Clone, Copy)]
pub struct DirtRoad;

#[derive(Debug, Clone, Copy)]
pub struct StoneRoad;

/// 建造进行中：实体正在建造，progress 到 total 即完成
#[derive(Debug, Clone, Copy)]
pub struct Building {
    #[allow(dead_code)]
    pub recipe_index: usize,
    pub progress: u32,
    pub total: u32,
}

/// 尖刺陷阱：生物踩上触发伤害后消失。builder 记录建造者，用于可见性判断
#[derive(Debug, Clone, Copy)]
pub struct StickTrap {
    pub builder: hecs::Entity,
}

/// Plan 08 建筑标记
#[derive(Debug, Clone, Copy)]
pub struct LeanTo;
#[derive(Debug, Clone, Copy)]
pub struct PitShelter;
#[derive(Debug, Clone, Copy)]
pub struct SmokingRack;

impl Hunger {
    pub fn clamp(&mut self) {
        self.value = self.value.clamp(0.0, 100.0);
    }
}

impl Thirst {
    pub fn clamp(&mut self) {
        self.value = self.value.clamp(0.0, 100.0);
    }
}

impl Energy {
    pub fn clamp(&mut self) {
        self.value = self.value.clamp(0.0, 100.0);
    }
}

impl Mood {
    pub fn clamp(&mut self) {
        self.value = self.value.clamp(0.0, 100.0);
    }
}

impl Captive {
    pub fn clamp(&mut self) {
        self.will = self.will.clamp(0.0, 100.0);
    }
}

/// 潮湿：0=干爽, 100=浸透。淋雨涨、烤火降。
#[derive(Debug, Clone, Copy)]
pub struct Wet {
    pub value: f32,
}

// ── Plan 09 温度 + 效果系统 ──

/// 体温：0-100, 50=最舒适。每 tick 向环境温度趋近。
#[derive(Debug, Clone, Copy)]
pub struct BodyTemp {
    pub value: f32,
}

/// 持续效果种类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    Diarrhea,   // 腹泻：口渴×1.5, 心情−10
    // 预留：
    // Bleeding,
    // Infection,
    // Poison,
}

/// 持续效果：挂在角色身上，每 tick 递减 remaining，归零自动移除
#[derive(Debug, Clone)]
pub struct StatusEffect {
    pub kind: EffectKind,
    pub remaining: u32,
}

/// 雨后水洼：临时实体，到时蒸发
#[derive(Debug, Clone, Copy)]
pub struct Puddle;

impl Wet {
    #[allow(dead_code)]
    pub fn clamp(&mut self) {
        self.value = self.value.clamp(0.0, 100.0);
    }

    /// 潮湿标签
    pub fn label(self) -> &'static str {
        if self.value <= 20.0 {
            "干爽的"
        } else if self.value <= 50.0 {
            "潮湿的"
        } else if self.value <= 80.0 {
            "湿透的"
        } else {
            "泡在水里"
        }
    }

    /// 精力衰减乘数（加在现有速率上）
    #[allow(dead_code)]
    pub fn energy_penalty(self) -> f32 {
        if self.value <= 50.0 {
            0.0
        } else if self.value <= 80.0 {
            0.5
        } else {
            1.0
        }
    }

    /// 心情惩罚
    #[allow(dead_code)]
    pub fn mood_penalty(self) -> f32 {
        if self.value <= 20.0 {
            0.0
        } else if self.value <= 50.0 {
            3.0
        } else if self.value <= 80.0 {
            8.0
        } else {
            15.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_kind_auto_wet_rate() {
        assert_eq!(TerrainKind::ShallowMarsh.auto_wet_rate(), 0.05);
        assert_eq!(TerrainKind::ShallowWater.auto_wet_rate(), 0.08);
        assert_eq!(TerrainKind::Water.auto_wet_rate(), 0.10);
        assert_eq!(TerrainKind::Grass.auto_wet_rate(), 0.0);
        assert_eq!(TerrainKind::DenseForest.auto_wet_rate(), 0.0);
    }

    #[test]
    fn item_kind_new_keys_exist() {
        assert_eq!(ItemKind::Herb.key(), "herb");
        assert_eq!(ItemKind::Clay.key(), "clay");
        assert_eq!(ItemKind::MetalOre.key(), "metal_ore");
        assert_eq!(ItemKind::PoisonMush.key(), "poison_mush");
    }
}
