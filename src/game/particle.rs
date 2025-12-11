use glam::Vec3;

pub struct SmokeParticle {
    pub position: Vec3,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub size: f32,
    pub initial_size: f32,
    pub start_time: f32,
}

impl SmokeParticle {
    pub fn new(position: Vec3, start_time: f32) -> Self {
        let scale = 0.04;
        let initial_size = 24.0 * scale * 0.5;
        Self {
            position,
            lifetime: 0.0,
            max_lifetime: 2.0,
            size: initial_size,
            initial_size,
            start_time,
        }
    }

    pub fn update(&mut self, _dt: f32, current_time: f32) -> bool {
        let elapsed = current_time - self.start_time;
        self.lifetime = elapsed;
        
        let life_ratio = self.lifetime / self.max_lifetime;
        if life_ratio >= 1.0 {
            return false;
        }
        
        // Quad scaling? 
        let size_growth = 1.0 + life_ratio * 1.5;
        self.size = self.initial_size * size_growth;
        
        true
    }

    pub fn get_alpha(&self) -> f32 {
        let life_ratio = self.lifetime / self.max_lifetime;
        if life_ratio >= 1.0 {
            return 0.0;
        }
        
        if life_ratio < 0.1 {
            life_ratio / 0.1 * 0.33
        } else {
            let fade_start = 0.7;
            let fade_end = 1.0;
            if life_ratio < fade_start {
                0.33
            } else {
                0.33 * (1.0 - (life_ratio - fade_start) / (fade_end - fade_start)).max(0.0)
            }
        }
    }
}

pub struct FlameParticle {
    pub position: Vec3,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub size: f32,
    pub texture_index: u32,
}

impl FlameParticle {
    pub fn new(position: Vec3, texture_index: u32) -> Self {
        Self {
            position,
            lifetime: 0.0,
            max_lifetime: 0.15,
            size: 0.3,
            texture_index,
        }
    }

    pub fn update(&mut self, dt: f32, rocket_velocity: Vec3) -> bool {
        self.lifetime += dt;
        let life_ratio = self.lifetime / self.max_lifetime;
        
        let vel_len = rocket_velocity.length();
        let dir = if vel_len > 0.001 {
            -rocket_velocity / vel_len
        } else {
            Vec3::new(-1.0, 0.0, 0.0)
        };
        
        self.position += rocket_velocity * dt * 0.3 + dir * 0.5 * dt;
        
        let size_curve = 1.0 - life_ratio * 0.5;
        self.size = 0.3 * size_curve;
        
        self.lifetime < self.max_lifetime
    }
}
