/// 标记组件（无字段）的主列表。
///
/// 添加新标记组件时只需在这里加一行，枚举变体、
/// collect 臂和 apply 臂会自动生成。
///
/// 格式：`[VariantName, snake_name, ComponentType]`
/// - VariantName: ComponentSnapshot 枚举变体名（PascalCase）
/// - snake_name: 保存时用的 key 名（不管，仅文档）
/// - ComponentType: 对应的 hecs 组件类型名
#[macro_export]
macro_rules! marker_snapshots {
    ($macro:ident) => {
        $macro! {
            [Player,        player,         Player]
            [Colonist,      colonist,       Colonist]
            [Hostile,       hostile,        Hostile]
            [Dead,          dead,           Dead]
            [Fleeing,       fleeing,        Fleeing]
            [Tree,          tree,           Tree]
            [Boulder,       boulder,        Boulder]
            [Wall,          wall,           Wall]
            [WoodWall,      wood_wall,      WoodWall]
            [StoneWall,     stone_wall,     StoneWall]
            [Window,        window,         Window]
            [Bed,           bed,            Bed]
            [ContainerTag,  container_tag,  ContainerTag]
            [Floor,         floor,          Floor]
            [DirtRoad,      dirt_road,      DirtRoad]
            [StoneRoad,     stone_road,     StoneRoad]
            [Campfire,      campfire,       Campfire]
            [WolfDen,       wolf_den,       WolfDen]
            [LeanTo,        lean_to,        LeanTo]
            [PitShelter,    pit_shelter,    PitShelter]
            [SmokingRack,   smoking_rack,   SmokingRack]
            [Puddle,        puddle,         Puddle]
        }
    };
}

/// 生成 ComponentSnapshot 枚举的标记变体（用 `,` 分隔）
#[macro_export]
macro_rules! gen_marker_variants {
    ($([$variant:ident, $snake:ident, $comp:ident]),* $(,)?) => {
        $(
            $variant,
        )*
    };
}

/// 生成 collect_components 中的标记组件收集臂
#[macro_export]
macro_rules! gen_marker_collect_arms {
    ($([$variant:ident, $snake:ident, $comp:ident]),* $(,)?) => {
        $(
            if world.get::<&comp::$comp>(e).is_ok() { out.push($variant); }
        )*
    };
}

/// 生成 apply_components 中的标记组件应用臂
#[macro_export]
macro_rules! gen_marker_apply_arms {
    ($([$variant:ident, $snake:ident, $comp:ident]),* $(,)?) => {
        $(
            $variant => { let _ = world.insert_one(e, comp::$comp); }
        )*
    };
}
