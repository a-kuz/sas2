use serde::{Deserialize, Serialize};
use std::hash::Hash;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Map {
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<Vec<Tile>>,
    pub spawn_points: Vec<SpawnPoint>,
    pub items: Vec<Item>,
    pub jumppads: Vec<JumpPad>,
    pub teleporters: Vec<Teleporter>,
    pub lights: Vec<LightSource>,
    #[serde(default)]
    pub background_elements: Vec<BackgroundElement>,
    pub tile_width: f32,
    pub tile_height: f32,
    pub ground_y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackgroundElement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub texture_name: String,
    pub alpha: f32,
    pub additive: bool,
    pub scale: f32,
    #[serde(default)]
    pub animation_speed: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tile {
    pub solid: bool,
    pub texture_id: u16,
    #[serde(default)]
    pub shader_name: Option<String>,
    #[serde(default)]
    pub detail_texture: Option<String>,
    #[serde(default)]
    pub glow_texture: Option<String>,
    #[serde(default)]
    pub blend_alpha: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnPoint {
    pub x: f32,
    pub y: f32,
    pub team: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub x: f32,
    pub y: f32,
    pub item_type: ItemType,
    pub respawn_time: u32,
    pub active: bool,
    #[serde(default)]
    pub vel_x: f32,
    #[serde(default)]
    pub vel_y: f32,
    #[serde(default)]
    pub dropped: bool,
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub spin_yaw: f32,
    #[serde(default)]
    pub pitch: f32,
    #[serde(default)]
    pub roll: f32,
    #[serde(default)]
    pub spin_pitch: f32,
    #[serde(default)]
    pub spin_roll: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JumpPad {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub force_x: f32,
    pub force_y: f32,
    pub cooldown: u8,
}

impl JumpPad {
    pub fn update(&mut self) {
        if self.cooldown > 0 {
            self.cooldown -= 1;
        }
    }

    pub fn can_activate(&self) -> bool {
        self.cooldown == 0
    }

    pub fn activate(&mut self) {
        self.cooldown = 30;
    }

    pub fn check_collision(&self, px: f32, py: f32) -> bool {
        let in_x = px >= self.x && px <= self.x + self.width;
        let y_diff = py - self.y;
        let in_y = y_diff >= -10.0 && y_diff <= 10.0;
        in_x && in_y
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Teleporter {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub dest_x: f32,
    pub dest_y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightSource {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub intensity: f32,
    pub flicker: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    Health25,
    Health50,
    Health100,
    Armor50,
    Armor100,
    Shotgun,
    GrenadeLauncher,
    RocketLauncher,
    LightningGun,
    Railgun,
    Plasmagun,
    BFG,
    Quad,
    Regen,
    Battle,
    Flight,
    Haste,
    Invis,
}

impl Map {
    pub fn new() -> Self {
        Self {
            width: 50,
            height: 50,
            tiles: vec![
                vec![
                    Tile {
                        solid: false,
                        texture_id: 0,
                        shader_name: None,
                        detail_texture: None,
                        glow_texture: None,
                        blend_alpha: 1.0,
                    };
                    50
                ];
                50
            ],
            spawn_points: vec![],
            items: vec![],
            jumppads: vec![],
            teleporters: vec![],
            lights: vec![],
            background_elements: vec![],
            tile_width: 32.0,
            tile_height: 16.0,
            ground_y: 0.0,
        }
    }

    pub fn load_from_file(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        use super::map_loader::MapFile;
        let path = format!("maps/{}.json", name);
        let map_file = MapFile::load_from_file(&path)?;
        Ok(map_file.to_map())
    }

    #[inline]
    pub fn is_solid(&self, tile_x: i32, tile_y: i32) -> bool {
        if tile_x < 0 || tile_y < 0 || tile_x >= self.width as i32 || tile_y >= self.height as i32 {
            return true;
        }
        self.tiles[tile_x as usize][tile_y as usize].solid
    }

    #[inline]
    pub fn origin_x(&self) -> f32 {
        -(self.width as f32 * self.tile_width) * 0.5
    }

    #[inline]
    pub fn world_to_tile_x(&self, world_x: f32) -> i32 {
        let local_x = world_x - self.origin_x();
        (local_x / self.tile_width).floor() as i32
    }

    #[inline]
    pub fn world_to_tile_y(&self, world_y: f32) -> i32 {
        let from_bottom = (world_y / self.tile_height).floor() as i32;
        (self.height as i32 - 1) - from_bottom
    }

    #[inline]
    pub fn is_solid_world(&self, world_x: f32, world_y: f32) -> bool {
        self.is_solid(self.world_to_tile_x(world_x), self.world_to_tile_y(world_y))
    }

    pub fn map_width(&self) -> usize {
        self.width
    }

    pub fn map_height(&self) -> usize {
        self.height
    }

    pub fn find_safe_spawn_position(&self) -> (f32, f32) {
        if !self.spawn_points.is_empty() {
            let sp = &self.spawn_points[0];
            (sp.x, sp.y)
        } else {
            (0.0, 100.0)
        }
    }
}
