use glam::Vec3;
use crate::engine::math::Frustum;
use crate::game::constants::*;

pub struct Rocket {
    pub position: Vec3,
    pub previous_position: Vec3,
    pub velocity: Vec3,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub active: bool,
    pub trail_time: f32,
    pub owner_id: u32,
}

impl Rocket {
    pub fn new(position: Vec3, direction: Vec3, speed: f32, frustum: &Frustum, owner_id: u32) -> Self {
        let velocity = direction.normalize() * speed;
        let max_lifetime = frustum.estimate_visibility_time(position, velocity, 0.014285714285714285);
        
        Self {
            position,
            previous_position: position,
            velocity,
            lifetime: 0.0,
            max_lifetime,
            active: true,
            trail_time: 0.0,
            owner_id,
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

        if !frustum.contains_sphere(self.position, 0.014285714285714285) {
            self.active = false;
        }
    }
    
    pub fn is_visible(&self, frustum: &Frustum) -> bool {
        frustum.contains_sphere(self.position, 0.014285714285714285)
    }
}

pub struct Grenade {
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: f32,
    pub fuse_time: f32,
    pub active: bool,
    pub owner_id: u32,
    pub bounced: bool,
}

impl Grenade {
    pub fn new(position: Vec3, velocity: Vec3, owner_id: u32) -> Self {
        Self {
            position,
            velocity,
            lifetime: 0.0,
            fuse_time: GRENADE_FUSE_SECS,
            active: true,
            owner_id,
            bounced: false,
        }
    }

    pub fn update(&mut self, dt: f32, ground_y: f32) {
        if !self.active {
            return;
        }

        self.position += self.velocity * dt;
        self.velocity.y -= GRAVITY * dt;
        self.lifetime += dt;

        if self.position.y <= ground_y {
            self.position.y = ground_y;
            self.velocity.y = -self.velocity.y * GRENADE_BOUNCE_FLOOR;
            self.velocity.x /= GRENADE_SLOWDOWN;
            self.bounced = true;
        }

        if self.lifetime >= self.fuse_time {
            self.active = false;
        }
    }
}

pub struct Plasma {
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub active: bool,
    pub owner_id: u32,
}

impl Plasma {
    pub fn new(position: Vec3, direction: Vec3, owner_id: u32) -> Self {
        let velocity = direction.normalize() * PLASMA_SPEED;
        Self {
            position,
            velocity,
            lifetime: 0.0,
            max_lifetime: 10.0,
            active: true,
            owner_id,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if !self.active {
            return;
        }

        self.position += self.velocity * dt;
        self.lifetime += dt;

        if self.lifetime >= self.max_lifetime {
            self.active = false;
        }
    }
}

pub struct BFGBall {
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub active: bool,
    pub owner_id: u32,
}

impl BFGBall {
    pub fn new(position: Vec3, direction: Vec3, owner_id: u32) -> Self {
        let velocity = direction.normalize() * BFG_SPEED;
        Self {
            position,
            velocity,
            lifetime: 0.0,
            max_lifetime: 10.0,
            active: true,
            owner_id,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if !self.active {
            return;
        }

        self.position += self.velocity * dt;
        self.lifetime += dt;

        if self.lifetime >= self.max_lifetime {
            self.active = false;
        }
    }
}
