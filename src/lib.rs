pub mod snapshot_macro;
pub mod app_types;
pub mod app;
pub mod components;
pub mod data;
pub mod desc;
pub mod entity_kind;
pub mod events;
pub mod items;
pub mod narrative;
pub mod save;
pub mod systems;
pub mod ui;
pub mod village;
pub mod world;

// Convenience re-exports for integration tests
pub use app::App;
pub use data::{init_item_registry, init_terrain_registry, load_actors, load_food, load_terrain, DataError};
pub use systems::{run_tick, ticks_this_frame};
