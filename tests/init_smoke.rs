/// 烟雾测试：验证完整初始化管线不会 panic。
/// - 加载 terrain.ron / actors.ron / food.ron / items.ron
/// - 构造 App（地图生成 + 实体 spawn + 村庄生成）
/// - 跑 100 个 tick 不出事
use rand::thread_rng;

use bloodsoil::data::{
    init_item_registry, init_terrain_registry, load_actors, load_food, load_terrain,
};
use bloodsoil::systems::run_tick;
use bloodsoil::App;

#[test]
fn app_constructs_and_ticks_without_panic() {
    let terrain =
        load_terrain("assets/data/terrain.ron").expect("加载 terrain.ron 失败");
    init_terrain_registry(terrain.clone()).expect("注册地形失败");
    let actors =
        load_actors("assets/data/actors.ron").expect("加载 actors.ron 失败");
    let food_data =
        load_food("assets/data/food.ron").expect("加载 food.ron 失败");
    init_item_registry("assets/data/items.ron").expect("注册物品失败");

    let mut rng = thread_rng();
    let mut app =
        App::new(&terrain, &actors, food_data, &mut rng).expect("App 构造失败");

    // 基础断言
    assert!(!app.log.is_empty(), "初始化后日志不应为空");
    let entity_count = app.world.query::<&bloodsoil::components::Position>().iter().count();
    assert!(entity_count > 5,
        "世界应有若干实体，实际 {entity_count}");

    // 跑 100 tick，不做任何输入
    for _ in 0..100 {
        run_tick(&mut app, &mut rng);
    }

    // 100 tick 后世界不应崩溃
    assert!(app.tick > 0, "tick 应有推进");
}
