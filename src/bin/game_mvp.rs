use std::sync::Arc;
use std::time::Instant;

use glam::{Mat3, Mat4, Vec3};
use pollster::FutureExt;
use wgpu::Texture;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use test_md3_standalone::anim::AnimConfig;
use test_md3_standalone::loader::{load_textures_for_model_static, load_weapon_textures_static, load_rocket_textures_static};
use test_md3_standalone::math::{axis_from_mat3, attach_rotated_entity, orientation_to_mat4, Orientation, Frustum};
use test_md3_standalone::md3::MD3Model;
use test_md3_standalone::renderer::{MD3Renderer, WgpuRenderer};


struct Light {
    position: Vec3,
    color: Vec3,
    radius: f32,
    flicker_enabled: bool,
    flicker_frequency: f32,
    flicker_intensity: f32,
    flicker_phase: f32,
    flicker_randomized: bool,
}

impl Light {
    fn new(position: Vec3, color: Vec3, radius: f32) -> Self {
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

    fn with_flicker(
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

    fn with_randomized_flicker(
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

    fn get_color_at_time(&self, time: f32) -> Vec3 {
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

struct LightingParams {
    lights: Vec<Light>,
    ambient: f32,
}

impl LightingParams {
    fn new() -> Self {
        Self {
            lights: vec![
                Light::new(Vec3::new(-10.0, 9.0, 12.0), Vec3::new(1.6, 1.6, 2.7), 135.0),
                Light::with_flicker(
                    Vec3::new(9.0, 1.0, 0.0),
                    Vec3::new(20.5, 2.5, 2.5),
                    5.0,
                    8.0,
                    0.4,
                    0.0,
                ),
                Light::with_randomized_flicker(
                    Vec3::new(-15.0, 2.0, -8.0),
                    Vec3::new(15.0, 12.0, 8.0),
                    8.0,
                    10.0,
                    0.5,
                ),
            ],
            ambient: 0.00015,
        }
    }
}

struct Camera {
    x: f32,
    y: f32,
    z: f32,
}

impl Camera {
    fn new() -> Self {
        Self {
            x: 0.0,
            y: 5.0,
            z: 35.0,
        }
    }


    fn get_view_proj(&self, aspect: f32) -> (Mat4, Vec3) {
        let camera_pos = Vec3::new(self.x, self.y, self.z);
        let camera_target = Vec3::new(self.x, self.y, 0.0);
        let view_matrix = Mat4::look_at_rh(camera_pos, camera_target, Vec3::Y);
        let proj_matrix = Mat4::perspective_rh(std::f32::consts::PI / 4.0, aspect, 0.1, 1000.0);
        (proj_matrix * view_matrix, camera_pos)
    }
}

struct Player {
    x: f32,
    vx: f32,
    facing_right: bool,
    is_moving: bool,
    animation_time: f32,
}

impl Player {
    fn new() -> Self {
        Self {
            x: 0.0,
            vx: 0.0,
            facing_right: true,
            is_moving: false,
            animation_time: 0.0,
        }
    }

    fn update(&mut self, dt: f32, move_left: bool, move_right: bool) {
        let accel = 100.0;
        let friction = 10.0;
        let max_speed = 15.0;

        if move_left && !move_right {
            self.vx -= accel * dt;
            self.facing_right = false;
            self.is_moving = true;
        } else if move_right && !move_left {
            self.vx += accel * dt;
            self.facing_right = true;
            self.is_moving = true;
        } else {
            self.is_moving = false;
        }

        self.vx -= self.vx * friction * dt;
        self.vx = self.vx.clamp(-max_speed, max_speed);

        if self.vx.abs() < 0.01 {
            self.vx = 0.0;
        }

        self.x += self.vx * dt;

        if self.is_moving {
            self.animation_time += dt;
        }
    }
}

struct Rocket {
    position: Vec3,
    previous_position: Vec3,
    velocity: Vec3,
    lifetime: f32,
    max_lifetime: f32,
    active: bool,
    trail_time: f32,
}

impl Rocket {
    fn new(position: Vec3, direction: Vec3, speed: f32, frustum: &Frustum) -> Self {
        let velocity = direction.normalize() * speed;
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

    fn update(&mut self, dt: f32, frustum: &Frustum) {
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
    
    fn is_visible(&self, frustum: &Frustum) -> bool {
        frustum.contains_sphere(self.position, 0.5)
    }
}

struct SmokeParticle {
    position: Vec3,
    lifetime: f32,
    max_lifetime: f32,
    size: f32,
    initial_size: f32,
    start_time: f32,
}

impl SmokeParticle {
    fn new(position: Vec3, start_time: f32) -> Self {
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

    fn update(&mut self, dt: f32, current_time: f32) -> bool {
        let elapsed = current_time - self.start_time;
        self.lifetime = elapsed;
        
        let life_ratio = self.lifetime / self.max_lifetime;
        if life_ratio >= 1.0 {
            return false;
        }
        
        let size_growth = 1.0 + life_ratio * 1.5;
        self.size = self.initial_size * size_growth;
        
        true
    }

    fn get_alpha(&self) -> f32 {
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

struct FlameParticle {
    position: Vec3,
    lifetime: f32,
    max_lifetime: f32,
    size: f32,
    texture_index: u32,
}

impl FlameParticle {
    fn new(position: Vec3, texture_index: u32) -> Self {
        Self {
            position,
            lifetime: 0.0,
            max_lifetime: 0.15,
            size: 0.3,
            texture_index,
        }
    }

    fn update(&mut self, dt: f32, rocket_velocity: Vec3) -> bool {
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


struct PlayerModel {
    lower: Option<MD3Model>,
    upper: Option<MD3Model>,
    head: Option<MD3Model>,
    weapon: Option<MD3Model>,
    lower_textures: Vec<Option<String>>,
    upper_textures: Vec<Option<String>>,
    head_textures: Vec<Option<String>>,
    weapon_textures: Vec<Option<String>>,
    anim_config: Option<AnimConfig>,
}

struct GameApp {
    window: Option<Arc<Window>>,
    wgpu_renderer: Option<WgpuRenderer>,
    md3_renderer: Option<MD3Renderer>,
    player_model: PlayerModel,
    player2_model: PlayerModel,
    rocket_model: Option<MD3Model>,
    rocket_textures: Vec<Option<String>>,
    depth_texture: Option<Texture>,
    depth_view: Option<wgpu::TextureView>,
    start_time: Instant,
    last_frame_time: Instant,
    last_fps_update: Instant,
    frame_count: u32,
    fps: f32,
    player: Player,
    move_left: bool,
    move_right: bool,
    shoot_pressed: bool,
    rockets: Vec<Rocket>,
    last_shot_time: f32,
    smoke_particles: Vec<SmokeParticle>,
    flame_particles: Vec<FlameParticle>,
    camera: Camera,
    camera_move_x_neg: bool,
    camera_move_x_pos: bool,
    camera_move_y_neg: bool,
    camera_move_y_pos: bool,
    camera_move_z_neg: bool,
    camera_move_z_pos: bool,
}

impl PlayerModel {
    fn new() -> Self {
        Self {
            lower: None,
            upper: None,
            head: None,
            weapon: None,
            lower_textures: Vec::new(),
            upper_textures: Vec::new(),
            head_textures: Vec::new(),
            weapon_textures: Vec::new(),
            anim_config: None,
        }
    }
}

impl GameApp {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            window: None,
            wgpu_renderer: None,
            md3_renderer: None,
            player_model: PlayerModel::new(),
            player2_model: PlayerModel::new(),
            rocket_model: None,
            rocket_textures: Vec::new(),
            depth_texture: None,
            depth_view: None,
            start_time: now,
            last_frame_time: now,
            last_fps_update: now,
            frame_count: 0,
            fps: 0.0,
            player: Player::new(),
            move_left: false,
            move_right: false,
            shoot_pressed: false,
            rockets: Vec::new(),
            last_shot_time: 0.0,
            smoke_particles: Vec::new(),
            flame_particles: Vec::new(),
            camera: Camera::new(),
            camera_move_x_neg: false,
            camera_move_x_pos: false,
            camera_move_y_neg: false,
            camera_move_y_pos: false,
            camera_move_z_neg: false,
            camera_move_z_pos: false,
        }
    }

    fn create_depth(&mut self) {
        if let Some(ref wgpu_renderer) = self.wgpu_renderer {
            let (width, height) = wgpu_renderer.get_surface_size();
            let depth_texture = wgpu_renderer
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("Depth Texture"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth24PlusStencil8,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
            let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.depth_texture = Some(depth_texture);
            self.depth_view = Some(depth_view);
        }
    }

    fn load_model_part(paths: &[&str]) -> Option<MD3Model> {
        paths
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .and_then(|path| {
                println!("Loading model: {}", path);
                MD3Model::load(path).ok()
            })
    }

    fn update_fps_counter(&mut self, now: Instant) {
        self.frame_count += 1;
        let fps_elapsed = now.duration_since(self.last_fps_update).as_secs_f32();
        if fps_elapsed >= 0.5 {
            self.fps = self.frame_count as f32 / fps_elapsed;
            self.frame_count = 0;
            self.last_fps_update = now;
            if let Some(ref window) = self.window {
                window.set_title(&format!(
                    "SAS2 MVP | FPS: {:.0} | X: {:.1}",
                    self.fps, self.player.x
                ));
            }
        }
    }

    fn calculate_legs_frame(
        anim_config: &Option<AnimConfig>,
        is_moving: bool,
        animation_time: f32,
        model: &MD3Model,
    ) -> usize {
        if let Some(ref config) = anim_config {
            let anim = if is_moving {
                &config.legs_run
            } else {
                &config.legs_idle
            };
            let frame_in_anim = if anim.looping_frames > 0 {
                ((animation_time * anim.fps as f32) as usize) % anim.looping_frames
            } else {
                0
            };
            let frame = anim.first_frame + frame_in_anim;
            frame.min(model.header.num_bone_frames as usize - 1)
        } else {
            0
        }
    }

    fn calculate_torso_frame(
        anim_config: &Option<AnimConfig>,
        elapsed_time: f32,
        model: &MD3Model,
    ) -> usize {
        if let Some(ref config) = anim_config {
            let anim = &config.torso_stand;
            let frame_in_anim = if anim.looping_frames > 0 {
                let loop_frames = anim.looping_frames.min(anim.num_frames);
                ((elapsed_time * anim.fps as f32) as usize) % loop_frames
            } else {
                0
            };
            let frame = anim.first_frame + frame_in_anim;
            frame.min(model.header.num_bone_frames as usize - 1)
        } else {
            0
        }
    }

    fn shoot_rocket(&mut self, view_proj: Mat4) {
        let muzzle_offset = if self.player.facing_right {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(-1.0, 0.0, 0.0)
        };
        let rocket_position = Vec3::new(self.player.x, 0.5, 0.0) + muzzle_offset;
        let direction = if self.player.facing_right {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(-1.0, 0.0, 0.0)
        };
        let frustum = Frustum::from_view_proj(view_proj);
        self.rockets.push(Rocket::new(rocket_position, direction, 10.0, &frustum));
        let time = self.start_time.elapsed().as_secs_f32();
        self.last_shot_time = time;
    }

    fn find_tag<'a>(tags: &'a [test_md3_standalone::md3::Tag], name: &str) -> Option<&'a test_md3_standalone::md3::Tag> {
        tags.iter().find(|t| {
            let tag_name = std::str::from_utf8(&t.name).unwrap_or("");
            tag_name.trim_end_matches('\0') == name
        })
    }

    fn render_player<'a>(
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        md3_renderer: &mut MD3Renderer,
        surface_format: wgpu::TextureFormat,
        player_model: &'a PlayerModel,
        game_transform: Mat4,
        scale_mat: Mat4,
        lower_orientation: Orientation,
        lower_frame: usize,
        upper_frame: usize,
        view_proj: Mat4,
        camera_pos: Vec3,
        lights: &[(Vec3, Vec3, f32)],
        ambient: f32,
        include_weapon: bool,
    ) -> (Option<Orientation>, Vec<(&'a MD3Model, usize, &'a [Option<String>], Mat4)>) {
        let mut shadow_models = Vec::new();
        let mut upper_orientation = lower_orientation;
        let mut head_orientation: Option<Orientation> = None;
        let mut weapon_orientation: Option<Orientation> = None;

        if let Some(ref lower) = player_model.lower {
            let md3_model_mat = scale_mat * orientation_to_mat4(&lower_orientation);
            let model_mat = game_transform * md3_model_mat;
            md3_renderer.render_model(
                encoder,
                view,
                depth_view,
                surface_format,
                lower,
                lower_frame,
                &player_model.lower_textures,
                model_mat,
                view_proj,
                camera_pos,
                lights,
                ambient,
                true,
            );
            shadow_models.push((lower, lower_frame, player_model.lower_textures.as_slice(), model_mat));

            if let Some(tags) = lower.tags.get(lower_frame) {
                if let Some(torso_tag) = Self::find_tag(tags, "tag_torso") {
                    upper_orientation = attach_rotated_entity(&lower_orientation, torso_tag);
                }
            }
        }

        if let Some(ref upper) = player_model.upper {
            let md3_model_mat = scale_mat * orientation_to_mat4(&upper_orientation);
            let model_mat = game_transform * md3_model_mat;
            md3_renderer.render_model(
                encoder,
                view,
                depth_view,
                surface_format,
                upper,
                upper_frame,
                &player_model.upper_textures,
                model_mat,
                view_proj,
                camera_pos,
                lights,
                ambient,
                true,
            );
            shadow_models.push((upper, upper_frame, player_model.upper_textures.as_slice(), model_mat));

            if let Some(tags) = upper.tags.get(upper_frame) {
                if let Some(head_tag) = Self::find_tag(tags, "tag_head") {
                    head_orientation = Some(attach_rotated_entity(&upper_orientation, head_tag));
                }
                if include_weapon {
                    if let Some(weapon_tag) = Self::find_tag(tags, "tag_weapon") {
                        weapon_orientation = Some(attach_rotated_entity(&upper_orientation, weapon_tag));
                    }
                }
            }
        }

        if let (Some(ref head), Some(head_orient)) = (&player_model.head, head_orientation) {
            let md3_model_mat = scale_mat * orientation_to_mat4(&head_orient);
            let model_mat = game_transform * md3_model_mat;
            md3_renderer.render_model(
                encoder,
                view,
                depth_view,
                surface_format,
                head,
                0,
                &player_model.head_textures,
                model_mat,
                view_proj,
                camera_pos,
                lights,
                ambient,
                true,
            );
            shadow_models.push((head, 0, player_model.head_textures.as_slice(), model_mat));
        }

        if include_weapon {
            if let (Some(ref weapon), Some(weapon_orient)) = (&player_model.weapon, weapon_orientation) {
                let md3_model_mat = scale_mat * orientation_to_mat4(&weapon_orient);
                let model_mat = game_transform * md3_model_mat;
                md3_renderer.render_model(
                    encoder,
                    view,
                    depth_view,
                    surface_format,
                    weapon,
                    0,
                    &player_model.weapon_textures,
                    model_mat,
                    view_proj,
                    camera_pos,
                    lights,
                    ambient,
                    true,
                );
            }
        }

        (head_orientation, shadow_models)
    }

    fn update_particles(&mut self, dt: f32, frustum: &Frustum) {
        let time = self.start_time.elapsed().as_secs_f32();
        let step = 0.05;
        
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
                    
                    let particle_start_time = time - (rocket.trail_time - t);
                    self.smoke_particles.push(SmokeParticle::new(spawn_pos, particle_start_time));
                    
                    t += step;
                }
            }

            let flame_texture = ((rocket.trail_time * 20.0) as u32) % 3;
            let exhaust_dir = -rocket.velocity.normalize();
            let flame_pos = rocket.position + exhaust_dir * 0.15;
            self.flame_particles.push(FlameParticle::new(flame_pos, flame_texture));
        }

        for particle in &mut self.smoke_particles {
            if !particle.update(dt, time) {
                continue;
            }
        }
        self.smoke_particles.retain(|p| {
            let elapsed = time - p.start_time;
            elapsed < p.max_lifetime
        });

        for particle in &mut self.flame_particles {
            if let Some(rocket) = self.rockets.iter().find(|r| r.active && (r.position - particle.position).length() < 2.0) {
                particle.update(dt, rocket.velocity);
            } else {
                particle.lifetime += dt;
            }
        }
        self.flame_particles.retain(|p| p.lifetime < p.max_lifetime);
    }
}

impl ApplicationHandler for GameApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title("SAS2 MVP - WGPU")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        let mut wgpu_renderer = WgpuRenderer::new(window.clone()).block_on().unwrap();
        let mut md3_renderer =
            MD3Renderer::new(wgpu_renderer.device.clone(), wgpu_renderer.queue.clone());

        self.player_model.lower = Self::load_model_part(&[
            "q3-resources/models/players/sarge/lower.md3",
            "../q3-resources/models/players/sarge/lower.md3",
        ]);
        self.player_model.upper = Self::load_model_part(&[
            "q3-resources/models/players/sarge/upper.md3",
            "../q3-resources/models/players/sarge/upper.md3",
        ]);
        self.player_model.head = Self::load_model_part(&[
            "q3-resources/models/players/sarge/head.md3",
            "../q3-resources/models/players/sarge/head.md3",
        ]);
        self.player_model.weapon = Self::load_model_part(&[
            "q3-resources/models/weapons2/rocketl/rocketl.md3",
            "../q3-resources/models/weapons2/rocketl/rocketl.md3",
        ]);

        self.player2_model.lower = Self::load_model_part(&[
            "q3-resources/models/players/orbb/lower.md3",
            "../q3-resources/models/players/orbb/lower.md3",
        ]);
        self.player2_model.upper = Self::load_model_part(&[
            "q3-resources/models/players/orbb/upper.md3",
            "../q3-resources/models/players/orbb/upper.md3",
        ]);
        self.player2_model.head = Self::load_model_part(&[
            "q3-resources/models/players/orbb/head.md3",
            "../q3-resources/models/players/orbb/head.md3",
        ]);

        self.rocket_model = Self::load_model_part(&[
            "q3-resources/models/ammo/rocket/rocket.md3",
            "../q3-resources/models/ammo/rocket/rocket.md3",
        ]);

        self.player_model.anim_config = AnimConfig::load("sarge").ok();
        self.player2_model.anim_config = AnimConfig::load("orbb").ok();

        let surface_format = wgpu_renderer.surface_config.format;
        md3_renderer.create_pipeline(surface_format);

        if let Some(ref lower) = self.player_model.lower {
            self.player_model.lower_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, lower, "sarge", "lower");
        }
        if let Some(ref upper) = self.player_model.upper {
            self.player_model.upper_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, upper, "sarge", "upper");
        }
        if let Some(ref head) = self.player_model.head {
            self.player_model.head_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, head, "sarge", "head");
        }
        if let Some(ref weapon) = self.player_model.weapon {
            self.player_model.weapon_textures =
                load_weapon_textures_static(&mut wgpu_renderer, &mut md3_renderer, weapon);
        }

        if let Some(ref lower) = self.player2_model.lower {
            self.player2_model.lower_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, lower, "orbb", "lower");
        }
        if let Some(ref upper) = self.player2_model.upper {
            self.player2_model.upper_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, upper, "orbb", "upper");
        }
        if let Some(ref head) = self.player2_model.head {
            self.player2_model.head_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, head, "orbb", "head");
        }

        if let Some(ref rocket) = self.rocket_model {
            self.rocket_textures =
                load_rocket_textures_static(&mut wgpu_renderer, &mut md3_renderer, rocket);
        }

        self.window = Some(window.clone());
        self.wgpu_renderer = Some(wgpu_renderer);
        self.md3_renderer = Some(md3_renderer);
        self.create_depth();
        self.last_frame_time = Instant::now();

        window.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(ref mut wgpu_renderer) = self.wgpu_renderer {
                    wgpu_renderer.resize(size);
                    self.create_depth();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == winit::event::ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    match code {
                        KeyCode::KeyA => self.move_left = pressed,
                        KeyCode::KeyD => self.move_right = pressed,
                        KeyCode::KeyQ => self.camera_move_x_neg = pressed,
                        KeyCode::KeyE => self.camera_move_x_pos = pressed,
                        KeyCode::KeyW => self.camera_move_y_neg = pressed,
                        KeyCode::KeyS => self.camera_move_y_pos = pressed,
                        KeyCode::KeyR => self.camera_move_z_neg = pressed,
                        KeyCode::KeyF => self.camera_move_z_pos = pressed,
                        KeyCode::Space => {
                            if pressed && !self.shoot_pressed {
                                if let Some(ref wgpu_renderer) = self.wgpu_renderer {
                                    let (width, height) = wgpu_renderer.get_viewport_size();
                                    let aspect = width as f32 / height as f32;
                                    let (view_proj, _) = self.camera.get_view_proj(aspect);
                                    self.shoot_rocket(view_proj);
                                }
                            }
                            self.shoot_pressed = pressed;
                        }
                        KeyCode::Escape if pressed => event_loop.exit(),
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(self.last_frame_time).as_secs_f32();
                self.last_frame_time = now;

                self.update_fps_counter(now);
                self.player.update(dt, self.move_left, self.move_right);

                let camera_speed = 20.0;
                if self.camera_move_x_neg {
                    self.camera.x -= camera_speed * dt;
                }
                if self.camera_move_x_pos {
                    self.camera.x += camera_speed * dt;
                }
                if self.camera_move_y_neg {
                    self.camera.y -= camera_speed * dt;
                }
                if self.camera_move_y_pos {
                    self.camera.y += camera_speed * dt;
                }
                if self.camera_move_z_neg {
                    self.camera.z -= camera_speed * dt;
                }
                if self.camera_move_z_pos {
                    self.camera.z += camera_speed * dt;
                }

                let elapsed_time = self.start_time.elapsed().as_secs_f32();
                let lower_frame = self.player_model.lower.as_ref()
                    .map(|lower| Self::calculate_legs_frame(
                        &self.player_model.anim_config,
                        self.player.is_moving,
                        self.player.animation_time,
                        lower
                    ))
                    .unwrap_or(0);

                let upper_frame = self.player_model.upper.as_ref()
                    .map(|upper| Self::calculate_torso_frame(
                        &self.player_model.anim_config,
                        elapsed_time,
                        upper
                    ))
                    .unwrap_or(0);

                let player2_lower_frame = self.player2_model.lower.as_ref()
                    .map(|lower| Self::calculate_legs_frame(
                        &self.player2_model.anim_config,
                        false,
                        0.0,
                        lower
                    ))
                    .unwrap_or(0);

                let player2_upper_frame = self.player2_model.upper.as_ref()
                    .map(|upper| Self::calculate_torso_frame(
                        &self.player2_model.anim_config,
                        elapsed_time,
                        upper
                    ))
                    .unwrap_or(0);

                let (width, height) = if let Some(ref wgpu_renderer) = self.wgpu_renderer {
                    wgpu_renderer.get_viewport_size()
                } else {
                    return;
                };
                let aspect = width as f32 / height as f32;
                let (view_proj, _) = self.camera.get_view_proj(aspect);
                let frustum = Frustum::from_view_proj(view_proj);

                for rocket in &mut self.rockets {
                    rocket.update(dt, &frustum);
                }
                self.rockets.retain(|r| r.active);

                self.update_particles(dt, &frustum);

                let player_model = &self.player_model;
                let player2_model = &self.player2_model;
                let rocket_model = self.rocket_model.as_ref();

                let (wgpu_renderer, md3_renderer) =
                    match (self.wgpu_renderer.as_mut(), self.md3_renderer.as_mut()) {
                        (Some(w), Some(m)) => (w, m),
                        _ => return,
                    };

                let frame = match wgpu_renderer.begin_frame() {
                    Some(f) => f,
                    None => {
                        if let Some(ref window) = self.window {
                            window.request_redraw();
                        }
                        return;
                    }
                };
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    wgpu_renderer
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Game Encoder"),
                        });

                let depth_view = self.depth_view.as_ref().unwrap();
                {
                    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Clear Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.05,
                                    g: 0.05,
                                    b: 0.08,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        ..Default::default()
                    });
                }

                let frame_start = Instant::now();
                
                let (width, height) = wgpu_renderer.get_viewport_size();
                let aspect = width as f32 / height as f32;

                let (view_proj, camera_pos) = self.camera.get_view_proj(aspect);
                let frustum = Frustum::from_view_proj(view_proj);

                let mut lighting = LightingParams::new();
                let time = self.start_time.elapsed().as_secs_f32();
                
                let mut dynamic_lights = Vec::new();
                
                for rocket in &self.rockets {
                    if !rocket.is_visible(&frustum) {
                        continue;
                    }
                    
                    let flame_color = Vec3::new(3.5, 2.0, 0.8);
                    dynamic_lights.push(Light::with_randomized_flicker(
                        rocket.position,
                        flame_color,
                        10.0,
                        4.0,
                        4.3,
                    ));
                    
                    let flame_offset = if rocket.velocity.x > 0.0 { -0.8 } else { 0.8 };
                    let flame_pos = rocket.position + Vec3::new(flame_offset, 0.0, 0.0);
                    let flash_color = Vec3::new(4.0, 2.5, 1.0);
                    dynamic_lights.push(Light::with_randomized_flicker(
                        flame_pos,
                        flash_color,
                        6.0,
                        20.0,
                        0.4,
                    ));
                }
                
                let static_lights: Vec<(Vec3, Vec3, f32)> = lighting.lights.iter()
                    .map(|l| (l.position, l.get_color_at_time(time), l.radius))
                    .collect();
                
                let dynamic_lights_data: Vec<(Vec3, Vec3, f32)> = dynamic_lights.iter()
                    .map(|l| (l.position, l.get_color_at_time(time), l.radius))
                    .collect();
                
                let mut all_lights = static_lights.clone();
                all_lights.extend(dynamic_lights_data.iter().copied());

                md3_renderer.render_ground(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    &all_lights,
                    lighting.ambient,
                );

                md3_renderer.render_wall(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    &all_lights,
                    lighting.ambient,
                );

                let scale = 0.04;
                let scale_mat = Mat4::from_scale(Vec3::splat(scale));
                let surface_format = wgpu_renderer.surface_config.format;

                let correction = Mat3::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                let facing_angle = if self.player.facing_right {
                    0.0
                } else {
                    std::f32::consts::PI
                };
                let game_rotation_y = Mat3::from_rotation_y(facing_angle);
                let md3_rotation = game_rotation_y * correction;
                let lower_axis = axis_from_mat3(md3_rotation);
                let md3_lower_orientation = Orientation {
                    origin: Vec3::ZERO,
                    axis: lower_axis,
                };
                let ground_y = -1.4;
                let model_bottom_offset = 0.9;
                let player_y = ground_y + model_bottom_offset;
                let game_translation = Mat4::from_translation(Vec3::new(self.player.x, player_y, 0.0));
                let game_rotation = Mat4::from_mat3(game_rotation_y);
                let game_transform = game_translation * game_rotation;

                let (_, mut shadow_models) = Self::render_player(
                    &mut encoder,
                    &view,
                    depth_view,
                    md3_renderer,
                    surface_format,
                    player_model,
                    game_transform,
                    scale_mat,
                    md3_lower_orientation,
                    lower_frame,
                    upper_frame,
                    view_proj,
                    camera_pos,
                    &all_lights,
                    lighting.ambient,
                    true,
                );

                let player2_game_rotation_y = Mat3::from_rotation_y(std::f32::consts::PI);
                let player2_md3_rotation = player2_game_rotation_y * correction;
                let player2_lower_axis = axis_from_mat3(player2_md3_rotation);
                let player2_md3_lower_orientation = Orientation {
                    origin: Vec3::ZERO,
                    axis: player2_lower_axis,
                };
                let ground_y = -0.9;
                let model_bottom_offset = 0.9;
                let player2_y = ground_y + model_bottom_offset;
                let player2_game_translation = Mat4::from_translation(Vec3::new(10.0, player2_y, 0.0));
                let player2_game_rotation = Mat4::from_mat3(player2_game_rotation_y);
                let player2_game_transform = player2_game_translation * player2_game_rotation;

                let (_, player2_shadow_models) = Self::render_player(
                    &mut encoder,
                    &view,
                    depth_view,
                    md3_renderer,
                    surface_format,
                    player2_model,
                    player2_game_transform,
                    scale_mat,
                    player2_md3_lower_orientation,
                    player2_lower_frame,
                    player2_upper_frame,
                    view_proj,
                    camera_pos,
                    &all_lights,
                    lighting.ambient,
                    false,
                );
                shadow_models.extend(player2_shadow_models);



                if let Some(rocket_model) = rocket_model {
                    for rocket in &self.rockets {
                        if !rocket.active || !rocket.is_visible(&frustum) {
                            continue;
                        }
                        
                        let rocket_scale = 0.05;
                        let correction = Mat3::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                        let rocket_rotation = Mat3::from_rotation_y(
                            if rocket.velocity.x > 0.0 { 0.0 } else { std::f32::consts::PI }
                        ) * correction;
                        
                        let translation = Mat4::from_translation(rocket.position);
                        let rotation = Mat4::from_mat3(rocket_rotation);
                        let scale_mat = Mat4::from_scale(Vec3::splat(rocket_scale));
                        let model_mat = translation * rotation * scale_mat;
                        
                        md3_renderer.render_model(
                            &mut encoder,
                            &view,
                            depth_view,
                            surface_format,
                            rocket_model,
                            0,
                            &self.rocket_textures,
                            model_mat,
                            view_proj,
                            camera_pos,
                            &all_lights,
                            lighting.ambient,
                            false,
                        );
                    }
                }

                let smoke_particles: Vec<(Vec3, f32, f32)> = self.smoke_particles.iter()
                    .map(|p| (p.position, p.size, p.get_alpha()))
                    .collect();
                
                md3_renderer.render_particles(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    &smoke_particles,
                );

                let flame_particles: Vec<(Vec3, f32, u32)> = self.flame_particles.iter()
                    .map(|p| (p.position, p.size, p.texture_index))
                    .collect();
                
                md3_renderer.render_flames(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    &flame_particles,
                );

                md3_renderer.render_wall_shadows_batch(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    &static_lights,
                    lighting.ambient,
                    &shadow_models,
                );


                let render_time = frame_start.elapsed();
                
                wgpu_renderer.queue.submit(Some(encoder.finish()));
                wgpu_renderer.end_frame(frame);
                
                let total_time = frame_start.elapsed();
                if self.frame_count % 60 == 0 {
                    println!("Frame timing: render={:.2}ms, total={:.2}ms, submit={:.2}ms", 
                        render_time.as_secs_f64() * 1000.0,
                        total_time.as_secs_f64() * 1000.0,
                        (total_time - render_time).as_secs_f64() * 1000.0);
                }

                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = GameApp::new();
    event_loop.run_app(&mut app).unwrap();
}
