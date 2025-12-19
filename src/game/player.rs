use super::constants::*;
use super::map::Map;
use super::physics::pmove::{self, PmoveCmd, PmoveState};
use super::weapon::Weapon;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerState {
    Ground,
    Air,
    Crouching,
}

#[derive(Clone, Debug)]
pub struct PowerUps {
    pub quad: u16,
    pub regen: u16,
    pub battle: u16,
    pub flight: u16,
    pub haste: u16,
    pub invis: u16,
}

impl PowerUps {
    pub fn new() -> Self {
        Self {
            quad: 0,
            regen: 0,
            battle: 0,
            flight: 0,
            haste: 0,
            invis: 0,
        }
    }
}

pub struct Player {
    pub id: u32,
    pub name: String,
    pub model: String,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub prev_x: f32,
    pub prev_y: f32,
    pub facing_right: bool,
    pub is_moving: bool,
    pub is_moving_backward: bool,
    pub animation_time: f32,
    pub state: PlayerState,
    pub is_crouching: bool,
    pub crouch_time: f32,
    pub jump_time: f32,
    pub aim_angle: f32,
    pub model_yaw: f32,
    
    pub health: i32,
    pub armor: i32,
    pub frags: i32,
    pub deaths: i32,
    pub dead: bool,
    pub gibbed: bool,
    pub respawn_timer: f32,
    
    pub weapon: Weapon,
    pub has_weapon: [bool; 9],
    pub ammo: [u8; 9],
    pub refire: f32,
    pub weapon_switch_time: f32,
    pub weapon_raising: bool,
    pub weapon_raise_time: f32,
    
    pub powerups: PowerUps,
    
    pub lower_frame: usize,
    pub upper_frame: usize,
    pub frame_timer: f32,
    pub upper_frame_timer: f32,
    
    pub idle_time: f32,
    pub idle_yaw: f32,
    pub landing_time: f32,
    pub was_in_air: bool,
    
    pub barrel_spin_angle: f32,
    pub barrel_spin_speed: f32,
    
    pub excellent_count: u32,
    pub impressive_count: u32,
    
    pub hp_decay_timer: f32,
}

impl Player {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            name: format!("Player{}", id),
            model: "sarge".to_string(),
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            prev_x: 0.0,
            prev_y: 0.0,
            facing_right: true,
            is_moving: false,
            is_moving_backward: false,
            animation_time: 0.0,
            state: PlayerState::Ground,
            is_crouching: false,
            crouch_time: 0.0,
            jump_time: 0.0,
            aim_angle: 0.0,
            model_yaw: 0.0,
            
            health: STARTING_HEALTH,
            armor: 0,
            frags: 0,
            deaths: 0,
            dead: false,
            gibbed: false,
            respawn_timer: 0.0,
            
            weapon: Weapon::RocketLauncher,
            has_weapon: [true, true, false, false, true, false, false, false, false],
            ammo: [255, 100, 0, 0, 50, 0, 0, 0, 0],
            refire: 0.0,
            weapon_switch_time: 0.0,
            weapon_raising: false,
            weapon_raise_time: 0.0,
            
            powerups: PowerUps::new(),
            
            lower_frame: 0,
            upper_frame: 0,
            frame_timer: 0.0,
            upper_frame_timer: 0.0,
            
            idle_time: 0.0,
            idle_yaw: 0.0,
            landing_time: 0.0,
            was_in_air: false,
            
            barrel_spin_angle: 0.0,
            barrel_spin_speed: 0.0,
            
            excellent_count: 0,
            impressive_count: 0,
            
