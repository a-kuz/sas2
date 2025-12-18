use crate::engine::math::Frustum;
use crate::audio::events::{AudioEvent, AudioEventQueue};
use super::player::Player;
use super::weapons::{Rocket, Grenade, Plasma, BFGBall};
use super::particle::{SmokeParticle, FlameParticle};
use super::map::Map;
use super::lighting::LightingParams;
use super::items::Item;
use super::awards::AwardTracker;
use super::hitscan::{RailBeam, LightningBeam};
use super::physics::collision;
use super::combat;
use super::constants::*;
use glam::Vec3;

pub struct World {
    pub players: Vec<Player>,
    pub rockets: Vec<Rocket>,
    pub grenades: Vec<Grenade>,
    pub plasma_bolts: Vec<Plasma>,
    pub bfg_balls: Vec<BFGBall>,
    pub smoke_particles: Vec<SmokeParticle>,
    pub flame_particles: Vec<FlameParticle>,
    pub rail_beams: Vec<RailBeam>,
    pub lightning_beams: Vec<LightningBeam>,
    pub items: Vec<Item>,
    pub map: Map,
    pub lighting: LightingParams,
    pub time: f32,
    pub audio_events: AudioEventQueue,
    pub awards: AwardTracker,
}

impl World {
    pub fn new() -> Self {
        Self {
            players: Vec::new(),
            rockets: Vec::new(),
            grenades: Vec::new(),
            plasma_bolts: Vec::new(),
            bfg_balls: Vec::new(),
            smoke_particles: Vec::new(),
            flame_particles: Vec::new(),
            rail_beams: Vec::new(),
            lightning_beams: Vec::new(),
            items: Vec::new(),
            map: Map::new(),
            lighting: LightingParams::new(),
            time: 0.0,
            audio_events: AudioEventQueue::new(),
            awards: AwardTracker::new(),
        }
    }

    pub fn add_player(&mut self) -> u32 {
        let id = self.players.len() as u32;
        self.players.push(Player::new(id));
        id
    }

    pub fn update(&mut self, dt: f32, frustum: &Frustum) {
        self.time += dt;

        for player in &mut self.players {
            player.update_timers(dt);
        }

        for rocket in &mut self.rockets {
            rocket.update(dt, frustum);
        }

        for grenade in &mut self.grenades {
            grenade.update(dt, self.map.ground_y);
        }

        for plasma in &mut self.plasma_bolts {
            plasma.update(dt);
        }

        for bfg in &mut self.bfg_balls {
            bfg.update(dt);
        }

        self.check_projectile_collisions();

        for item in &mut self.items {
            item.update(dt);
        }

        self.check_item_pickups();

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
            let flame_pos = rocket.position + exhaust_dir * (0.004285714285714286);
            new_flame.push(FlameParticle::new(flame_pos, flame_texture));
        }

        self.smoke_particles.append(&mut new_smoke);
        self.flame_particles.append(&mut new_flame);

        for particle in &mut self.smoke_particles {
            particle.update(dt, self.time);
        }
        self.smoke_particles.retain(|p| {
            let elapsed = self.time - p.start_time;
            elapsed < p.max_lifetime
        });

        let active_socket_positions: Vec<(Vec3, Vec3)> = self.rockets.iter()
            .filter(|r| r.active)
            .map(|r| (r.position, r.velocity))
            .collect();

        for particle in &mut self.flame_particles {
            if let Some((_, vel)) = active_socket_positions.iter().find(|(pos, _)| (*pos - particle.position).length() < 0.05714285714285714) {
                particle.update(dt, *vel);
            } else {
                let zero_vel = Vec3::ZERO;
                particle.update(dt, zero_vel);
            }
        }
        
        self.flame_particles.retain(|p| p.lifetime < p.max_lifetime);

        self.rail_beams.retain_mut(|beam| beam.update(dt));
        self.lightning_beams.retain_mut(|beam| beam.update(dt));

