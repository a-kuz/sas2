use super::constants::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weapon {
    Gauntlet = 0,
    MachineGun = 1,
    Shotgun = 2,
    GrenadeLauncher = 3,
    RocketLauncher = 4,
    Lightning = 5,
    Railgun = 6,
    Plasmagun = 7,
    BFG = 8,
}

impl Weapon {
    pub fn damage(&self) -> i32 {
        match self {
            Weapon::Gauntlet => DAMAGE_GAUNTLET,
            Weapon::MachineGun => DAMAGE_MACHINEGUN,
            Weapon::Shotgun => DAMAGE_SHOTGUN,
            Weapon::GrenadeLauncher => DAMAGE_GRENADE,
            Weapon::RocketLauncher => DAMAGE_ROCKET,
            Weapon::Lightning => DAMAGE_SHAFT,
            Weapon::Railgun => DAMAGE_RAIL,
            Weapon::Plasmagun => DAMAGE_PLASMA,
            Weapon::BFG => DAMAGE_BFG,
        }
    }

    pub fn refire_time_seconds(&self) -> f32 {
        match self {
            Weapon::Gauntlet => 0.4,
            Weapon::MachineGun => 0.1,
            Weapon::Shotgun => 1.0,
            Weapon::GrenadeLauncher => 0.8,
            Weapon::RocketLauncher => 0.8,
            Weapon::Lightning => 0.05,
            Weapon::Railgun => 1.5,
            Weapon::Plasmagun => 0.1,
            Weapon::BFG => 0.2,
        }
    }

    pub fn switch_time_seconds(&self) -> f32 {
        0.45
    }

    pub fn ammo_per_shot(&self) -> u8 {
        match self {
            Weapon::Gauntlet => 0,
            Weapon::MachineGun => 1,
            Weapon::Shotgun => 1,
            Weapon::GrenadeLauncher => 1,
            Weapon::RocketLauncher => 1,
            Weapon::Lightning => 1,
            Weapon::Railgun => 1,
            Weapon::Plasmagun => 1,
            Weapon::BFG => 1,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Weapon::Gauntlet => "Gauntlet",
            Weapon::MachineGun => "Machine Gun",
            Weapon::Shotgun => "Shotgun",
            Weapon::GrenadeLauncher => "Grenade Launcher",
            Weapon::RocketLauncher => "Rocket Launcher",
            Weapon::Lightning => "Lightning Gun",
            Weapon::Railgun => "Railgun",
            Weapon::Plasmagun => "Plasma Gun",
            Weapon::BFG => "BFG10K",
        }
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Weapon::Gauntlet),
            1 => Some(Weapon::MachineGun),
            2 => Some(Weapon::Shotgun),
            3 => Some(Weapon::GrenadeLauncher),
            4 => Some(Weapon::RocketLauncher),
            5 => Some(Weapon::Lightning),
            6 => Some(Weapon::Railgun),
            7 => Some(Weapon::Plasmagun),
            8 => Some(Weapon::BFG),
            _ => None,
        }
    }

    pub fn is_projectile(&self) -> bool {
        matches!(
            self,
            Weapon::GrenadeLauncher
                | Weapon::RocketLauncher
                | Weapon::Plasmagun
                | Weapon::BFG
        )
    }

    pub fn is_hitscan(&self) -> bool {
        matches!(
            self,
            Weapon::MachineGun | Weapon::Shotgun | Weapon::Lightning | Weapon::Railgun
        )
    }
}
