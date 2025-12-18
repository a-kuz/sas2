use glam::{Mat4, Vec3};

pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            x: 0.0,
            y: 59.0,
            z: 500.0,
        }
    }

    pub fn get_view_proj(&self, aspect: f32) -> (Mat4, Vec3) {
        let camera_pos = Vec3::new(self.x, self.y, self.z);
        let camera_target = Vec3::new(self.x, self.y, 0.0);
        let view_matrix = Mat4::look_at_rh(camera_pos, camera_target, Vec3::Y);
        let proj_matrix = Mat4::perspective_rh(std::f32::consts::PI / 4.0, aspect, 0.1, 1000.0);
        (proj_matrix * view_matrix, camera_pos)
    }
}
