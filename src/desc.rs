use hecs::Entity;

use crate::app::App;
use crate::components::*;
use crate::entity_kind::EntityKind;
use crate::systems::crafting::RECIPES;

pub fn describe_tile(app: &App, x: i32, y: i32) -> Vec<String> {
    let mut lines = Vec::new();

    if !app.map.in_bounds(x, y) {
        lines.push("世界的尽头。".into());
        return lines;
    }

    let visible = {
        let (px, py) = app.actor_pos();
        app.can_see_tile((px, py), (x, y))
    };

    if !visible {
        lines.push("一片漆黑，什么都看不见。".into());
        lines.push("也许你需要一个火把。".into());
        return lines;
    }

    // 地形（始终第一行）
    if let Some(tile) = app.map.tile(x, y) {
        lines.push(format!("🟫 {}", tile.terrain_id));
        let terrain_desc = describe_terrain(tile.terrain_kind);
        if !terrain_desc.is_empty() {
            lines.push(format!("  {}", terrain_desc));
        }
    }
    if app.map.has_roof(x, y) {
        lines.push("  ⬜ 有屋顶覆盖".into());
    }

    let entities: Vec<Entity> = app
        .spatial
        .by_tile
        .get(&(x, y))
        .cloned()
        .unwrap_or_default();

    if entities.is_empty() {
        return lines;
    }

    lines.push(String::new()); // 空行分隔

    for &e in &entities {
        let kind = EntityKind::classify(app, e);
        let dead = app.world.get::<&Dead>(e).is_ok();

        // 跳过 Floor（无趣的地板不占行）
        if matches!(kind, EntityKind::Floor) {
            continue;
        }

        match &kind {
            // ── Pile：展开所有 slot ──
            EntityKind::Pile(_) => {
                if let Ok(pile) = app.world.get::<&Pile>(e) {
                    for slot in &pile.slots {
                        lines.push(format!("▸ {} ×{}", slot.item.label(), slot.count));
                        lines.push(format!("  {}", describe_item(slot.item)));
                    }
                }
            }

            // ── CraftWip ──
            EntityKind::CraftWip(ri, prog) => {
                let name = RECIPES.get(*ri).map(|r| r.name).unwrap_or("?");
                let total = RECIPES.get(*ri).map(|r| r.base_progress).unwrap_or(*prog);
                lines.push(format!("▸ 半成品·{}（{}/{}）", name, prog, total));
                lines.push("  材料已消耗，只需时间和光照就能继续。".into());
            }

            // ── 有名字的生物（玩家/殖民者/俘虏/狼）──
            _ if app.world.get::<&Name>(e).is_ok() => {
                let name = app.world.get::<&Name>(e).map(|n| n.0.clone()).unwrap_or_default();
                let health_tag = if dead {
                    "死去的"
                } else if let Ok(h) = app.world.get::<&Health>(e) {
                    health_label(h.hp, h.max_hp)
                } else {
                    ""
                };
                let wet_tag = if dead {
                    ""
                } else if let Ok(wet) = app.world.get::<&Wet>(e) {
                    if wet.value > 20.0 {
                        wet.label()
                    } else {
                        ""
                    }
                } else {
                    ""
                };

                let label = if dead {
                    format!("▸ {}——已经死了。", name)
                } else {
                    let mut parts = Vec::new();
                    if !health_tag.is_empty() { parts.push(health_tag); }
                    if !wet_tag.is_empty() { parts.push(wet_tag); }
                    parts.push(name.as_str());
                    format!("▸ {}", parts.join(" "))
                };
                lines.push(label);

                if let Some(desc) = describe_entity(app, e) {
                    // 只取俏皮话那一句（不重复名字）
                    lines.push(format!("  {}", desc));
                }
            }

            // ── 其他（墙/门/窗/树/岩/篝火/床/容器等）──
            _ => {
                if let Some(desc) = describe_entity(app, e) {
                    lines.push(format!("▸ {}", desc));
                }
            }
        }
    }

    if lines.len() <= 2 {
        // 只有地形行 + 空行
        lines.push("什么也没有。有时候'没有'就是最好的消息。".into());
    }

    lines
}

