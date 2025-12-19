use crate::game::constants::{PLAYER_HITBOX_HEIGHT, PLAYER_HITBOX_HEIGHT_CROUCH, PLAYER_HITBOX_WIDTH};
use crate::game::map::Map;

pub struct CollisionResult {
    pub new_x: f32,
    pub new_y: f32,
    pub new_vel_x: f32,
    pub new_vel_y: f32,
    pub on_ground: bool,
}

fn solid_at(map: &Map, x: f32, y: f32) -> bool {
    map.is_solid_world(x, y)
}

pub fn check_on_ground(x: f32, y: f32, map: &Map) -> bool {
    let half_w = PLAYER_HITBOX_WIDTH * 0.5 - 0.5;
    let probe_y = y - 0.5;
    solid_at(map, x - half_w, probe_y) || solid_at(map, x + half_w, probe_y)
}

pub fn move_with_collision(
    x: f32,
    y: f32,
    vel_x: f32,
    vel_y: f32,
    crouch: bool,
    dt: f32,
    map: &Map,
) -> CollisionResult {
    let hitbox_h = if crouch {
        PLAYER_HITBOX_HEIGHT_CROUCH
    } else {
        PLAYER_HITBOX_HEIGHT
    };
    let half_w = PLAYER_HITBOX_WIDTH * 0.5 - 0.5;

    let mut new_x = x;
    let mut new_y = y;
    let mut new_vel_x = vel_x;
    let mut new_vel_y = vel_y;

    let delta_x = vel_x * dt;
    let delta_y = vel_y * dt;

    if delta_x.abs() > 0.001 {
        let target_x = x + delta_x;
        let sample_y0 = y + 1.0;
        let sample_y1 = y + hitbox_h * 0.5;
        let sample_y2 = y + hitbox_h - 1.0;

        let x_blocked = solid_at(map, target_x - half_w, sample_y0)
            || solid_at(map, target_x + half_w, sample_y0)
            || solid_at(map, target_x - half_w, sample_y1)
            || solid_at(map, target_x + half_w, sample_y1)
            || solid_at(map, target_x - half_w, sample_y2)
            || solid_at(map, target_x + half_w, sample_y2);

        if !x_blocked {
            new_x = target_x;
        } else {
            let step_h = map.tile_height;
            let can_step = new_vel_y <= 0.5
                && solid_at(
                    map,
                    x + delta_x.signum() * (half_w + 1.0),
                    y + 1.0,
                )
                && !solid_at(
                    map,
                    x + delta_x.signum() * (half_w + 1.0),
                    y + step_h + 1.0,
                )
                && !solid_at(
                    map,
                    x + delta_x.signum() * (half_w + 1.0),
                    y + step_h + hitbox_h - 1.0,
                );

            if can_step {
                new_x = target_x;
                new_y = y + step_h;
            } else {
                new_vel_x = 0.0;
            }
        }
    }

    if delta_y.abs() > 0.001 {
        let target_y = new_y + delta_y;
        let head_y = target_y + hitbox_h;

        if delta_y < 0.0 {
            let blocked = solid_at(map, new_x - half_w, target_y - 0.5)
                || solid_at(map, new_x + half_w, target_y - 0.5);

            if blocked {
                new_vel_y = 0.0;
                let tile_h = map.tile_height.max(0.001);
                let tile_y = ((target_y - 0.5) / tile_h).floor();
                new_y = (tile_y + 1.0) * tile_h;
            } else {
                new_y = target_y;
            }
        } else {
            let blocked = solid_at(map, new_x - half_w, head_y + 0.5)
                || solid_at(map, new_x + half_w, head_y + 0.5);

            if blocked {
                new_vel_y = 0.0;
                let tile_h = map.tile_height.max(0.001);
                let tile_y = ((head_y + 0.5) / tile_h).floor();
                new_y = tile_y * tile_h - hitbox_h;
            } else {
                new_y = target_y;
            }
        }
    }

    let on_ground = check_on_ground(new_x, new_y, map) && new_vel_y <= 0.0;

    CollisionResult {
        new_x,
        new_y,
        new_vel_x,
        new_vel_y,
        on_ground,
    }
}

