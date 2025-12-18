use glam::Vec3;
use crate::game::weapon::Weapon;
use crate::game::constants::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemType {
    Health25,
    Health50,
    HealthMega,
    ArmorShard,
    Armor,
    ArmorHeavy,
    Shotgun,
    GrenadeLauncher,
    RocketLauncher,
    LightningGun,
    Railgun,
    Plasmagun,
    BFG,
    PowerupQuad,
    PowerupRegen,
    PowerupBattle,
    PowerupFlight,
    PowerupHaste,
    PowerupInvis,
}

impl ItemType {
    pub fn respawn_time(&self) -> u32 {
        match self {
            ItemType::Health25 | ItemType::Health50 | ItemType::HealthMega => ITEM_RESPAWN_HEALTH,
            ItemType::ArmorShard | ItemType::Armor | ItemType::ArmorHeavy => ITEM_RESPAWN_ARMOR,
            ItemType::Shotgun
            | ItemType::GrenadeLauncher
            | ItemType::RocketLauncher
            | ItemType::LightningGun
            | ItemType::Railgun
            | ItemType::Plasmagun
            | ItemType::BFG => ITEM_RESPAWN_WEAPON,
            ItemType::PowerupQuad
            | ItemType::PowerupRegen
            | ItemType::PowerupBattle
            | ItemType::PowerupFlight
            | ItemType::PowerupHaste
            | ItemType::PowerupInvis => ITEM_RESPAWN_POWERUP,
        }
    }

    pub fn to_weapon(&self) -> Option<Weapon> {
        match self {
            ItemType::Shotgun => Some(Weapon::Shotgun),
            ItemType::GrenadeLauncher => Some(Weapon::GrenadeLauncher),
            ItemType::RocketLauncher => Some(Weapon::RocketLauncher),
            ItemType::LightningGun => Some(Weapon::Lightning),
            ItemType::Railgun => Some(Weapon::Railgun),
            ItemType::Plasmagun => Some(Weapon::Plasmagun),
            ItemType::BFG => Some(Weapon::BFG),
            _ => None,
        }
    }

    pub fn health_amount(&self) -> Option<i32> {
        match self {
            ItemType::Health25 => Some(25),
            ItemType::Health50 => Some(50),
            ItemType::HealthMega => Some(100),
            _ => None,
        }
    }

    pub fn armor_amount(&self) -> Option<i32> {
        match self {
            ItemType::ArmorShard => Some(5),
            ItemType::Armor => Some(50),
            ItemType::ArmorHeavy => Some(100),
            _ => None,
        }
    }

    pub fn powerup_duration(&self) -> Option<u16> {
        match self {
            ItemType::PowerupQuad => Some(POWERUP_DURATION_QUAD),
            ItemType::PowerupRegen => Some(POWERUP_DURATION_REGEN),
            ItemType::PowerupBattle => Some(POWERUP_DURATION_BATTLE),
            ItemType::PowerupFlight => Some(POWERUP_DURATION_FLIGHT),
            ItemType::PowerupHaste => Some(POWERUP_DURATION_HASTE),
            ItemType::PowerupInvis => Some(POWERUP_DURATION_INVIS),
            _ => None,
        }
    }
}

pub struct Item {
    pub position: Vec3,
    pub item_type: ItemType,
    pub active: bool,
    pub respawn_timer: u32,
    pub yaw: f32,
    pub spin_speed: f32,
}

impl Item {
    pub fn new(position: Vec3, item_type: ItemType) -> Self {
        Self {
            position,
            item_type,
            active: true,
            respawn_timer: 0,
            yaw: 0.0,
            spin_speed: 2.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if !self.active {
            if self.respawn_timer > 0 {
                self.respawn_timer -= 1;
            } else {
                self.active = true;
            }
        }

        self.yaw += self.spin_speed * dt;
        if self.yaw > std::f32::consts::TAU {
            self.yaw -= std::f32::consts::TAU;
        }
    }

    pub fn pickup(&mut self) {
        self.active = false;
        self.respawn_timer = self.item_type.respawn_time();
    }

    pub fn check_pickup(&self, player_pos: Vec3, pickup_radius: f32) -> bool {
        if !self.active {
            return false;
        }

        let distance = (self.position - player_pos).length();
        distance < pickup_radius
    }
}