/// 生命值 → 文字描述
fn health_label(hp: f32, max: f32) -> &'static str {
    if max <= 0.0 {
        return "";
    }
    let ratio = hp / max;
    if ratio > 0.75 {
        "" // 健康，不修饰
    } else if ratio > 0.50 {
        "轻伤的"
    } else if ratio > 0.25 {
        "负伤的"
    } else if ratio > 0.0 {
        "重伤的"
    } else {
        "濒死的"
    }
}

fn describe_entity(app: &App, e: Entity) -> Option<String> {
    let kind = EntityKind::classify(app, e);
    if app.world.get::<&Dead>(e).is_ok() {
        let name = app
            .world
            .get::<&Name>(e)
            .map(|n| n.0.clone())
            .unwrap_or_else(|_| "某人".into());
        return Some(format!("{}——已经死了。尸体还在，但人不在了。", name));
    }
    let desc = match &kind {
        EntityKind::Player => {
            "这就是你。一个流亡者，两手空空，站在世界的残骸上。镜子可能没有，但水面也能照——虽然你不会喜欢的。"
                .into()
        }
        EntityKind::Colonist => {
            let name = app.world.get::<&Name>(e).map(|n| n.0.clone()).unwrap_or_default();
            format!("{}——你的殖民者。不知道是运气好还是不好，反正跟着你了。看样子还没后悔，但这种事说不好。", name)
        }
        EntityKind::Captive => {
            let name = app.world.get::<&Name>(e).map(|n| n.0.clone()).unwrap_or_default();
            format!("{}——一个俘虏，眼神里写满了'你等着'和'算了'。意志这东西就像墙上的裂缝，迟早会扩大的。", name)
        }
        EntityKind::Hostile => {
            "一只灰扑扑的狼，眼睛里写满了'饿'和'你是谁'。它不打算跟你讲道理。".into()
        }
        EntityKind::HostileFleeing => {
            "一只夹着尾巴的狼——你赢了，但它会记住你的脸。".into()
        }
        EntityKind::Door(true) => {
            "门敞开着，像是在说'请进'，也像是在说'懒得关'。嘎吱声是它唯一的语言。".into()
        }
        EntityKind::Door(false) => {
            "一扇紧闭的木门，每开一次就会掉一片木屑——这是实话，不是修辞。铰链在呻吟。".into()
        }
        EntityKind::StoneWall => {
            "冷冰冰的石墙，至少比木头靠谱——前提是没人带着铁锤路过。沉默而傲慢。".into()
        }
        EntityKind::WoodWall => {
            "几根烂木头勉强拼成的墙壁，似乎一阵大风就能推倒。但它还站着，这就够了。".into()
        }
        EntityKind::Window => {
            "一小块灰蒙蒙的玻璃，能透光但不能当镜子——擦也没用。窗外是什么？窗外是另一个问题。".into()
        }
        EntityKind::Bed => {
            "一张硬邦邦的木板床。躺上去能听见脊椎的哀嚎。至少比地板强——这个标准已经很低了。".into()
        }
        EntityKind::Container => {
            "布满虫眼的储物箱，打开前最好祈祷里面不是蜘蛛窝。木头已经老得记不清自己是哪棵树了。".into()
        }
        EntityKind::Floor => {
            "踩上去会嘎吱响的木地板，年久失修——但至少不用踩泥巴了。".into()
        }
        EntityKind::DirtRoad => {
            "一条踩实的泥土路，雨水冲出了细小的沟壑。前人走过的路，总比草丛里瞎撞强。".into()
        }
        EntityKind::StoneRoad => {
            "石板铺就的路面，缝隙里长着倔强的青苔。能铺这种路的人，肯定不简单——或者曾经不简单。".into()
        }
        EntityKind::StickTrap => {
            "削尖的木棍埋在浅坑里，尖端朝上。踩上去能把脚掌刺个对穿——甭管是狼还是你自己。".into()
        }
        EntityKind::WolfDen => {
            "一个散发着腥臭味的土坑——狼的巢穴。附近总有眼睛在暗处盯着你。".into()
        }
        EntityKind::LeanTo => {
            "几根长木棍斜搭在一起，铺着厚厚的树叶——这就是旧石器时代的家。挡风遮雨，漏光漏风，但总归是家。".into()
        }
        EntityKind::PitShelter => {
            "半截在地下的庇护所。挖坑、搭架子、盖树叶——冬暖夏凉。唯一的问题是下雨天会塌。".into()
        }
        EntityKind::SmokingRack => {
            "几根木棍绑成的架子。以后能烟熏肉——现在只能晒太阳。未来可期，肉还没来。".into()
        }
        EntityKind::Bramble => {
            "一丛纠缠的荆棘藤，尖刺闪闪发亮。藤条是好东西——但得先说服它放手。".into()
        }
        EntityKind::Campfire => {
            "一团不屈的火焰，是这该死的世界里唯一温暖的谎言。烧的是木柴，暖的是希望——虽然两者都不持久。".into()
        }
        EntityKind::Tree => {
            "一棵看起来比你活得久的树。它见多了你这样的人——来了，砍了，走了，然后新的又来了。".into()
        }
        EntityKind::Boulder => {
            "一块硕大的岩石，沉默而顽固。它拒绝评论你的开采技术。".into()
        }
        EntityKind::Bush(BushState::Fruiting) => {
            "一丛挂着红莓果的灌木，像是荒野给你的小费。摘还是不摘？这不是问题——答案永远是摘。".into()
        }
        EntityKind::Bush(BushState::Growing) => {
            "一丛正在努力长莓果的灌木。别催，它已经够努力了。".into()
        }
        EntityKind::Bush(BushState::None) => {
            "一丛被薅干净的灌木，光秃秃的，像个刚被打劫完的杂货铺。".into()
        }
        EntityKind::Pile(Some(item)) => describe_item(*item),
        EntityKind::Pile(None) => "一些散落的杂物，没人认领，也没人在乎。".into(),
        EntityKind::Named(name) if !name.is_empty() => {
            format!("{}——你说不上来这是什么，但它有个名字，这本身就挺奇怪的。", name)
        }
        EntityKind::CraftWip(ri, prog) => {
            let name = crate::systems::crafting::RECIPES
                .get(*ri)
                .map(|r| r.name)
                .unwrap_or("?");
            let total = crate::systems::crafting::RECIPES
                .get(*ri)
                .map(|r| r.base_progress)
                .unwrap_or(*prog);
            format!("半成品·{}（{}/{}）——材料已经消耗，只需要时间和光照就能继续。", name, prog, total)
        }
        EntityKind::Named(_) => return None,
    };
    Some(desc)
}

