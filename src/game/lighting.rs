use glam::Vec3;

#[derive(Clone)]
pub struct Light {
    pub position: Vec3,
    pub color: Vec3,
    pub radius: f32,
    pub flicker_enabled: bool,
    pub flicker_frequency: f32,
    pub flicker_intensity: f32,
    pub flicker_phase: f32,
    pub flicker_randomized: bool,
}

impl Light {
    pub fn new(position: Vec3, color: Vec3, radius: f32) -> Self {
        Self {
            position,
            color,
            radius,
            flicker_enabled: false,
            flicker_frequency: 0.0,
            flicker_intensity: 0.0,
            flicker_phase: 0.0,
            flicker_randomized: false,
        }
    }

    pub fn with_flicker(
        position: Vec3,
        color: Vec3,
        radius: f32,
        frequency: f32,
        intensity: f32,
        phase: f32,
    ) -> Self {
        Self {
            position,
            color,
            radius,
            flicker_enabled: true,
            flicker_frequency: frequency,
            flicker_intensity: intensity,
            flicker_phase: phase,
            flicker_randomized: false,
        }
    }

    pub fn with_randomized_flicker(
        position: Vec3,
        color: Vec3,
        radius: f32,
        frequency: f32,
        intensity: f32,
    ) -> Self {
        Self {
            position,
            color,
            radius,
            flicker_enabled: true,
            flicker_frequency: frequency,
            flicker_intensity: intensity,
            flicker_phase: 0.0,
            flicker_randomized: true,
        }
    }

    pub fn get_color_at_time(&self, time: f32) -> Vec3 {
        if !self.flicker_enabled {
            return self.color;
        }

        let flicker_value = if self.flicker_randomized {
            let seed = self.position.x * 73.0 + self.position.y * 97.0 + self.position.z * 113.0;
            let time_quantized = (time * self.flicker_frequency).floor();
            let hash = ((time_quantized + seed) * 12.9898).sin() * 43758.5453;
            let random = (hash - hash.floor()) * 2.0 - 1.0;
            let smooth = (time * self.flicker_frequency * 2.0 * std::f32::consts::PI).sin() * 0.3;
            (random + smooth).clamp(-1.0, 1.0)
        } else {
            (time * self.flicker_frequency * 2.0 * std::f32::consts::PI + self.flicker_phase).sin()
        };
        
        let normalized = (flicker_value + 1.0) * 0.5;
        let flicker_factor = 1.0 - self.flicker_intensity * (1.0 - normalized);
        self.color * flicker_factor.max(0.0)
    }
}

pub struct LightingParams {
    pub lights: Vec<Light>,
    pub ambient: f32,
}

impl LightingParams {
    pub fn new() -> Self {
        Self {
            lights: vec![
                Light::new(Vec3::new(-250.0, 50.0, 50.0), Vec3::new(1.6, 1.6, 2.7), 875.0),
                
            ],
            ambient: 0.015,
        }
    }

    pub fn from_map_lights(map_lights: &[super::map::LightSource]) -> Self {
        let lights: Vec<Light> = map_lights
            .iter()
            .map(|ls| {
                let position = Vec3::new(ls.x, ls.y, 400.0);
                let color = Vec3::new(
                    ls.r as f32 / 255.0,
                    ls.g as f32 / 255.0,
                    ls.b as f32 / 255.0,
                ) * ls.intensity;
                
                if ls.flicker {
                    Light::with_randomized_flicker(
                        position,
                        color,
                        ls.radius * 20.0,
                        8.0,
                        0.15,
                    )
                } else {
                    Light::new(position, color, ls.radius * 20.0)
                }
            })
            .collect();

        Self {
            lights,
            ambient: 0.015,
        }
    }
}
