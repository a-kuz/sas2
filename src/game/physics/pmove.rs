use crate::game::constants::{CROUCH_SPEED_MULT, HASTE_JUMP_MULT, HASTE_SPEED_MULT};
use crate::game::map::Map;
use crate::game::physics::tile_collision;

#[derive(Clone, Debug)]
pub struct PmoveState {
    pub x: f32,
    pub y: f32,
    pub vel_x: f32,
    pub vel_y: f32,
    pub was_in_air: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct PmoveCmd {
    pub move_right: f32,
    pub jump: bool,
    pub crouch: bool,
    pub haste_active: bool,
}

#[derive(Clone, Debug)]
pub struct PmoveResult {
    pub new_x: f32,
    pub new_y: f32,
    pub new_vel_x: f32,
    pub new_vel_y: f32,
    pub new_was_in_air: bool,
    pub jumped: bool,
    pub landed: bool,
    pub hit_jumppad: bool,
}

const MAX_SPEED_GROUND_TICK: f32 = 5.0;
const MAX_SPEED_AIR_TICK: f32 = 6.0;
const GROUND_ACCEL_TICK: f32 = 0.35;
const AIR_ACCEL_TICK: f32 = 0.35;
const GRAVITY_TICK: f32 = 0.056;
const JUMP_FORCE_TICK: f32 = 2.9;
const MAX_FALL_SPEED_TICK: f32 = 5.0;

fn tick_to_per_sec(v: f32) -> f32 {
    v * 60.0
}

pub fn pmove(state: &PmoveState, cmd: &PmoveCmd, dt: f32, map: &mut Map) -> PmoveResult {
    let dt_clamped = dt.min(0.05).max(0.0);
    let dt_norm = dt_clamped * 60.0;

    let mut x = state.x;
    let mut y = state.y;
    let mut vel_x = state.vel_x;
    let mut vel_y = state.vel_y;

    let on_ground = tile_collision::check_on_ground(x, y, map) && vel_y <= 0.0;

    let base_max_speed = if cmd.crouch {
        tick_to_per_sec(MAX_SPEED_GROUND_TICK * CROUCH_SPEED_MULT)
    } else {
        tick_to_per_sec(MAX_SPEED_GROUND_TICK)
    };
    let max_speed = if cmd.haste_active {
        base_max_speed * HASTE_SPEED_MULT
    } else {
        base_max_speed
    };

    let accel_tick = if on_ground {
        GROUND_ACCEL_TICK
    } else {
        AIR_ACCEL_TICK
    };
    let change_dir_accel_tick = accel_tick * 2.3;
    let accel_step = accel_tick * dt_norm * 60.0;
    let change_dir_step = change_dir_accel_tick * dt_norm * 60.0;

    if cmd.move_right < -0.01 {
        if vel_x > 0.0 {
            vel_x -= change_dir_step;
        }
        if vel_x > -max_speed {
            vel_x -= accel_step;
        }
        if vel_x < -max_speed {
            vel_x = -max_speed;
        }
    } else if cmd.move_right > 0.01 {
        if vel_x < 0.0 {
            vel_x += change_dir_step;
        }
        if vel_x < max_speed {
            vel_x += accel_step;
        }
        if vel_x > max_speed {
            vel_x = max_speed;
        }
    }

    let mut jumped = false;
    if cmd.jump && on_ground && vel_y >= -tick_to_per_sec(0.5) {
        let jump_force = if cmd.haste_active {
            tick_to_per_sec(JUMP_FORCE_TICK * HASTE_JUMP_MULT)
        } else {
            tick_to_per_sec(JUMP_FORCE_TICK)
        };
        vel_y = jump_force;
        jumped = true;
    }

    vel_y -= tick_to_per_sec(GRAVITY_TICK) * dt_norm;

    if vel_y > 0.0 && vel_y < tick_to_per_sec(1.0) {
        vel_y /= 1.0 + (0.11 * dt_norm);
    }
    if vel_y < 0.0 && vel_y > -tick_to_per_sec(5.0) {
        vel_y *= 1.0 + (0.1 * dt_norm);
    }

    if cmd.move_right.abs() < 0.01 {
        if vel_x.abs() > 0.01 {
            if on_ground {
                vel_x /= 1.0 + (0.14 * dt_norm);
            } else {
                vel_x /= 1.0 + (0.025 * dt_norm);
            }
            if vel_x.abs() < 0.01 {
                vel_x = 0.0;
            }
        }
    }

    let max_fall = tick_to_per_sec(MAX_FALL_SPEED_TICK);
    if vel_y < -max_fall {
        vel_y = -max_fall;
    }
    if vel_y > tick_to_per_sec(15.0) {
        vel_y = tick_to_per_sec(15.0);
    }

    let max_air = tick_to_per_sec(MAX_SPEED_AIR_TICK);
    if vel_x.abs() > max_air {
        vel_x = vel_x.signum() * max_air;
    }

    let mut coll = tile_collision::move_with_collision(x, y, vel_x, vel_y, cmd.crouch, dt_clamped, map);

    let mut hit_jumppad = false;
    for (i, jumppad) in map.jumppads.iter_mut().enumerate() {
        let can_activate = jumppad.can_activate();
        let in_bounds = jumppad.check_collision(coll.new_x, coll.new_y);
        let vel_ok = coll.new_vel_y >= -tick_to_per_sec(0.5);
        
        let dist = ((coll.new_x - (jumppad.x + jumppad.width * 0.5)).powi(2) + 
                    (coll.new_y - (jumppad.y + 16.0)).powi(2)).sqrt();
        let near = dist < 100.0;
        
        if near || in_bounds {
            let y_diff = coll.new_y - jumppad.y;
            println!("Jumppad[{}]: pos=({:.2},{:.2}) size={:.2}, player=({:.2},{:.2}), y_diff={:.2}, dist={:.2}, can_activate={}, in_bounds={}, vel_ok={}, vel_y={:.2}", 
                i, jumppad.x, jumppad.y, jumppad.width, coll.new_x, coll.new_y, y_diff, dist, can_activate, in_bounds, vel_ok, coll.new_vel_y);
        }
        
        if can_activate && in_bounds && vel_ok {
            let force_x_per_sec = tick_to_per_sec(jumppad.force_x);
            let force_y_per_sec = tick_to_per_sec(jumppad.force_y);
            println!("Jumppad[{}] ACTIVATED: force_x={:.2}, force_y={:.2}, new_vel_y={:.2}", 
                i, jumppad.force_x, jumppad.force_y, -force_y_per_sec);
            coll.new_vel_x += force_x_per_sec;
            coll.new_vel_y = -force_y_per_sec;
            jumppad.activate();
            hit_jumppad = true;
        } else if in_bounds {
            println!("Jumppad[{}] FAILED: can_activate={}, vel_ok={}, vel_y={:.2}", 
                i, can_activate, vel_ok, coll.new_vel_y);
        }
    }

    let landed = coll.on_ground && state.was_in_air;

    x = coll.new_x;
    y = coll.new_y;
    vel_x = coll.new_vel_x;
    vel_y = coll.new_vel_y;

    PmoveResult {
        new_x: x,
        new_y: y,
        new_vel_x: vel_x,
        new_vel_y: vel_y,
        new_was_in_air: !coll.on_ground,
        jumped,
        landed,
        hit_jumppad,
    }
}

