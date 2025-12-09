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
use test_md3_standalone::loader::{load_textures_for_model_static, load_weapon_textures_static};
use test_md3_standalone::math::{axis_from_mat3, attach_rotated_entity, orientation_to_mat4, Orientation};
use test_md3_standalone::md3::MD3Model;
use test_md3_standalone::renderer::{MD3Renderer, WgpuRenderer};


struct Light {
    position: Vec3,
    color: Vec3,
    radius: f32,
}

impl Light {
    fn new(position: Vec3, color: Vec3, radius: f32) -> Self {
        Self {
            position,
            color,
            radius,
        }
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
                Light::new(Vec3::new(-30.0, 3.0, 12.0), Vec3::new(1.6, 1.6, 1.7), 35.0),
                Light::new(Vec3::new(9.0, 0.0, 0.0), Vec3::new(20.5, 2.5, 2.5), 5.0),
            ],
            ambient: 0.00015,
        }
    }
}

struct Camera {
    distance: f32,
    height: f32,
}

impl Camera {
    fn new() -> Self {
        Self {
            distance: 20.0,
            height: 2.0,
        }
    }

    fn get_view_proj(&self, aspect: f32) -> (Mat4, Vec3) {
        let camera_pos = Vec3::new(0.0, self.height, self.distance);
        let camera_target = Vec3::new(0.0, self.height * 0.5, 0.0);
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
    velocity: Vec3,
    lifetime: f32,
    active: bool,
}

impl Rocket {
    fn new(position: Vec3, direction: Vec3, speed: f32) -> Self {
        Self {
            position,
            velocity: direction.normalize() * speed,
            lifetime: 0.0,
            active: true,
        }
    }

