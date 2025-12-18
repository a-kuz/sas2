use super::constants::*;
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

    pub fn update(&mut self, dt: f32, move_left: bool, move_right: bool, jump: bool, crouch: bool, ground_y: f32, aim_angle: f32) -> Vec<crate::audio::events::AudioEvent> {
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

        let speed_mult = if self.powerups.haste > 0 { HASTE_SPEED_MULT } else { 1.0 };
        let jump_mult = if self.powerups.haste > 0 { HASTE_JUMP_MULT } else { 1.0 };

        let gravity = GRAVITY;
        let jump_velocity = JUMP_VELOCITY * jump_mult;
        let ground_accel = GROUND_ACCEL * speed_mult;
        let air_accel = AIR_ACCEL * speed_mult;
        let ground_friction = FRICTION;
        let air_friction = AIR_FRICTION;
        let max_speed = MAX_SPEED * speed_mult;
        let crouch_speed_mult = CROUCH_SPEED_MULT;
        
        let on_ground = self.y <= ground_y && self.vy <= 0.0;
        
        if on_ground {
            self.y = ground_y;
            self.vy = 0.0;
            
            if self.was_in_air {
                self.landing_time = 0.0;
                audio_events.push(crate::audio::events::AudioEvent::PlayerLand { x: self.x });
            }
            self.was_in_air = false;
            
            if crouch {
                self.state = PlayerState::Crouching;
                self.is_crouching = true;
                self.crouch_time += dt;
            } else {
                self.is_crouching = false;
                self.crouch_time = 0.0;
                
                if jump {
                    self.vy = jump_velocity;
                    self.state = PlayerState::Air;
                    self.jump_time = 0.0;
                    self.was_in_air = true;
                    audio_events.push(crate::audio::events::AudioEvent::PlayerJump { 
                        x: self.x, 
                        model: self.model.clone() 
                    });
                } else {
                    self.state = PlayerState::Ground;
                }
            }
        } else {
            self.state = PlayerState::Air;
            self.is_crouching = false;
            self.crouch_time = 0.0;
            self.jump_time += dt;
            self.was_in_air = true;
        }
        
        self.landing_time += dt;
        
        let accel = if on_ground { ground_accel } else { air_accel };
        let friction = if on_ground { ground_friction } else { air_friction };
        let speed_mult_final = if self.is_crouching { crouch_speed_mult } else { 1.0 };
        
        if move_left && !move_right {
            self.vx -= accel * dt;
            self.is_moving = true;
        } else if move_right && !move_left {
            self.vx += accel * dt;
            self.is_moving = true;
        } else {
            self.is_moving = false;
        }
        
        self.vx -= self.vx * friction * dt;
        self.vx = self.vx.clamp(-max_speed * speed_mult_final, max_speed * speed_mult_final);
        
        if self.vx.abs() < 0.01 {
            self.vx = 0.0;
        }
        
        if !on_ground {
            self.vy -= gravity * dt;
        }
        
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        
        if self.y < ground_y {
            self.y = ground_y;
            self.vy = 0.0;
        }
        
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
