use glam::Vec3;
use crate::game::player::Player;
use crate::game::constants::*;

pub struct CollisionResult {
    pub collided: bool,
    pub player_id: Option<u32>,
    pub position: Vec3,
}

pub fn check_sphere_player_collision(
    sphere_pos: Vec3,
    sphere_radius: f32,
    player: &Player,
) -> bool {
    if player.dead {
        return false;
    }

    let player_pos = Vec3::new(player.x, player.y, 0.0);
    let distance = (sphere_pos - player_pos).length();
    let hitbox_radius = PLAYER_HITBOX_WIDTH / 2.0;

    distance < (sphere_radius + hitbox_radius)
}

pub fn check_projectile_players_collision(
    projectile_pos: Vec3,
    projectile_radius: f32,
    shooter_id: u32,
    players: &[Player],
) -> CollisionResult {
    for player in players {
        if player.id == shooter_id || player.dead {
            continue;
        }

        if check_sphere_player_collision(projectile_pos, projectile_radius, player) {
            return CollisionResult {
                collided: true,
                player_id: Some(player.id),
                position: projectile_pos,
            };
        }
    }

    CollisionResult {
        collided: false,
        player_id: None,
        position: projectile_pos,
    }
}

pub fn check_explosion_damage(
    explosion_pos: Vec3,
    explosion_radius: f32,
    player: &Player,
) -> Option<(i32, Vec3)> {
    if player.dead {
        return None;
    }

    let player_pos = Vec3::new(player.x, player.y + 0.014285714285714285, 0.0);
    let distance = (explosion_pos - player_pos).length();

    if distance > explosion_radius {
        return None;
    }

    let damage_falloff = 1.0 - (distance / explosion_radius);
    let base_damage = DAMAGE_ROCKET;
    let damage = (base_damage as f32 * damage_falloff) as i32;

    let knockback_dir = (player_pos - explosion_pos).normalize();

    Some((damage.max(1), knockback_dir))
}

pub fn check_all_explosion_damage(
    explosion_pos: Vec3,
    explosion_radius: f32,
    shooter_id: u32,
    players: &[Player],
) -> Vec<(u32, i32, Vec3)> {
    let mut results = Vec::new();

    for player in players {
        if player.id == shooter_id {
            continue;
        }

        if let Some((damage, knockback)) = check_explosion_damage(explosion_pos, explosion_radius, player) {
            results.push((player.id, damage, knockback));
        }
    }

    results
}

pub fn check_projectile_ground_collision(
    projectile_pos: Vec3,
    ground_y: f32,
) -> bool {
    projectile_pos.y <= ground_y
}

pub fn check_line_segment_circle(
    line_start: Vec3,
    line_end: Vec3,
    circle_center: Vec3,
    circle_radius: f32,
) -> bool {
    let line_vec = line_end - line_start;
    let to_circle = circle_center - line_start;
    
    let line_length_sq = line_vec.length_squared();
    if line_length_sq == 0.0 {
        return (circle_center - line_start).length() < circle_radius;
    }

    let t = (to_circle.dot(line_vec) / line_length_sq).clamp(0.0, 1.0);
    let closest_point = line_start + line_vec * t;
    
    (circle_center - closest_point).length() < circle_radius
}
