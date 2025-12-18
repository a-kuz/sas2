
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerState {
    Ground,
    Air,
    Crouching,
}

pub struct Player {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub facing_right: bool,
    pub is_moving: bool,
    pub animation_time: f32,
    pub state: PlayerState,
    pub is_crouching: bool,
    pub crouch_time: f32,
    pub jump_time: f32,
    pub aim_angle: f32,
    pub model_yaw: f32,
}

impl Player {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            facing_right: true,
            is_moving: false,
            animation_time: 0.0,
            state: PlayerState::Ground,
            is_crouching: false,
            crouch_time: 0.0,
            jump_time: 0.0,
            aim_angle: 0.0,
            model_yaw: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32, move_left: bool, move_right: bool, jump: bool, crouch: bool, ground_y: f32, aim_angle: f32) {
        let was_moving = self.is_moving;
        let was_state = self.state;
        
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

        const GRAVITY: f32 = 800.0;
        const JUMP_VELOCITY: f32 = 270.0;
        const GROUND_ACCEL: f32 = 100.0;
        const AIR_ACCEL: f32 = 10.0;
        const GROUND_FRICTION: f32 = 10.0;
        const AIR_FRICTION: f32 = 0.1;
        const MAX_SPEED: f32 = 320.0;
        const CROUCH_SPEED_MULT: f32 = 0.5;
        
        let on_ground = self.y <= ground_y && self.vy <= 0.0;
        
        if on_ground {
            self.y = ground_y;
            self.vy = 0.0;
            
            if crouch {
                self.state = PlayerState::Crouching;
                self.is_crouching = true;
                self.crouch_time += dt;
            } else {
                self.is_crouching = false;
                self.crouch_time = 0.0;
                
                if jump {
                    self.vy = JUMP_VELOCITY;
                    self.state = PlayerState::Air;
                    self.jump_time = 0.0;
                } else {
                    self.state = PlayerState::Ground;
                }
            }
        } else {
            self.state = PlayerState::Air;
            self.is_crouching = false;
            self.crouch_time = 0.0;
            self.jump_time += dt;
        }
        
        let accel = if on_ground { GROUND_ACCEL } else { AIR_ACCEL };
        let friction = if on_ground { GROUND_FRICTION } else { AIR_FRICTION };
        let speed_mult = if self.is_crouching { CROUCH_SPEED_MULT } else { 1.0 };
        
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
        self.vx = self.vx.clamp(-MAX_SPEED * speed_mult, MAX_SPEED * speed_mult);
        
        if self.vx.abs() < 0.01 {
            self.vx = 0.0;
        }
        
        if !on_ground {
            self.vy -= GRAVITY * dt;
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
    }
}