            hp_decay_timer: 0.0,
        }
    }

    pub fn spawn(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
        self.vx = 0.0;
        self.vy = 0.0;
        self.health = STARTING_HEALTH;
        self.armor = 0;
        self.dead = false;
        self.gibbed = false;
        self.respawn_timer = 0.0;
        self.weapon = Weapon::RocketLauncher;
        self.has_weapon = [true, true, false, false, true, false, false, false, false];
        self.ammo = [255, 100, 0, 0, 50, 0, 0, 0, 0];
        self.powerups = PowerUps::new();
    }

    pub fn update_timers(&mut self, dt: f32) {
        if self.dead {
            if self.respawn_timer > 0.0 {
                self.respawn_timer -= dt;
            }
            return;
        }

        if self.refire > 0.0 {
            self.refire -= dt;
            if self.refire < 0.0 {
                self.refire = 0.0;
            }
        }

        if self.weapon_switch_time > 0.0 {
            self.weapon_switch_time -= dt;
            if self.weapon_switch_time < 0.0 {
                self.weapon_switch_time = 0.0;
            }
        }

        if self.weapon_raise_time > 0.0 {
            self.weapon_raise_time -= dt;
            if self.weapon_raise_time < 0.0 {
                self.weapon_raise_time = 0.0;
                self.weapon_raising = false;
            }
        }

        if self.powerups.quad > 0 {
            self.powerups.quad = self.powerups.quad.saturating_sub(1);
        }
        if self.powerups.regen > 0 {
            self.powerups.regen = self.powerups.regen.saturating_sub(1);
            if self.powerups.regen % 10 == 0 && self.health < 200 {
                self.health += 1;
            }
        }
        if self.powerups.battle > 0 {
            self.powerups.battle = self.powerups.battle.saturating_sub(1);
        }
        if self.powerups.flight > 0 {
            self.powerups.flight = self.powerups.flight.saturating_sub(1);
        }
        if self.powerups.haste > 0 {
            self.powerups.haste = self.powerups.haste.saturating_sub(1);
        }
        if self.powerups.invis > 0 {
            self.powerups.invis = self.powerups.invis.saturating_sub(1);
        }

        if self.health > 100 {
            self.hp_decay_timer += dt;
            if self.hp_decay_timer >= 1.0 {
                self.health -= 1;
                self.hp_decay_timer = 0.0;
            }
        } else {
            self.hp_decay_timer = 0.0;
        }

        let is_moving = self.vx.abs() > 0.1;
        if is_moving {
            self.idle_time = 0.0;
            self.idle_yaw = 0.0;
        } else {
            self.idle_time += dt;
            if self.idle_time > 1.0 {
                self.idle_yaw = ((self.idle_time - 1.0) * 1.2).sin() * 0.15;
            }
        }

        if matches!(self.weapon, Weapon::MachineGun) {
            if self.barrel_spin_speed > 0.0 {
                self.barrel_spin_speed -= BARREL_SPIN_FRICTION * dt;
                if self.barrel_spin_speed < 0.0 {
                    self.barrel_spin_speed = 0.0;
                }
            }
        } else {
            self.barrel_spin_speed = 0.0;
        }
        
        if self.barrel_spin_speed > 0.0 {
            self.barrel_spin_angle += self.barrel_spin_speed * dt;
            if self.barrel_spin_angle > std::f32::consts::TAU {
                self.barrel_spin_angle -= std::f32::consts::TAU;
            }
        }
    }

    pub fn update(&mut self, dt: f32, move_left: bool, move_right: bool, jump: bool, crouch: bool, map: &mut Map, aim_angle: f32) -> Vec<crate::audio::events::AudioEvent> {
        let mut audio_events = Vec::new();
        let was_moving = self.is_moving;
        let was_state = self.state;
        
        self.prev_x = self.x;
        self.prev_y = self.y;
        
        self.aim_angle = aim_angle;
        
        let normalized_angle = if aim_angle > std::f32::consts::PI {
            aim_angle - 2.0 * std::f32::consts::PI
        } else {
            aim_angle
        };

        if normalized_angle.abs() < std::f32::consts::FRAC_PI_2 {
            self.facing_right = true;
        } else {
            self.facing_right = false;
        }

        let mut target_yaw = normalized_angle;
        
        let current_normalized = self.model_yaw;
        let mut angle_diff = target_yaw - current_normalized;
        
        while angle_diff > std::f32::consts::PI {
            angle_diff -= 2.0 * std::f32::consts::PI;
        }
        while angle_diff < -std::f32::consts::PI {
            angle_diff += 2.0 * std::f32::consts::PI;
        }
        
        if angle_diff.abs() > std::f32::consts::PI * 0.9 {
            if target_yaw > 0.0 {
                target_yaw -= 2.0 * std::f32::consts::PI;
            } else {
                target_yaw += 2.0 * std::f32::consts::PI;
            }
            angle_diff = target_yaw - current_normalized;
        }
        
        let turn_speed = 12.0;
        let max_turn = turn_speed * dt;
        let turn_amount = angle_diff.clamp(-max_turn, max_turn);
        self.model_yaw += turn_amount;
        
        while self.model_yaw > std::f32::consts::PI {
            self.model_yaw -= 2.0 * std::f32::consts::PI;
        }
        while self.model_yaw < -std::f32::consts::PI {
            self.model_yaw += 2.0 * std::f32::consts::PI;
        }

        let move_axis = match (move_left, move_right) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        };

        let state = PmoveState {
            x: self.x,
            y: self.y,
            vel_x: self.vx,
            vel_y: self.vy,
            was_in_air: self.was_in_air,
        };
        let cmd = PmoveCmd {
            move_right: move_axis,
            jump,
            crouch,
            haste_active: self.powerups.haste > 0,
        };

        let result = pmove::pmove(&state, &cmd, dt, map);

        self.x = result.new_x;
        self.y = result.new_y;
        self.vx = result.new_vel_x;
        self.vy = result.new_vel_y;

        use crate::game::constants::PLAYER_HITBOX_WIDTH;
        let half_w = PLAYER_HITBOX_WIDTH * 0.5;
        let player_left = self.x - half_w;
        let player_right = self.x + half_w;
        let player_bottom = self.y;
        let player_top = self.y + crate::game::constants::PLAYER_HITBOX_HEIGHT;
        
        for (i, teleporter) in map.teleporters.iter().enumerate() {
            let tp_left = teleporter.x;
            let tp_right = teleporter.x + teleporter.width;
            let tp_bottom = teleporter.y;
            let tp_top = teleporter.y + teleporter.height;
            
            let in_x = player_right >= tp_left && player_left <= tp_right;
            let in_y = player_top >= tp_bottom && player_bottom <= tp_top;
            
            let center_x = teleporter.x + teleporter.width * 0.5;
            let center_y = teleporter.y + teleporter.height * 0.5;
            let dist = ((self.x - center_x).powi(2) + ((self.y + 35.0) - center_y).powi(2)).sqrt();
            let near = dist < 100.0;
            
            if near || in_x || in_y {
                println!("Teleporter[{}]: pos=({:.2},{:.2}) size=({:.2},{:.2}), dest=({:.2},{:.2}), player=({:.2},{:.2}) hitbox=({:.2}-{:.2}, {:.2}-{:.2}), dist={:.2}, in_x={}, in_y={}", 
                    i, teleporter.x, teleporter.y, teleporter.width, teleporter.height, 
                    teleporter.dest_x, teleporter.dest_y, self.x, self.y, player_left, player_right, player_bottom, player_top, dist, in_x, in_y);
            }
            
            if in_x && in_y {
                println!("Teleporter[{}] ACTIVATED: ({:.2},{:.2}) -> ({:.2},{:.2})", 
                    i, self.x, self.y, teleporter.dest_x, teleporter.dest_y);
                self.x = teleporter.dest_x;
                self.y = teleporter.dest_y;
                break;
            }
        }

        self.was_in_air = result.new_was_in_air;
        self.is_crouching = crouch;

        let on_ground = !self.was_in_air;
        self.state = if on_ground {
            if crouch {
                PlayerState::Crouching
            } else {
                PlayerState::Ground
            }
        } else {
            PlayerState::Air
        };

        if crouch {
            self.crouch_time += dt;
        } else {
            self.crouch_time = 0.0;
        }

        if on_ground {
            self.jump_time = 0.0;
        } else {
            self.jump_time += dt;
        }

        if result.jumped {
            self.jump_time = 0.0;
            audio_events.push(crate::audio::events::AudioEvent::PlayerJump {
                x: self.x,
                model: self.model.clone(),
            });
        }

        if result.landed {
            self.landing_time = 0.0;
            audio_events.push(crate::audio::events::AudioEvent::PlayerLand { x: self.x });
        }

        self.landing_time += dt;

        self.is_moving = self.vx.abs() > 0.1 || (!on_ground && self.vy.abs() > 0.5);
        self.is_moving_backward = on_ground && (
            (self.facing_right && self.vx < -0.1) || 
            (!self.facing_right && self.vx > 0.1)
        );
        
        if self.is_moving != was_moving || self.state != was_state {
            self.animation_time = 0.0;
        }
        self.animation_time += dt;
        
        audio_events
    }

    pub fn damage(&mut self, amount: i32) -> bool {
        if self.dead {
            return false;
        }

        let mut damage = amount;
        
        if self.powerups.battle > 0 {
            damage = damage / 2;
        }

        if self.armor > 0 {
            let armor_save = damage / 2;
            let armor_damage = armor_save.min(self.armor);
            self.armor -= armor_damage;
            damage -= armor_damage;
        }

        self.health -= damage;

        if self.health <= 0 {
            self.health = 0;
            self.dead = true;
            self.gibbed = amount >= 100;
            self.respawn_timer = 3.0;
            return true;
        }

        false
    }

    pub fn can_fire(&self) -> bool {
        !self.dead && self.refire <= 0.0 && self.weapon_switch_time <= 0.0
    }

    pub fn switch_weapon(&mut self, weapon: Weapon) -> bool {
        if self.dead || self.weapon_switch_time > 0.0 {
            return false;
        }

        let weapon_index = weapon as usize;
        if !self.has_weapon[weapon_index] {
            return false;
        }

        if self.weapon == weapon {
            return false;
        }

        self.weapon = weapon;
        self.weapon_switch_time = 0.45;
        self.weapon_raising = true;
        self.weapon_raise_time = 0.45;
        
        true
    }

    pub fn add_ammo(&mut self, weapon: Weapon, amount: u8) {
        let weapon_index = weapon as usize;
        self.ammo[weapon_index] = self.ammo[weapon_index].saturating_add(amount);
    }

    pub fn consume_ammo(&mut self) -> bool {
        let weapon_index = self.weapon as usize;
        let cost = self.weapon.ammo_per_shot();
        
        if self.ammo[weapon_index] >= cost {
            self.ammo[weapon_index] -= cost;
            
            if matches!(self.weapon, Weapon::MachineGun) {
                self.barrel_spin_speed += BARREL_SPIN_ACCEL_IMPULSE;
                if self.barrel_spin_speed > BARREL_SPIN_MAX_SPEED {
                    self.barrel_spin_speed = BARREL_SPIN_MAX_SPEED;
                }
            }
            
            true
        } else {
            false
        }
    }

    pub fn give_weapon(&mut self, weapon: Weapon) {
        let weapon_index = weapon as usize;
        self.has_weapon[weapon_index] = true;
    }

    pub fn add_health(&mut self, amount: i32) -> bool {
        if self.health >= 200 {
            return false;
        }
        self.health = (self.health + amount).min(200);
        true
    }

    pub fn add_armor(&mut self, amount: i32) -> bool {
        if self.armor >= 200 {
            return false;
        }
        self.armor = (self.armor + amount).min(200);
        true
    }
}
