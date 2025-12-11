use glam::Vec3;
use crate::engine::math::Frustum;

pub struct Rocket {
    pub position: Vec3,
    pub previous_position: Vec3,
    pub velocity: Vec3,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub active: bool,
    pub trail_time: f32,
}

impl Rocket {
    pub fn new(position: Vec3, direction: Vec3, speed: f32, frustum: &Frustum) -> Self {
        let velocity = direction.normalize() * speed;
        // estimate_visibility_time was used before, but it's better to just have a fixed max lifetime 
        // OR rely on Frustum if we really want that optimization.
        // For "universal model", relying on frustum for LIFETIME is weird gameplay-wise (rockets dying when offscreen?).
        // Actually, the original code had:
        // let max_lifetime = frustum.estimate_visibility_time(position, velocity, 0.5);
        // This implies optimization.
        // I'll keep the signature but maybe default to a reasonable max if not optimized.
        
        let max_lifetime = frustum.estimate_visibility_time(position, velocity, 0.5);
        
        Self {
            position,
            previous_position: position,
            velocity,
            lifetime: 0.0,
            max_lifetime,
            active: true,
            trail_time: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32, frustum: &Frustum) {
        if !self.active {
            return;
        }

        self.previous_position = self.position;
        self.position += self.velocity * dt;
        self.lifetime += dt;
        self.trail_time += dt;

        if self.lifetime > self.max_lifetime {
            self.active = false;
            return;
        }

        if !frustum.contains_sphere(self.position, 0.5) {
            self.active = false;
        }
    }
    
    pub fn is_visible(&self, frustum: &Frustum) -> bool {
        frustum.contains_sphere(self.position, 0.5)
    }
}
