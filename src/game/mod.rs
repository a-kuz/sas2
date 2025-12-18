pub mod core;
pub mod physics;
pub mod effects;
pub mod weapons;

pub mod awards;
pub mod camera;
pub mod combat;
pub mod constants;
pub mod game_state;
pub mod hitscan;
pub mod items;
pub mod lighting;
pub mod menu;
pub mod particle;
pub mod weapon;
pub mod player;
pub mod map;
pub mod map_loader;
pub mod world;

pub use core::player::PlayerState;
pub use core::camera::Camera;
pub use core::world::World;
pub use effects::lighting::{Light, LightingParams};
pub use weapons::projectile::Rocket;