    fn update(&mut self, dt: f32) {
        if !self.active {
            return;
        }

        self.position += self.velocity * dt;
        self.lifetime += dt;

        if self.lifetime > 10.0 {
            self.active = false;
        }
    }
}


struct GameApp {
    window: Option<Arc<Window>>,
    wgpu_renderer: Option<WgpuRenderer>,
    md3_renderer: Option<MD3Renderer>,
    player_lower: Option<MD3Model>,
    player_upper: Option<MD3Model>,
    player_head: Option<MD3Model>,
    weapon: Option<MD3Model>,
    player2_lower: Option<MD3Model>,
    player2_upper: Option<MD3Model>,
    player2_head: Option<MD3Model>,
    rocket_model: Option<MD3Model>,
    anim_config: Option<AnimConfig>,
    player2_anim_config: Option<AnimConfig>,
    lower_textures: Vec<Option<String>>,
    upper_textures: Vec<Option<String>>,
    head_textures: Vec<Option<String>>,
    weapon_textures: Vec<Option<String>>,
    player2_lower_textures: Vec<Option<String>>,
    player2_upper_textures: Vec<Option<String>>,
    player2_head_textures: Vec<Option<String>>,
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
}

impl GameApp {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            window: None,
            wgpu_renderer: None,
            md3_renderer: None,
            player_lower: None,
            player_upper: None,
            player_head: None,
            weapon: None,
            player2_lower: None,
            player2_upper: None,
            player2_head: None,
            rocket_model: None,
            anim_config: None,
            player2_anim_config: None,
            lower_textures: Vec::new(),
            upper_textures: Vec::new(),
            head_textures: Vec::new(),
            weapon_textures: Vec::new(),
            player2_lower_textures: Vec::new(),
            player2_upper_textures: Vec::new(),
            player2_head_textures: Vec::new(),
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
        }
    }

    fn create_depth(&mut self) {
        if let Some(ref wgpu_renderer) = self.wgpu_renderer {
            let (width, height) = wgpu_renderer.get_viewport_size();
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

    fn calculate_animation_frame(&self, is_moving: bool, animation_time: f32, model: &MD3Model) -> usize {
        if let Some(ref config) = self.anim_config {
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

    fn calculate_torso_frame(&self, model: &MD3Model) -> usize {
        if let Some(ref config) = self.anim_config {
            let anim = &config.torso_stand;
            let frame_in_anim = if anim.looping_frames > 0 {
                ((self.start_time.elapsed().as_secs_f32() * anim.fps as f32) as usize)
                    % anim.looping_frames
            } else {
                0
            };
            let frame = anim.first_frame + frame_in_anim;
            frame.min(model.header.num_bone_frames as usize - 1)
        } else {
            0
        }
    }

    fn shoot_rocket(&mut self) {
        println!("ROCKET SHOT! Position: ({:.2}, {:.2}, {:.2}), Direction: ({:.2}, {:.2}, {:.2}), Total rockets: {}", 
            self.player.x, 0.5, 0.0,
            if self.player.facing_right { 1.0 } else { -1.0 }, 0.0, 0.0,
            self.rockets.len() + 1);
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
        self.rockets.push(Rocket::new(rocket_position, direction, 35.0));
        println!("Rocket created. Active rockets: {}", self.rockets.len());
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

        self.player_lower = Self::load_model_part(&[
            "q3-resources/models/players/sarge/lower.md3",
            "../q3-resources/models/players/sarge/lower.md3",
        ]);
        self.player_upper = Self::load_model_part(&[
            "q3-resources/models/players/sarge/upper.md3",
            "../q3-resources/models/players/sarge/upper.md3",
        ]);
        self.player_head = Self::load_model_part(&[
            "q3-resources/models/players/sarge/head.md3",
            "../q3-resources/models/players/sarge/head.md3",
        ]);
        self.weapon = Self::load_model_part(&[
            "q3-resources/models/weapons2/rocketl/rocketl.md3",
            "../q3-resources/models/weapons2/rocketl/rocketl.md3",
        ]);

        self.player2_lower = Self::load_model_part(&[
            "q3-resources/models/players/orbb/lower.md3",
            "../q3-resources/models/players/orbb/lower.md3",
        ]);
        self.player2_upper = Self::load_model_part(&[
            "q3-resources/models/players/orbb/upper.md3",
            "../q3-resources/models/players/orbb/upper.md3",
        ]);
        self.player2_head = Self::load_model_part(&[
            "q3-resources/models/players/orbb/head.md3",
            "../q3-resources/models/players/orbb/head.md3",
        ]);

        self.rocket_model = Self::load_model_part(&[
            "q3-resources/models/ammo/rocket/rocket.md3",
            "../q3-resources/models/ammo/rocket/rocket.md3",
        ]);

        self.anim_config = AnimConfig::load("sarge").ok();
        self.player2_anim_config = AnimConfig::load("orbb").ok();

        let surface_format = wgpu_renderer.surface_config.format;
        md3_renderer.create_pipeline(surface_format);

        if let Some(ref lower) = self.player_lower {
            self.lower_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, lower, "sarge", "lower");
        }
        if let Some(ref upper) = self.player_upper {
            self.upper_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, upper, "sarge", "upper");
        }
        if let Some(ref head) = self.player_head {
            self.head_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, head, "sarge", "head");
        }
        if let Some(ref weapon) = self.weapon {
            self.weapon_textures =
                load_weapon_textures_static(&mut wgpu_renderer, &mut md3_renderer, weapon);
        }

        if let Some(ref lower) = self.player2_lower {
            self.player2_lower_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, lower, "orbb", "lower");
        }
        if let Some(ref upper) = self.player2_upper {
            self.player2_upper_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, upper, "orbb", "upper");
        }
        if let Some(ref head) = self.player2_head {
            self.player2_head_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, head, "orbb", "head");
        }

        if let Some(ref rocket) = self.rocket_model {
            for mesh in &rocket.meshes {
                let shader_name = std::str::from_utf8(&mesh.header.name)
                    .unwrap_or("")
                    .trim_end_matches('\0');
                
                let texture_paths = vec![
                    format!("../q3-resources/models/ammo/rocket/{}.png", shader_name),
                    format!("../q3-resources/models/ammo/rocket/{}.jpg", shader_name),
                ];
                
                let mut texture_loaded = false;
                for texture_path in texture_paths {
                    if std::path::Path::new(&texture_path).exists() {
                        if let Ok(data) = std::fs::read(&texture_path) {
                            if let Ok(img) = image::load_from_memory(&data) {
                                let img = img.to_rgba8();
                                let size = wgpu::Extent3d {
                                    width: img.width(),
                                    height: img.height(),
                                    depth_or_array_layers: 1,
                                };
                                let texture = wgpu_renderer.device.create_texture(&wgpu::TextureDescriptor {
                                    label: Some("Rocket Texture"),
                                    size,
                                    mip_level_count: 1,
                                    sample_count: 1,
                                    dimension: wgpu::TextureDimension::D2,
                                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                                    view_formats: &[],
                                });

                                wgpu_renderer.queue.write_texture(
                                    wgpu::ImageCopyTexture {
                                        texture: &texture,
                                        mip_level: 0,
                                        origin: wgpu::Origin3d::ZERO,
                                        aspect: wgpu::TextureAspect::All,
                                    },
                                    &img,
                                    wgpu::ImageDataLayout {
                                        offset: 0,
                                        bytes_per_row: Some(4 * img.width()),
                                        rows_per_image: Some(img.height()),
                                    },
                                    size,
                                );

                                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                                let sampler = wgpu_renderer.device.create_sampler(&wgpu::SamplerDescriptor {
                                    address_mode_u: wgpu::AddressMode::Repeat,
                                    address_mode_v: wgpu::AddressMode::Repeat,
                                    address_mode_w: wgpu::AddressMode::Repeat,
                                    mag_filter: wgpu::FilterMode::Linear,
                                    min_filter: wgpu::FilterMode::Linear,
                                    mipmap_filter: wgpu::FilterMode::Linear,
                                    ..Default::default()
                                });

                                let wgpu_tex = test_md3_standalone::renderer::WgpuTexture {
                                    texture,
                                    view,
                                    sampler,
                                };

                                md3_renderer.load_texture(&texture_path, wgpu_tex);
                                self.rocket_textures.push(Some(texture_path));
                                texture_loaded = true;
                                break;
                            }
                        }
                    }
                }
                if !texture_loaded {
                    self.rocket_textures.push(None);
                }
            }
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
                        KeyCode::KeyA | KeyCode::ArrowLeft => self.move_left = pressed,
                        KeyCode::KeyD | KeyCode::ArrowRight => self.move_right = pressed,
                        KeyCode::Space => {
                            if pressed && !self.shoot_pressed {
                                self.shoot_rocket();
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

                for rocket in &mut self.rockets {
                    rocket.update(dt);
                }
                self.rockets.retain(|r| r.active);

                let lower_frame = self.player_lower.as_ref()
                    .map(|lower| self.calculate_animation_frame(self.player.is_moving, self.player.animation_time, lower))
                    .unwrap_or(0);

                let upper_frame = self.player_upper.as_ref()
                    .map(|upper| self.calculate_torso_frame(upper))
                    .unwrap_or(0);

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

                let camera = Camera::new();
                let (view_proj, camera_pos) = camera.get_view_proj(aspect);

                let mut lighting = LightingParams::new();
                
                for rocket in &self.rockets {
                    let flame_color = Vec3::new(3.5, 2.0, 0.8);
                    lighting.lights.push(Light::new(rocket.position, flame_color, 12.0));
                    
                    let flame_offset = if rocket.velocity.x > 0.0 { -1.2 } else { 1.2 };
                    let flame_pos = rocket.position + Vec3::new(flame_offset, 0.0, 0.0);
                    let flash_color = Vec3::new(4.0, 2.5, 1.0);
                    lighting.lights.push(Light::new(flame_pos, flash_color, 8.0));
                }
                
                let lights: Vec<(Vec3, Vec3, f32)> = lighting.lights.iter()
                    .map(|l| (l.position, l.color, l.radius))
                    .collect();

                md3_renderer.render_ground(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    &lights,
                    lighting.ambient,
                );

                md3_renderer.render_wall(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    &lights,
                    lighting.ambient,
                );

                let correction = Mat3::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                let facing_angle = if self.player.facing_right {
                    0.0
                } else {
                    std::f32::consts::PI
                };
                let game_rotation_y = Mat3::from_rotation_y(facing_angle);
                let md3_rotation = game_rotation_y * correction;
                let lower_axis = axis_from_mat3(md3_rotation);
                
                let md3_lower_origin = Vec3::ZERO;
                let md3_lower_orientation = Orientation {
                    origin: md3_lower_origin,
                    axis: lower_axis,
                };

                let scale = 0.04;
                let scale_mat = Mat4::from_scale(Vec3::splat(scale));
                let ground_y = -1.50;
                let model_bottom_offset = 0.9;
                let player_y = ground_y + model_bottom_offset;
                let game_translation = Mat4::from_translation(Vec3::new(self.player.x, player_y, 0.0));
                let game_rotation = Mat4::from_mat3(game_rotation_y);
                let game_transform = game_translation * game_rotation;

                let surface_format = wgpu_renderer.surface_config.format;

                let mut upper_orientation = md3_lower_orientation;
                let mut head_orientation: Option<Orientation> = None;
                let mut weapon_orientation: Option<Orientation> = None;

                if let Some(ref lower) = self.player_lower {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&md3_lower_orientation);
                    let model_mat = game_transform * md3_model_mat;
                    md3_renderer.render_model(
                        &mut encoder,
                        &view,
                        depth_view,
                        surface_format,
                        lower,
                        lower_frame,
                        &self.lower_textures,
                        model_mat,
                        view_proj,
                        camera_pos,
                        &lights,
                        lighting.ambient,
                        true,
                    );

                    if let Some(tags) = lower.tags.get(lower_frame) {
                        if let Some(torso_tag) = tags.iter().find(|t| {
                            let name = std::str::from_utf8(&t.name).unwrap_or("");
                            name.trim_end_matches('\0') == "tag_torso"
                        }) {
                            upper_orientation =
                                attach_rotated_entity(&md3_lower_orientation, torso_tag);
                        }
                    }
                }

                if let Some(ref upper) = self.player_upper {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&upper_orientation);
                    let model_mat = game_transform * md3_model_mat;
                    md3_renderer.render_model(
                        &mut encoder,
                        &view,
                        depth_view,
                        surface_format,
                        upper,
                        upper_frame,
                        &self.upper_textures,
                        model_mat,
                        view_proj,
                        camera_pos,
                        &lights,
                        lighting.ambient,
                        true,
                    );

                    if let Some(tags) = upper.tags.get(upper_frame) {
                        if let Some(head_tag) = tags.iter().find(|t| {
                            let name = std::str::from_utf8(&t.name).unwrap_or("");
                            name.trim_end_matches('\0') == "tag_head"
                        }) {
                            head_orientation =
                                Some(attach_rotated_entity(&upper_orientation, head_tag));
                        }
                        
                        if let Some(weapon_tag) = tags.iter().find(|t| {
                            let name = std::str::from_utf8(&t.name).unwrap_or("");
                            name.trim_end_matches('\0') == "tag_weapon"
                        }) {
                            weapon_orientation =
                                Some(attach_rotated_entity(&upper_orientation, weapon_tag));
                        }
                    }
                }

                if let (Some(ref head), Some(head_orient)) =
                    (&self.player_head, head_orientation)
                {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&head_orient);
                    let model_mat = game_transform * md3_model_mat;
                    md3_renderer.render_model(
                        &mut encoder,
                        &view,
                        depth_view,
                        surface_format,
                        head,
                        0,
                        &self.head_textures,
                        model_mat,
                        view_proj,
                        camera_pos,
                        &lights,
                        lighting.ambient,
                        true,
                    );
                }

                if let (Some(ref weapon), Some(weapon_orient)) =
                    (&self.weapon, weapon_orientation)
                {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&weapon_orient);
                    let model_mat = game_transform * md3_model_mat;
                    md3_renderer.render_model(
                        &mut encoder,
                        &view,
                        depth_view,
                        surface_format,
                        weapon,
                        0,
                        &self.weapon_textures,
                        model_mat,
                        view_proj,
                        camera_pos,
                        &lights,
                        lighting.ambient,
                        true,
                    );
                }

                let player2_game_rotation_y = Mat3::from_rotation_y(std::f32::consts::PI);
                let player2_md3_rotation = player2_game_rotation_y * correction;
                let player2_lower_axis = axis_from_mat3(player2_md3_rotation);
                
                let player2_md3_lower_origin = Vec3::ZERO;
                let player2_md3_lower_orientation = Orientation {
                    origin: player2_md3_lower_origin,
                    axis: player2_lower_axis,
                };
                
                let ground_y = -1.50;
                let model_bottom_offset = 0.6;
                let player2_y = ground_y + model_bottom_offset;
                let player2_game_translation = Mat4::from_translation(Vec3::new(10.0, player2_y, 0.0));
                let player2_game_rotation = Mat4::from_mat3(player2_game_rotation_y);
                let player2_game_transform = player2_game_translation * player2_game_rotation;

                let mut player2_upper_orientation = player2_md3_lower_orientation;
                let mut player2_head_orientation: Option<Orientation> = None;

                if let Some(ref lower) = self.player2_lower {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&player2_md3_lower_orientation);
                    let model_mat = player2_game_transform * md3_model_mat;
                    md3_renderer.render_model(
                        &mut encoder,
                        &view,
                        depth_view,
                        surface_format,
                        lower,
                        0,
                        &self.player2_lower_textures,
                        model_mat,
                        view_proj,
                        camera_pos,
                        &lights,
                        lighting.ambient,
                        true,
                    );

                    if let Some(tags) = lower.tags.get(0) {
                        if let Some(torso_tag) = tags.iter().find(|t| {
                            let name = std::str::from_utf8(&t.name).unwrap_or("");
                            name.trim_end_matches('\0') == "tag_torso"
                        }) {
                            player2_upper_orientation =
                                attach_rotated_entity(&player2_md3_lower_orientation, torso_tag);
                        }
                    }
                }

                if let Some(ref upper) = self.player2_upper {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&player2_upper_orientation);
                    let model_mat = player2_game_transform * md3_model_mat;
                    md3_renderer.render_model(
                        &mut encoder,
                        &view,
                        depth_view,
                        surface_format,
                        upper,
                        0,
                        &self.player2_upper_textures,
                        model_mat,
                        view_proj,
                        camera_pos,
                        &lights,
                        lighting.ambient,
                        true,
                    );

                    if let Some(tags) = upper.tags.get(0) {
                        if let Some(head_tag) = tags.iter().find(|t| {
                            let name = std::str::from_utf8(&t.name).unwrap_or("");
                            name.trim_end_matches('\0') == "tag_head"
                        }) {
                            player2_head_orientation =
                                Some(attach_rotated_entity(&player2_upper_orientation, head_tag));
                        }
                    }
                }

                if let (Some(ref head), Some(head_orient)) =
                    (&self.player2_head, player2_head_orientation)
                {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&head_orient);
                    let model_mat = player2_game_transform * md3_model_mat;
                    md3_renderer.render_model(
                        &mut encoder,
                        &view,
                        depth_view,
                        surface_format,
                        head,
                        0,
                        &self.player2_head_textures,
                        model_mat,
                        view_proj,
                        camera_pos,
                        &lights,
                        lighting.ambient,
                        true,
                    );
                }


                let mut shadow_models = Vec::new();

                if let Some(ref lower) = self.player_lower {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&md3_lower_orientation);
                    let model_mat = game_transform * md3_model_mat;
                    shadow_models.push((
                        lower,
                        lower_frame,
                        self.lower_textures.as_slice(),
                        model_mat,
                    ));
                }

                if let Some(ref upper) = self.player_upper {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&upper_orientation);
                    let model_mat = game_transform * md3_model_mat;
                    shadow_models.push((
                        upper,
                        upper_frame,
                        self.upper_textures.as_slice(),
                        model_mat,
                    ));
                }

                if let (Some(ref head), Some(head_orient)) =
                    (&self.player_head, head_orientation)
                {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&head_orient);
                    let model_mat = game_transform * md3_model_mat;
                    shadow_models.push((
                        head,
                        0,
                        self.head_textures.as_slice(),
                        model_mat,
                    ));
                }

                if let (Some(ref weapon), Some(weapon_orient)) =
                    (&self.weapon, weapon_orientation)
                {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&weapon_orient);
                    let model_mat = game_transform * md3_model_mat;
                    shadow_models.push((
                        weapon,
                        0,
                        self.weapon_textures.as_slice(),
                        model_mat,
                    ));
                }

                if let Some(ref lower) = self.player2_lower {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&player2_md3_lower_orientation);
                    let model_mat = player2_game_transform * md3_model_mat;
                    shadow_models.push((
                        lower,
                        0,
                        self.player2_lower_textures.as_slice(),
                        model_mat,
                    ));
                }

                if let Some(ref upper) = self.player2_upper {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&player2_upper_orientation);
                    let model_mat = player2_game_transform * md3_model_mat;
                    shadow_models.push((
                        upper,
                        0,
                        self.player2_upper_textures.as_slice(),
                        model_mat,
                    ));
                }

                if let (Some(ref head), Some(head_orient)) =
                    (&self.player2_head, player2_head_orientation)
                {
                    let md3_model_mat = scale_mat * orientation_to_mat4(&head_orient);
                    let model_mat = player2_game_transform * md3_model_mat;
                    shadow_models.push((
                        head,
                        0,
                        self.player2_head_textures.as_slice(),
                        model_mat,
                    ));
                }

                if let Some(ref rocket_model) = self.rocket_model {
                    for rocket in &self.rockets {
                        if !rocket.active {
                            continue;
                        }
                        
                        let rocket_scale = 0.15;
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
                            &lights,
                            lighting.ambient,
                            false,
                        );
                    }
                }

                md3_renderer.render_wall_shadows_batch(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    &lights,
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
