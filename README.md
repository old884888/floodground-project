# 血壤 · Bloodsoil

> "你建立在尸骨上的家园，终将成为你的坟墓。"

## 一句话

文字为核心的殖民地模拟游戏。你是一个流亡群体的"看不见的操控者"，在古文明遗弃的大陆上从零建家。表面是砍树种地打野人——**内核是对人性的全面压榨，以及系统对你干的事的无情报复。**

## 技术栈

| 层 | 选型 | 理由 |
|----|------|------|
| ECS | Rust + hecs | 轻量 ECS，单 crate 零依赖，编译秒过 |
| UI | ratatui + crossterm | 纯终端渲染，中文支持好 |
| 数据 | 外部 `.ron` 文件 + serde | 调数值不改代码，Mod 支持天然就绪 |
| 存档 | serde + 自定义格式 (未来做) | 数据结构先行，序列化后补 |

## 核心设计哲学

1. **系统不审判玩家，但会反应。** 你做残忍操作→涨残忍度→系统有对应的世界反馈。
2. **低耦合。** 所有系统只通过事件总线对话，不互相读写内部数据。加新系统不用改旧的。
3. **数据驱动。** 所有数值、种族、物品、建筑定义走外部文件，Rust 只管逻辑。
4. **文本是产品。** 三层叙事管线（即时日志 + 辐射影响 + 记忆沉淀），模板引擎先行。

## 当前实现进度

```
✅ Planmini1   骨架 MVP（地图/物品/砍挖采/饥渴/调试）
✅ Planmini2   狼 AI（追/逃/咬/群生成）
✅ Planmini3   翻垃圾（Pile系统/e两段式/侧栏Tab/F6调试）
✅ Plan 04     村庄（CDDA模板/室内黑暗/门窗床容器交互）
✅ Plan 05     制作系统（旧石器工具链/半成品续作/光照五级/主菜单）
⬜ 更多敌人    ⬜ 水源+喝水    ⬜ 存档    ⬜ 建造系统
```

## 文件夹结构

```
血壤/
├── README.md               ← 本文件
├── scripts/                ← 启动脚本
│   ├── 启动血壤.bat
│   └── 启动血壤.ps1
├── docs/                   ← 文档总目录
│   ├── design/             ← 游戏设计
│   │   ├── 游戏介绍.md
│   │   ├── 世界观.md
│   │   ├── 架构设计.md
│   │   └── 设计约束.md
│   ├── dev/                ← 开发协作
│   │   ├── 对话纪要.md
│   │   └── AI约束.md
│   └── plans/              ← 开发计划
│       └── 归档-已完成计划.md  ← 6个已完成Plan的设计摘要
├── assets/data/            ← .ron 游戏数据
│   ├── terrain.ron
│   ├── food.ron
│   └── actors.ron
└── src/                    ← Rust 源码
    ├── main.rs
    ├── app.rs
    ├── components.rs
    ├── world.rs
    ├── events.rs
    ├── data.rs
    ├── items.rs
    ├── entity_kind.rs
    ├── narrative.rs
    ├── desc.rs
    ├── village.rs
    ├── systems/            ← 13 个子系统
    │   ├── crafting.rs     ← 制作系统
    │   ├── input.rs / movement.rs / combat.rs / ...
    └── ui/                 ← ratatui 终端 UI
        ├── menu.rs         ← 主菜单
        ├── craft_menu.rs   ← 制作弹窗
        ├── map_view.rs / side_panel.rs / ...
```
