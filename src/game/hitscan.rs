use glam::Vec3;
use crate::game::weapon::Weapon;
use crate::game::player::Player;

pub struct HitResult {
    pub hit: bool,
    pub hit_player_id: Option<u32>,
    pub hit_position: Vec3,
    pub damage: i32,
}

pub struct RailBeam {
    pub start: Vec3,
    pub end: Vec3,
    pub lifetime: f32,
    pub max_lifetime: f32,
}

impl RailBeam {
    pub fn new(start: Vec3, end: Vec3) -> Self {
        Self {
            start,
            end,
            lifetime: 0.0,
            max_lifetime: 0.5,
        }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        self.lifetime += dt;
        self.lifetime < self.max_lifetime
    }
}

pub struct LightningBeam {
    pub start: Vec3,
    pub end: Vec3,
    pub lifetime: f32,
    pub max_lifetime: f32,
}

impl LightningBeam {
    pub fn new(start: Vec3, end: Vec3) -> Self {
        Self {
            start,
            end,
            lifetime: 0.0,
            max_lifetime: 0.15,
        }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        self.lifetime += dt;
        self.lifetime < self.max_lifetime
    }
}

pub fn hitscan_trace(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    shooter_id: u32,
    players: &[Player],
    weapon: Weapon,
) -> HitResult {
    let normalized_dir = direction.normalize();
    let ray_end = origin + normalized_dir * max_distance;

    let mut closest_hit: Option<(u32, Vec3, f32)> = None;

    for player in players {
        if player.id == shooter_id || player.dead {
            continue;
        }

        let player_pos = Vec3::new(player.x, player.y, 0.0);
        let to_player = player_pos - origin;
        let projection = to_player.dot(normalized_dir);

        if projection < 0.0 || projection > max_distance {
            continue;
        }

        let closest_point = origin + normalized_dir * projection;
        let distance_to_ray = (player_pos - closest_point).length();

        let hitbox_radius = 0.45714285714285713;

        if distance_to_ray < hitbox_radius {
            let dist_from_origin = projection;
            if closest_hit.is_none() || dist_from_origin < closest_hit.unwrap().2 {
                closest_hit = Some((player.id, closest_point, dist_from_origin));
            }
        }
    }

    if let Some((player_id, hit_pos, _)) = closest_hit {
        let mut damage = weapon.damage();
        
        if matches!(weapon, Weapon::Shotgun) {
            let spread_factor = rand::random::<f32>();
            damage = (damage as f32 * (0.5 + spread_factor * 0.5)) as i32;
        }

        HitResult {
            hit: true,
            hit_player_id: Some(player_id),
            hit_position: hit_pos,
            damage,
        }
    } else {
        HitResult {
            hit: false,
            hit_player_id: None,
            hit_position: ray_end,
            damage: 0,
        }
    }
}

pub fn shotgun_trace(
    origin: Vec3,
    direction: Vec3,
    shooter_id: u32,
    players: &[Player],
) -> Vec<HitResult> {
    let mut results = Vec::new();
    let pellet_count = 10;
    let spread = 0.1;

    for _ in 0..pellet_count {
        let spread_x = (rand::random::<f32>() - 0.5) * spread;
        let spread_y = (rand::random::<f32>() - 0.5) * spread;
        
        let spread_dir = direction + Vec3::new(spread_x, spread_y, 0.0);
        let result = hitscan_trace(origin, spread_dir, 57.142857142857146, shooter_id, players, Weapon::Shotgun);
        results.push(result);
    }

    results
}

