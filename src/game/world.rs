use crate::engine::math::Frustum;
use super::player::Player;
use super::weapon::Rocket;
use super::particle::{SmokeParticle, FlameParticle};
use super::map::Map;
use super::lighting::LightingParams;

pub struct World {
    pub players: Vec<Player>,
    pub rockets: Vec<Rocket>,
    pub smoke_particles: Vec<SmokeParticle>,
    pub flame_particles: Vec<FlameParticle>,
    pub map: Map,
    pub lighting: LightingParams,
    pub time: f32,
}

impl World {
    pub fn new() -> Self {
        Self {
            players: Vec::new(),
            rockets: Vec::new(),
            smoke_particles: Vec::new(),
            flame_particles: Vec::new(),
            map: Map::new(),
            lighting: LightingParams::new(),
            time: 0.0,
        }
    }

    pub fn add_player(&mut self) -> u32 {
        let id = self.players.len() as u32; // Simple ID generation
        self.players.push(Player::new(id));
        id
    }

    pub fn update(&mut self, dt: f32, frustum: &Frustum) {
        self.time += dt;

        // Update Rockets
        for rocket in &mut self.rockets {
            rocket.update(dt, frustum);
        }

        // Particle logic moved from main
        // We need to spawn particles based on rockets.
        // This is where "Systems" or separation helps avoid borrow issues.
        // If I iterate rockets (immutable) I can push to particles (mutable).
        
        // Spawn smoke/flame
        let step = 0.05;
        let mut new_smoke = Vec::new();
        let mut new_flame = Vec::new();

        for rocket in &self.rockets {
             if !rocket.active || !rocket.is_visible(frustum) {
                continue;
            }

            let start_time = rocket.trail_time - dt;
            let t_start = ((start_time / step).floor() + 1.0) * step;
            let t_end = (rocket.trail_time / step).floor() * step;
            
            if t_end >= t_start {
                let mut t = t_start;
                while t <= t_end {
                    let time_back = rocket.trail_time - t;
                    let alpha = if dt > 0.001 { time_back / dt } else { 0.0 };
                    let alpha = alpha.min(1.0).max(0.0);
                    let spawn_pos = rocket.previous_position * (1.0 - alpha) + rocket.position * alpha;
                    
                    let particle_start_time = self.time - (rocket.trail_time - t);
                    new_smoke.push(SmokeParticle::new(spawn_pos, particle_start_time));
                    
                    t += step;
                }
            }

            let flame_texture = ((rocket.trail_time * 20.0) as u32) % 3;
            let exhaust_dir = -rocket.velocity.normalize();
            let flame_pos = rocket.position + exhaust_dir * 0.15;
            new_flame.push(FlameParticle::new(flame_pos, flame_texture));
        }

        self.smoke_particles.append(&mut new_smoke);
        self.flame_particles.append(&mut new_flame);

        // Update Particles
        for particle in &mut self.smoke_particles {
            particle.update(dt, self.time);
        }
        self.smoke_particles.retain(|p| {
            let elapsed = self.time - p.start_time;
            elapsed < p.max_lifetime
        });

        for particle in &mut self.flame_particles {
             // Logic for flame specific finding rocket?
             // "if let Some(rocket) = rockets.find... transform match"
             // This is expensive to do O(N*M). 
             // In game_mvp.rs:
             // if let Some(rocket) = self.rockets.iter().find(|r| r.active && (r.position - particle.position).length() < 2.0)
             
             // I'll implement a simplified version or the same loop.
             // But I can't borrow self.rockets while iterating self.flame_particles if both in self?
             // Actually, I can decouple.
             // But here 'self' borrows everything.
             // I'll do it carefully below.
             particle.lifetime += dt; // Default update
        }
        
        // Correct flame update with rocket position matching
        // We need to split borrows or use indices.
        // Or just clone the rocket data needed?
        // Let's do a separate pass or just iterate carefully.
        // Actually, for simplicity/MVP refactor, I might skip the "flame follows rocket" exactly if it complicates safety too much, 
        // OR implement it by collecting rocket positions first.
        let active_socket_positions: Vec<(glam::Vec3, glam::Vec3)> = self.rockets.iter()
            .filter(|r| r.active)
            .map(|r| (r.position, r.velocity))
            .collect();

        for particle in &mut self.flame_particles {
            if let Some((_, vel)) = active_socket_positions.iter().find(|(pos, _)| (*pos - particle.position).length() < 2.0) {
                 // Reset/keep alive?
                 // game_mvp.rs: particle.update(dt, rocket.velocity);
                 // flame update logic is: lifetime += dt, position += ...
                 particle.update(dt, *vel);
            }
        }
        
        self.flame_particles.retain(|p| p.lifetime < p.max_lifetime);
        
        // Clean dead rockets
        // self.rockets.retain(|r| r.active); 
        // (Maybe keep them for a bit? Original code didn't seem to retain explicitly in update_particles loop, 
        // but `shoot_rocket` just pushed. I should clean them up.)
        self.rockets.retain(|r| r.active);
    }
}
