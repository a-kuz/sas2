use glam::{Mat4, Vec3};
use super::map::Map;

pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub target_x: f32,
    pub target_y: f32,
    pub pitch: f32,
    pub yaw: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            x: 0.0,
            y: 59.0,
            z: 500.0,
            target_x: 0.0,
            target_y: 59.0,
            pitch: 0.0,
            yaw: 0.0,
        }
    }

    pub fn follow(&mut self, player_x: f32, player_y: f32) {
        self.target_x = player_x;
        self.target_y = player_y + 59.0;
    }

    pub fn update(&mut self, dt: f32, map: &Map, aspect: f32) {
        const SMOOTHNESS: f32 = 3.0;
        const FOV: f32 = std::f32::consts::PI / 4.0;

        self.x += (self.target_x - self.x) * SMOOTHNESS * dt;
        self.y += (self.target_y - self.y) * SMOOTHNESS * dt;

        let map_width_world = map.width as f32 * map.tile_width;
        let map_height_world = map.height as f32 * map.tile_height;

        let distance_to_ground = self.z;
        let half_view_height = distance_to_ground * (FOV * 0.5).tan();
        let half_view_width = half_view_height * aspect;

        let map_left = map.origin_x();
        let map_right = map.origin_x() + map_width_world;
        let map_bottom = map.ground_y;
        let map_top = map.ground_y + map_height_world;

        let min_x = map_left + half_view_width;
        let max_x = map_right - half_view_width;
        let min_y = map_bottom + half_view_height;
        let max_y = map_top - half_view_height;

        if max_x > min_x {
            self.x = self.x.clamp(min_x, max_x);
        } else {
            self.x = (map_left + map_right) * 0.5;
        }

        if max_y > min_y {
            self.y = self.y.clamp(min_y, max_y);
        } else {
            self.y = (map_bottom + map_top) * 0.5;
        }
    }

    pub fn get_view_proj(&self, aspect: f32) -> (Mat4, Vec3) {
        let camera_pos = Vec3::new(self.x, self.y, self.z);
        
        let pitch_offset = self.pitch * 100.0;
        let yaw_offset = self.yaw * 50.0;
        let camera_target = Vec3::new(self.x + yaw_offset, self.y + pitch_offset, 0.0);
        
        let view_matrix = Mat4::look_at_rh(camera_pos, camera_target, Vec3::Y);
        let proj_matrix = Mat4::perspective_rh(std::f32::consts::PI / 4.0, aspect, 0.1, 1000.0);
        (proj_matrix * view_matrix, camera_pos)
    }
}
