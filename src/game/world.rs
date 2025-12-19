use crate::engine::math::Frustum;
use crate::audio::events::{AudioEvent, AudioEventQueue};
use super::player::Player;
use super::weapons::{Rocket, Grenade, Plasma, BFGBall};
use super::particle::{SmokeParticle, FlameParticle};
use super::map::{Map, ItemType};
use super::lighting::LightingParams;
use super::awards::AwardTracker;
use super::hitscan::{RailBeam, LightningBeam, hitscan_trace, shotgun_trace};
use super::weapon::Weapon;
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
            map: Map::new(),
            lighting: LightingParams::new(),
            time: 0.0,
            audio_events: AudioEventQueue::new(),
            awards: AwardTracker::new(),
        }
    }

    pub fn add_player(&mut self) -> u32 {
        let id = self.players.len() as u32;
        let mut player = Player::new(id);
        
        let spawn_idx = (id as usize) % self.map.spawn_points.len().max(1);
        let (spawn_x, spawn_y) = if !self.map.spawn_points.is_empty() {
            let sp = &self.map.spawn_points[spawn_idx];
            (sp.x, sp.y)
        } else {
            self.map.find_safe_spawn_position()
        };
        
        player.spawn(spawn_x, spawn_y);
        self.players.push(player);
        id
    }

    pub fn update(&mut self, dt: f32, frustum: &Frustum) {
        self.time += dt;

        for jumppad in &mut self.map.jumppads {
            jumppad.update();
        }

        for player in &self.players {
            if !player.dead {
                for jumppad in &mut self.map.jumppads {
                    if jumppad.can_activate() && jumppad.check_collision(player.x, player.y) {
                        jumppad.activate();
                    }
                }
            }
        }

        for player in &mut self.players {
            player.update_timers(dt);
        }

        for player in &mut self.players {
            if player.dead && player.respawn_timer <= 0.0 {
                let spawn_idx = (player.id as usize) % self.map.spawn_points.len().max(1);
                let (spawn_x, spawn_y) = if !self.map.spawn_points.is_empty() {
                    let sp = &self.map.spawn_points[spawn_idx];
                    (sp.x, sp.y)
                } else {
                    self.map.find_safe_spawn_position()
                };
                
                player.spawn(spawn_x, spawn_y);
            }
        }

        for rocket in &mut self.rockets {
            rocket.update(dt, frustum);
        }

        for grenade in &mut self.grenades {
            grenade.update(dt, &self.map);
        }

        for plasma in &mut self.plasma_bolts {
            plasma.update(dt);
        }

        for bfg in &mut self.bfg_balls {
            bfg.update(dt);
        }

        self.check_projectile_collisions();

        self.update_items(dt);
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
            } else {
                let tile_x = self.map.world_to_tile_x(rocket.position.x);
                let tile_y = self.map.world_to_tile_y(rocket.position.y);
                if self.map.is_solid(tile_x, tile_y) {
                    rocket.active = false;
                    explosions.push((rocket.position, ROCKET_SPLASH_RADIUS, rocket.owner_id));
                    self.audio_events.push(AudioEvent::Explosion { x: rocket.position.x });
                }
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

            if grenade.lifetime >= grenade.fuse_time {
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
            } else {
                let tile_x = self.map.world_to_tile_x(plasma.position.x);
                let tile_y = self.map.world_to_tile_y(plasma.position.y);
                if self.map.is_solid(tile_x, tile_y) {
                    plasma.active = false;
                    explosions.push((plasma.position, PLASMA_SPLASH_RADIUS, plasma.owner_id));
                }
            }
        }

        for bfg in &mut self.bfg_balls {
            if !bfg.active {
                continue;
            }

            let collision = collision::check_projectile_players_collision(
                bfg.position,
                0.028571428571428574,
                bfg.owner_id,
                &self.players,
            );

            if collision.collided {
                bfg.active = false;
                explosions.push((bfg.position, BFG_SPLASH_RADIUS, bfg.owner_id));
                self.audio_events.push(AudioEvent::Explosion { x: bfg.position.x });
            } else {
                let tile_x = self.map.world_to_tile_x(bfg.position.x);
                let tile_y = self.map.world_to_tile_y(bfg.position.y);
                if self.map.is_solid(tile_x, tile_y) {
                    bfg.active = false;
                    explosions.push((bfg.position, BFG_SPLASH_RADIUS, bfg.owner_id));
                    self.audio_events.push(AudioEvent::Explosion { x: bfg.position.x });
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

    fn update_items(&mut self, _dt: f32) {
        for item in &mut self.map.items {
            if !item.active {
                if item.respawn_time > 0 {
                    item.respawn_time -= 1;
                } else {
                    item.active = true;
                }
            }
        }

        self.map.items.retain(|item| {
            !item.dropped || item.active || item.respawn_time > 0
        });
    }

    fn check_item_pickups(&mut self) {
        for player in &mut self.players {
            if player.dead {
                continue;
            }

            for item in &mut self.map.items {
                if !item.active {
                    continue;
                }

                let dx = player.x - item.x;
                let dy = player.y - item.y;
                let dist_sq = dx * dx + dy * dy;
                
                if dist_sq < 24.0 * 24.0 {
                    let mut picked_up = false;

                    match item.item_type {
                        ItemType::Health25 => {
                            if player.health < 100 {
                                player.health = (player.health + 25).min(100);
                                picked_up = true;
                                self.audio_events.push(AudioEvent::ItemPickup { x: item.x });
                            }
                        }
                        ItemType::Health50 => {
                            if player.health < 100 {
                                player.health = (player.health + 50).min(100);
                                picked_up = true;
                                self.audio_events.push(AudioEvent::ItemPickup { x: item.x });
                            }
                        }
                        ItemType::Health100 => {
                            if player.health < 200 {
                                player.health = (player.health + 100).min(200);
                                picked_up = true;
                                self.audio_events.push(AudioEvent::ItemPickup { x: item.x });
                            }
                        }
                        ItemType::Armor50 => {
                            if player.armor < 100 {
                                player.armor = (player.armor + 50).min(100);
                                picked_up = true;
                                self.audio_events.push(AudioEvent::ArmorPickup { x: item.x });
                            }
                        }
                        ItemType::Armor100 => {
                            if player.armor < 200 {
                                player.armor = (player.armor + 100).min(200);
                                picked_up = true;
                                self.audio_events.push(AudioEvent::ArmorPickup { x: item.x });
                            }
                        }
                        ItemType::RocketLauncher => {
                            player.has_weapon[4] = true;
                            player.ammo[4] = (player.ammo[4] + 10).min(100);
                            picked_up = true;
                            self.audio_events.push(AudioEvent::WeaponPickup { x: item.x });
                        }
                        ItemType::LightningGun => {
                            player.has_weapon[5] = true;
                            player.ammo[5] = (player.ammo[5].saturating_add(100)).min(200);
                            picked_up = true;
                            self.audio_events.push(AudioEvent::WeaponPickup { x: item.x });
                        }
                        ItemType::Railgun => {
                            player.has_weapon[6] = true;
                            player.ammo[6] = (player.ammo[6] + 10).min(100);
                            picked_up = true;
                            self.audio_events.push(AudioEvent::WeaponPickup { x: item.x });
                        }
                        ItemType::Plasmagun => {
                            player.has_weapon[7] = true;
                            player.ammo[7] = (player.ammo[7] + 50).min(200);
                            picked_up = true;
                            self.audio_events.push(AudioEvent::WeaponPickup { x: item.x });
                        }
                        ItemType::Shotgun => {
                            player.has_weapon[2] = true;
                            player.ammo[2] = (player.ammo[2] + 10).min(100);
                            picked_up = true;
                            self.audio_events.push(AudioEvent::WeaponPickup { x: item.x });
                        }
                        ItemType::GrenadeLauncher => {
                            player.has_weapon[3] = true;
                            player.ammo[3] = (player.ammo[3] + 10).min(100);
                            picked_up = true;
                            self.audio_events.push(AudioEvent::WeaponPickup { x: item.x });
                        }
                        ItemType::BFG => {
                            player.has_weapon[8] = true;
                            player.ammo[8] = (player.ammo[8] + 15).min(200);
                            picked_up = true;
                            self.audio_events.push(AudioEvent::WeaponPickup { x: item.x });
                        }
                        ItemType::Quad => {
                            player.powerups.quad = POWERUP_DURATION_QUAD;
                            picked_up = true;
                            self.audio_events.push(AudioEvent::PowerupPickup { x: item.x });
                        }
                        ItemType::Regen => {
                            player.powerups.regen = POWERUP_DURATION_REGEN;
                            picked_up = true;
                            self.audio_events.push(AudioEvent::PowerupPickup { x: item.x });
                        }
                        ItemType::Battle => {
                            player.powerups.battle = POWERUP_DURATION_BATTLE;
                            picked_up = true;
                            self.audio_events.push(AudioEvent::PowerupPickup { x: item.x });
                        }
                        ItemType::Flight => {
                            player.powerups.flight = POWERUP_DURATION_FLIGHT;
                            picked_up = true;
                            self.audio_events.push(AudioEvent::PowerupPickup { x: item.x });
                        }
                        ItemType::Haste => {
                            player.powerups.haste = POWERUP_DURATION_HASTE;
                            picked_up = true;
                            self.audio_events.push(AudioEvent::PowerupPickup { x: item.x });
                        }
                        ItemType::Invis => {
                            player.powerups.invis = POWERUP_DURATION_INVIS;
                            picked_up = true;
                            self.audio_events.push(AudioEvent::PowerupPickup { x: item.x });
                        }
                    }

                    if picked_up {
                        item.active = false;
                        item.respawn_time = match item.item_type {
                            ItemType::Health25 | ItemType::Health50 | ItemType::Health100 => ITEM_RESPAWN_HEALTH,
                            ItemType::Armor50 | ItemType::Armor100 => ITEM_RESPAWN_ARMOR,
                            ItemType::Shotgun | ItemType::GrenadeLauncher => 300,
                            ItemType::RocketLauncher | ItemType::LightningGun | ItemType::Railgun | ItemType::Plasmagun => ITEM_RESPAWN_WEAPON,
                            ItemType::BFG => 600,
                            ItemType::Quad | ItemType::Regen | ItemType::Battle | ItemType::Flight | ItemType::Haste | ItemType::Invis => ITEM_RESPAWN_POWERUP,
                        };
                    }
                }
            }
        }
    }

    pub fn try_fire(&mut self, player_id: u32, aim_angle: f32, frustum: &Frustum) -> bool {
        let player = match self.players.iter_mut().find(|p| p.id == player_id) {
            Some(p) => p,
            None => return false,
        };

        if !player.can_fire() {
            return false;
        }

        if !player.consume_ammo() {
            return false;
        }

        player.refire = player.weapon.refire_time_seconds();

        let weapon = player.weapon;
        let player_x = player.x;
        let player_y = player.y;
        let player_vx = player.vx;
        let player_vy = player.vy;

        if weapon.is_projectile() {
            let direction = Vec3::new(aim_angle.cos(), aim_angle.sin(), 0.0);
            let spawn_pos = Vec3::new(player_x, player_y, 0.0);

            match weapon {
                Weapon::RocketLauncher => {
                    let rocket = Rocket::new(spawn_pos, direction, ROCKET_SPEED, frustum, player_id);
                    self.rockets.push(rocket);
                }
                Weapon::GrenadeLauncher => {
                    let base_velocity = direction * GRENADE_SPEED;
                    let velocity = Vec3::new(
                        base_velocity.x + player_vx * 0.5,
                        base_velocity.y + player_vy * 0.5 - 1.5,
                        0.0,
                    );
                    let grenade = Grenade::new(spawn_pos, velocity, player_id);
                    self.grenades.push(grenade);
                }
                Weapon::Plasmagun => {
                    let plasma = Plasma::new(spawn_pos, direction, player_id);
                    self.plasma_bolts.push(plasma);
                }
                Weapon::BFG => {
                    let bfg = BFGBall::new(spawn_pos, direction, player_id);
                    self.bfg_balls.push(bfg);
                }
                _ => {}
            }
        } else if weapon.is_hitscan() {
            let origin = Vec3::new(player_x, player_y, 0.0);
            let direction = Vec3::new(aim_angle.cos(), aim_angle.sin(), 0.0);

            match weapon {
                Weapon::Shotgun => {
                    let hits = shotgun_trace(origin, direction, player_id, &self.players);
                    for hit in hits {
                        if hit.hit {
                            if let Some(victim_id) = hit.hit_player_id {
                                let attacker_has_quad = self.players.iter()
                                    .find(|p| p.id == player_id)
                                    .map(|p| p.powerups.quad > 0)
                                    .unwrap_or(false);

                                if let Some(victim) = self.players.iter_mut().find(|p| p.id == victim_id) {
                                    let result = combat::apply_damage(victim, hit.damage, attacker_has_quad, None);
                                    
                                    if result.killed {
                                        self.audio_events.push(AudioEvent::PlayerDeath {
                                            x: victim.x,
                                            model: victim.model.clone(),
                                        });
                                    } else {
                                        self.audio_events.push(AudioEvent::PlayerPain {
                                            health: result.final_health,
                                            x: victim.x,
                                            model: victim.model.clone(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                Weapon::MachineGun | Weapon::Lightning => {
                    let max_distance = 57.142857142857146;
                    let hit = hitscan_trace(origin, direction, max_distance, player_id, &self.players, weapon);
                    
                    if hit.hit {
                        if let Some(victim_id) = hit.hit_player_id {
                            let attacker_has_quad = self.players.iter()
                                .find(|p| p.id == player_id)
                                .map(|p| p.powerups.quad > 0)
                                .unwrap_or(false);

                            if let Some(victim) = self.players.iter_mut().find(|p| p.id == victim_id) {
                                let result = combat::apply_damage(victim, hit.damage, attacker_has_quad, None);
                                
                                if result.killed {
                                    self.audio_events.push(AudioEvent::PlayerDeath {
                                        x: victim.x,
                                        model: victim.model.clone(),
                                    });
                                } else {
                                    self.audio_events.push(AudioEvent::PlayerPain {
                                        health: result.final_health,
                                        x: victim.x,
                                        model: victim.model.clone(),
                                    });
                                }
                            }
                        }

                        if matches!(weapon, Weapon::Lightning) {
                            let beam = LightningBeam::new(origin, hit.hit_position);
                            self.lightning_beams.push(beam);
                        }
                    }
                }
                Weapon::Railgun => {
                    let max_distance = 285.71428571428567;
                    let hit = hitscan_trace(origin, direction, max_distance, player_id, &self.players, weapon);
                    
                    if hit.hit {
                        if let Some(victim_id) = hit.hit_player_id {
                            let attacker_has_quad = self.players.iter()
                                .find(|p| p.id == player_id)
                                .map(|p| p.powerups.quad > 0)
                                .unwrap_or(false);

                            if let Some(victim) = self.players.iter_mut().find(|p| p.id == victim_id) {
                                let result = combat::apply_damage(victim, hit.damage, attacker_has_quad, None);
                                
                                if result.killed {
                                    self.audio_events.push(AudioEvent::PlayerDeath {
                                        x: victim.x,
                                        model: victim.model.clone(),
                                    });
                                } else {
                                    self.audio_events.push(AudioEvent::PlayerPain {
                                        health: result.final_health,
                                        x: victim.x,
                                        model: victim.model.clone(),
                                    });
                                }
                            }
                        }
                    }

                    let beam = RailBeam::new(origin, hit.hit_position);
                    self.rail_beams.push(beam);
                }
                Weapon::Gauntlet => {
                    let max_distance = 1.1428571428571428;
                    let hit = hitscan_trace(origin, direction, max_distance, player_id, &self.players, weapon);
                    
                    if hit.hit {
                        if let Some(victim_id) = hit.hit_player_id {
                            let attacker_has_quad = self.players.iter()
                                .find(|p| p.id == player_id)
                                .map(|p| p.powerups.quad > 0)
                                .unwrap_or(false);

                            if let Some(victim) = self.players.iter_mut().find(|p| p.id == victim_id) {
                                let result = combat::apply_damage(victim, hit.damage, attacker_has_quad, None);
                                
                                if result.killed {
                                    self.audio_events.push(AudioEvent::PlayerDeath {
                                        x: victim.x,
                                        model: victim.model.clone(),
                                    });
                                } else {
                                    self.audio_events.push(AudioEvent::PlayerPain {
                                        health: result.final_health,
                                        x: victim.x,
                                        model: victim.model.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        true
    }
}
