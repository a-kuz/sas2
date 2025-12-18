pub mod events;

use events::AudioEvent;
use kira::{
    manager::{AudioManager, AudioManagerSettings, backend::DefaultBackend},
    sound::static_sound::{StaticSoundData, StaticSoundSettings},
    Volume,
};
use std::collections::HashMap;

pub struct AudioSystem {
    manager: AudioManager,
    sounds: HashMap<String, StaticSoundData>,
    enabled: bool,
}

impl AudioSystem {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
        
        Ok(Self {
            manager,
            sounds: HashMap::new(),
            enabled: true,
        })
    }

    pub fn load_sound(&mut self, name: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let sound_data = StaticSoundData::from_file(path)?;
        self.sounds.insert(name.to_string(), sound_data);
        Ok(())
    }

    pub fn play(&mut self, name: &str, volume: f32) {
        if !self.enabled {
            return;
        }

        if let Some(sound_data) = self.sounds.get(name) {
            let mut settings = StaticSoundSettings::default();
            settings.volume = Volume::Amplitude(volume as f64).into();
            
            let _ = self.manager.play(sound_data.clone().with_settings(settings));
        }
    }

    pub fn play_positional(&mut self, name: &str, volume: f32, x: f32, listener_x: f32) {
        if !self.enabled {
            return;
        }

        let distance = (x - listener_x).abs();
        let max_distance = 800.0;

        if distance > max_distance {
            return;
        }

        let distance_volume = 1.0 - (distance / max_distance).min(1.0);
        let final_volume = volume * distance_volume;

        if final_volume > 0.01 {
            self.play(name, final_volume);
        }
    }

    pub fn process_event(&mut self, event: &AudioEvent, listener_x: f32) {
        use crate::game::weapon::Weapon;
        use crate::game::awards::AwardType;

        match event {
            AudioEvent::WeaponFire {
                weapon,
                x,
                has_quad,
            } => {
                if *has_quad {
                    self.play("quad_fire", 0.8);
                }

                let sound_name = match weapon {
                    Weapon::Gauntlet => "gauntlet",
                    Weapon::MachineGun => "mg_fire",
                    Weapon::Shotgun => "shotgun_fire",
                    Weapon::GrenadeLauncher => "grenade_fire",
                    Weapon::RocketLauncher => "rocket_fire",
                    Weapon::Lightning => "lightning_fire",
                    Weapon::Railgun => "railgun_fire",
                    Weapon::Plasmagun => "plasma_fire",
                    Weapon::BFG => "bfg_fire",
                };
                let volume = match weapon {
                    Weapon::MachineGun => 0.3,
                    Weapon::Lightning => 0.3,
                    Weapon::Gauntlet => 0.4,
                    Weapon::Plasmagun => 0.4,
                    Weapon::Shotgun => 0.5,
                    Weapon::GrenadeLauncher => 0.5,
                    Weapon::RocketLauncher => 0.6,
                    Weapon::Railgun => 0.7,
                    Weapon::BFG => 0.8,
                };
                self.play_positional(sound_name, volume, *x, listener_x);
            }
            AudioEvent::WeaponSwitch => self.play("weapon_switch", 0.4),
            AudioEvent::Explosion { x } => {
                self.play_positional("rocket_explode", 0.7, *x, listener_x);
            }
            AudioEvent::PlayerPain { health, x, model } => {
                let sound_base = if *health < 25 {
                    "pain_25"
                } else if *health < 50 {
                    "pain_50"
                } else if *health < 75 {
                    "pain_75"
                } else {
                    "pain_100"
                };
                let sound_name = format!("{}_{}", sound_base, model);
                self.play_positional(&sound_name, 0.5, *x, listener_x);
            }
            AudioEvent::PlayerDeath { x, model } => {
                let sound_name = format!("death_{}", model);
                self.play_positional(&sound_name, 0.6, *x, listener_x);
            }
            AudioEvent::PlayerGib { x } => {
                self.play_positional("gib", 0.7, *x, listener_x);
            }
            AudioEvent::PlayerJump { x, model } => {
                let sound_name = format!("jump_{}", model);
                self.play_positional(&sound_name, 0.3, *x, listener_x);
            }
            AudioEvent::PlayerLand { x } => {
                self.play_positional("land", 0.4, *x, listener_x);
            }
            AudioEvent::PlayerHit { damage } => {
                let sound_name = if *damage >= 100 {
                    "hit_100"
                } else if *damage >= 50 {
                    "hit_75"
                } else if *damage >= 25 {
                    "hit_50"
                } else {
                    "hit_25"
                };
                self.play(sound_name, 0.5);
            }
            AudioEvent::ItemPickup { x } => {
                self.play_positional("item_pickup", 0.5, *x, listener_x);
            }
            AudioEvent::ArmorPickup { x } => {
                self.play_positional("armor_pickup", 0.5, *x, listener_x);
            }
            AudioEvent::WeaponPickup { x } => {
                self.play_positional("weapon_pickup", 0.5, *x, listener_x);
            }
            AudioEvent::PowerupPickup { x } => {
                self.play_positional("powerup_pickup", 0.6, *x, listener_x);
            }
            AudioEvent::QuadDamage => {
                self.play("quad_damage", 0.9);
            }
            AudioEvent::Award { award_type } => {
                let sound_name = match award_type {
                    AwardType::Excellent => "excellent",
                    AwardType::Impressive => "impressive",
                    AwardType::Humiliation => "humiliation",
                    AwardType::Perfect => "perfect",
                    AwardType::Accuracy => "accuracy",
                };
                self.play(sound_name, 0.8);
            }
        }
    }

    pub fn load_all_sounds(&mut self) {
        let sounds = vec![
            ("mg_fire", "q3-resources/sound/weapons/machinegun/machgf1b.wav"),
            ("shotgun_fire", "q3-resources/sound/weapons/shotgun/sshotf1b.wav"),
            ("rocket_fire", "q3-resources/sound/weapons/rocket/rocklf1a.wav"),
            ("rocket_explode", "q3-resources/sound/weapons/rocket/rocklx1a.wav"),
            ("grenade_fire", "q3-resources/sound/weapons/grenade/grenlf1a.wav"),
            ("plasma_fire", "q3-resources/sound/weapons/plasma/hyprbf1a.wav"),
            ("railgun_fire", "q3-resources/sound/weapons/railgun/railgf1a.wav"),
            ("lightning_fire", "q3-resources/sound/weapons/lightning/lg_hum.wav"),
            ("bfg_fire", "q3-resources/sound/weapons/bfg/bfg_fire.wav"),
            ("gauntlet", "q3-resources/sound/weapons/melee/fstatck.wav"),
            ("land", "q3-resources/sound/player/land1.wav"),
            ("gib", "q3-resources/sound/player/gibsplt1.wav"),
            ("weapon_switch", "q3-resources/sound/weapons/change.wav"),
            ("item_pickup", "q3-resources/sound/items/n_health.wav"),
            ("armor_pickup", "q3-resources/sound/items/s_health.wav"),
            ("weapon_pickup", "q3-resources/sound/misc/w_pkup.wav"),
            ("powerup_pickup", "q3-resources/sound/items/protect.wav"),
            ("quad_damage", "q3-resources/sound/items/quaddamage.wav"),
            ("quad_fire", "q3-resources/sound/items/quaddamage_fire.wav"),
            ("hit_25", "q3-resources/sound/feedback/hit25.wav"),
            ("hit_50", "q3-resources/sound/feedback/hit50.wav"),
            ("hit_75", "q3-resources/sound/feedback/hit75.wav"),
            ("hit_100", "q3-resources/sound/feedback/hit100.wav"),
            ("excellent", "q3-resources/sound/feedback/excellent.wav"),
            ("impressive", "q3-resources/sound/feedback/impressive.wav"),
            ("humiliation", "q3-resources/sound/feedback/humiliation.wav"),
            ("perfect", "q3-resources/sound/feedback/perfect.wav"),
            ("accuracy", "q3-resources/sound/feedback/accuracy.wav"),
        ];

        for (name, path) in sounds {
            if let Err(e) = self.load_sound(name, path) {
                eprintln!("Failed to load sound {}: {}", name, e);
            }
        }
    }
}