fn describe_item(item: ItemKind) -> String {
    crate::data::item_def(item.key()).desc.clone()
}

/// 地形描述（观察模式）
fn describe_terrain(kind: TerrainKind) -> &'static str {
    match kind {
        TerrainKind::Grass => "你踩在松软的草地上。风从草尖溜过——这里什么都没发生过，什么都不会发生。",
        TerrainKind::LightForest => "稀疏的树木投下斑驳的影子。走起来不算费劲，但总有人在看着你。",
        TerrainKind::DenseForest => "树枝刮过你的脸——密林果然不好走。光到这里就累了，你呢？",
        TerrainKind::Hill => "脚下的坡度让你喘了口气。站高点能看得远——也能被人看得更清。",
        TerrainKind::ShallowMarsh => "泥水浸过你的鞋底。每一步都在吮吸——这地方在尝试吞掉你。",
        TerrainKind::ShallowWater => "浅水没过脚踝。涉水前行，裤脚已经湿透——但至少还站得稳。",
        TerrainKind::Sand => "松软的沙地让每一步都打滑。脚印留不了多久——风会替你抹掉。",
        TerrainKind::Water => "深水区。你不会游泳——或者说，你想试试吗？",
        TerrainKind::Dirt => "踩实的泥土，营地的味道。说不上好闻，但至少熟悉。",
    }
}
