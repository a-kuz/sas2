use super::map::{
    BackgroundElement, Item, ItemType, JumpPad, LightSource, Map, SpawnPoint, Teleporter, Tile,
};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapFile {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub tile_width: f32,
    pub tile_height: f32,
    pub tile_data: Vec<TileRow>,
    pub spawn_points: Vec<SpawnPointData>,
    pub items: Vec<ItemData>,
    pub jumppads: Vec<JumpPadData>,
    pub teleporters: Vec<TeleporterData>,
    pub lights: Vec<LightData>,
    #[serde(default)]
    pub background_elements: Option<Vec<BackgroundElement>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileRow {
    pub y: usize,
    pub tiles: Vec<TileData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileData {
    pub x_start: usize,
    pub x_end: usize,
    pub solid: bool,
    pub texture_id: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnPointData {
    pub tile_x: f32,
    pub tile_y: f32,
    pub team: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemData {
    pub tile_x: f32,
    pub tile_y: f32,
    pub item_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JumpPadData {
    pub tile_x: f32,
    pub tile_y: f32,
    pub width_tiles: f32,
    pub force_x: f32,
    pub force_y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeleporterData {
    pub tile_x: f32,
    pub tile_y: f32,
    pub width_tiles: f32,
    pub height_tiles: f32,
    pub dest_tile_x: f32,
    pub dest_tile_y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightData {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "default_intensity")]
    pub intensity: f32,
    #[serde(default)]
    pub flicker: bool,
}

fn default_intensity() -> f32 {
    1.0
}

impl MapFile {
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let map_file: MapFile = serde_json::from_reader(reader)?;
        Ok(map_file)
    }

    pub fn to_map(&self) -> Map {
        let origin_x = -((self.width as f32) * self.tile_width) * 0.5;
        let origin_y = (self.height as f32 - 1.0) * self.tile_height;
        let mut tiles = vec![
            vec![
                Tile {
                    solid: false,
                    texture_id: 0,
                    shader_name: None,
                    detail_texture: None,
                    glow_texture: None,
                    blend_alpha: 1.0,
                };
                self.height
            ];
            self.width
        ];

        for row in &self.tile_data {
            let y = row.y;
            if y >= self.height {
                continue;
            }
            for tile_data in &row.tiles {
                for x in tile_data.x_start..=tile_data.x_end.min(self.width - 1) {
                    tiles[x][y] = Tile {
                        solid: tile_data.solid,
                        texture_id: tile_data.texture_id,
                        shader_name: None,
                        detail_texture: None,
                        glow_texture: None,
                        blend_alpha: 1.0,
                    };
                }
            }
        }

        let spawn_points = self
            .spawn_points
            .iter()
            .map(|sp| SpawnPoint {
                x: origin_x + sp.tile_x * self.tile_width,
                y: origin_y - sp.tile_y * self.tile_height,
                team: sp.team,
            })
            .collect();

        let items = self
            .items
            .iter()
            .filter_map(|item| {
                let item_type = match item.item_type.as_str() {
                    "Health25" => ItemType::Health25,
                    "Health50" => ItemType::Health50,
                    "Health100" => ItemType::Health100,
                    "Armor50" => ItemType::Armor50,
                    "Armor100" => ItemType::Armor100,
                    "Shotgun" => ItemType::Shotgun,
                    "GrenadeLauncher" => ItemType::GrenadeLauncher,
                    "RocketLauncher" => ItemType::RocketLauncher,
                    "LightningGun" => ItemType::LightningGun,
                    "Railgun" => ItemType::Railgun,
                    "Plasmagun" => ItemType::Plasmagun,
                    "BFG" => ItemType::BFG,
                    "Quad" => ItemType::Quad,
                    "Regen" => ItemType::Regen,
                    "Battle" => ItemType::Battle,
                    "Flight" => ItemType::Flight,
                    "Haste" => ItemType::Haste,
                    "Invis" => ItemType::Invis,
                    _ => return None,
                };
                Some(Item {
                    x: origin_x + item.tile_x * self.tile_width,
                    y: origin_y - item.tile_y * self.tile_height,
                    item_type,
                    respawn_time: 0,
                    active: true,
                    vel_x: 0.0,
                    vel_y: 0.0,
                    dropped: false,
                    yaw: 0.0,
                    spin_yaw: 0.0,
                    pitch: 0.0,
                    roll: 0.0,
                    spin_pitch: 0.0,
                    spin_roll: 0.0,
                })
            })
            .collect();

        let jumppads = self
            .jumppads
            .iter()
            .map(|jp| JumpPad {
                x: origin_x + jp.tile_x * self.tile_width,
                y: origin_y - jp.tile_y * self.tile_height,
                width: jp.width_tiles * self.tile_width,
                force_x: jp.force_x,
                force_y: jp.force_y,
                cooldown: 0,
            })
            .collect();

        let teleporters = self
            .teleporters
            .iter()
            .map(|tp| Teleporter {
                x: origin_x + tp.tile_x * self.tile_width,
                y: origin_y - tp.tile_y * self.tile_height,
                width: tp.width_tiles * self.tile_width,
                height: tp.height_tiles * self.tile_height,
                dest_x: origin_x + tp.dest_tile_x * self.tile_width,
                dest_y: origin_y - tp.dest_tile_y * self.tile_height,
            })
            .collect();

        let lights = self
            .lights
            .iter()
            .map(|l| LightSource {
                x: l.x,
                y: l.y,
                radius: l.radius,
                r: l.r,
                g: l.g,
                b: l.b,
                intensity: l.intensity,
                flicker: l.flicker,
            })
            .collect();

        Map {
            width: self.width,
            height: self.height,
            tiles,
            spawn_points,
            items,
            jumppads,
            teleporters,
            lights,
            background_elements: self.background_elements.clone().unwrap_or_default(),
            tile_width: self.tile_width,
            tile_height: self.tile_height,
            ground_y: 0.0,
        }
    }
}