        self.rockets.retain(|r| r.active);
        self.grenades.retain(|g| g.active);
        self.plasma_bolts.retain(|p| p.active);
        self.bfg_balls.retain(|b| b.active);
    }

    fn check_projectile_collisions(&mut self) {
        let mut explosions = Vec::new();

        for rocket in &mut self.rockets {
            if !rocket.active {
                continue;
            }

            let collision = collision::check_projectile_players_collision(
                rocket.position,
                0.014285714285714285,
                rocket.owner_id,
                &self.players,
            );

            if collision.collided {
                rocket.active = false;
                explosions.push((rocket.position, ROCKET_SPLASH_RADIUS, rocket.owner_id));
                self.audio_events.push(AudioEvent::Explosion { x: rocket.position.x });
            } else if collision::check_projectile_ground_collision(rocket.position, self.map.ground_y) {
                rocket.active = false;
                explosions.push((rocket.position, ROCKET_SPLASH_RADIUS, rocket.owner_id));
                self.audio_events.push(AudioEvent::Explosion { x: rocket.position.x });
            }
        }

        for grenade in &mut self.grenades {
            if !grenade.active {
                continue;
            }

            let collision = collision::check_projectile_players_collision(
                grenade.position,
                0.014285714285714285,
                grenade.owner_id,
                &self.players,
            );

            if collision.collided {
                grenade.active = false;
                explosions.push((grenade.position, GRENADE_SPLASH_RADIUS, grenade.owner_id));
                self.audio_events.push(AudioEvent::Explosion { x: grenade.position.x });
            }
        }

        for plasma in &mut self.plasma_bolts {
            if !plasma.active {
                continue;
            }

            let collision = collision::check_projectile_players_collision(
                plasma.position,
                0.008571428571428572,
                plasma.owner_id,
                &self.players,
            );

            if collision.collided {
                plasma.active = false;
                if let Some(player_id) = collision.player_id {
                    let attacker_has_quad = self.players.iter()
                        .find(|p| p.id == plasma.owner_id)
                        .map(|p| p.powerups.quad > 0)
                        .unwrap_or(false);

                    if let Some(player) = self.players.iter_mut().find(|p| p.id == player_id) {
                        let result = combat::apply_damage(player, DAMAGE_PLASMA, attacker_has_quad, None);
                        
                        if result.killed {
                            self.audio_events.push(AudioEvent::PlayerDeath {
                                x: player.x,
                                model: player.model.clone(),
                            });
                        } else {
                            self.audio_events.push(AudioEvent::PlayerPain {
                                health: result.final_health,
                                x: player.x,
                                model: player.model.clone(),
                            });
                        }
                    }
                }
            }
        }

        for (explosion_pos, radius, owner_id) in explosions {
            let damages = collision::check_all_explosion_damage(
                explosion_pos,
                radius,
                owner_id,
                &self.players,
            );

            let attacker_has_quad = self.players.iter()
                .find(|p| p.id == owner_id)
                .map(|p| p.powerups.quad > 0)
                .unwrap_or(false);

            for (player_id, damage, knockback) in damages {
                if let Some(player) = self.players.iter_mut().find(|p| p.id == player_id) {
                    let result = combat::apply_damage(player, damage, attacker_has_quad, Some(knockback));
                    
                    if result.killed {
                        self.audio_events.push(AudioEvent::PlayerDeath {
                            x: player.x,
                            model: player.model.clone(),
                        });
                    } else {
                        self.audio_events.push(AudioEvent::PlayerPain {
                            health: result.final_health,
                            x: player.x,
                            model: player.model.clone(),
                        });
                    }
                }
            }
        }
    }

    fn check_item_pickups(&mut self) {
        for player in &mut self.players {
            if player.dead {
                continue;
            }

            let player_pos = Vec3::new(player.x, player.y, 0.0);

            for item in &mut self.items {
                if item.check_pickup(player_pos, 0.9142857142857143) {
                    if let Some(weapon) = item.item_type.to_weapon() {
                        player.give_weapon(weapon);
                        self.audio_events.push(AudioEvent::WeaponPickup { x: player.x });
                    } else if let Some(health) = item.item_type.health_amount() {
                        if player.add_health(health) {
                            self.audio_events.push(AudioEvent::ItemPickup { x: player.x });
                        } else {
                            continue;
                        }
                    } else if let Some(armor) = item.item_type.armor_amount() {
                        if player.add_armor(armor) {
                            self.audio_events.push(AudioEvent::ArmorPickup { x: player.x });
                        } else {
                            continue;
                        }
                    } else if let Some(duration) = item.item_type.powerup_duration() {
                        use super::items::ItemType;
                        match item.item_type {
                            ItemType::PowerupQuad => player.powerups.quad = duration,
                            ItemType::PowerupRegen => player.powerups.regen = duration,
                            ItemType::PowerupBattle => player.powerups.battle = duration,
                            ItemType::PowerupFlight => player.powerups.flight = duration,
                            ItemType::PowerupHaste => player.powerups.haste = duration,
                            ItemType::PowerupInvis => player.powerups.invis = duration,
                            _ => {}
                        }
                        self.audio_events.push(AudioEvent::PowerupPickup { x: player.x });
                    }

                    item.pickup();
                }
            }
        }
    }
}
