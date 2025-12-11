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

use sas2::engine::anim::{AnimConfig, AnimRange};
use sas2::engine::loader::{load_textures_for_model_static, load_weapon_textures_static, load_rocket_textures_static};
use sas2::engine::math::{axis_from_mat3, attach_rotated_entity, orientation_to_mat4, Orientation, Frustum};
use sas2::engine::md3::MD3Model;
use sas2::engine::renderer::{MD3Renderer, WgpuRenderer};

use sas2::game::world::World;
use sas2::game::camera::Camera;
use sas2::game::lighting::{LightingParams, Light};
// use sas2::game::player::Player;
use sas2::game::weapon::Rocket;

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
    
    // World state
    world: World,
    local_player_id: u32,
    
    // Input state
    move_left: bool,
    move_right: bool,
    jump_pressed: bool,
    crouch_pressed: bool,
    shoot_pressed: bool,
    last_shot_time: f32,
    is_shooting: bool,
    shoot_anim_start_time: f32,
    
    player2_gesture_start_time: f32,
    player2_is_gesturing: bool,
    player2_next_gesture_time: f32,
    
    camera: Camera,
    camera_move_x_neg: bool,
    camera_move_x_pos: bool,
    camera_move_y_neg: bool,
    camera_move_y_pos: bool,
    camera_move_z_neg: bool,
    camera_move_z_pos: bool,
}

impl GameApp {
    fn new() -> Self {
        let now = Instant::now();
        let mut world = World::new();
        let local_player_id = world.add_player();
        
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
            
            world,
            local_player_id,
            
            move_left: false,
            move_right: false,
            jump_pressed: false,
            crouch_pressed: false,
            shoot_pressed: false,
            last_shot_time: 0.0,
            is_shooting: false,
            shoot_anim_start_time: 0.0,
            
            player2_gesture_start_time: 0.0,
            player2_is_gesturing: false,
            player2_next_gesture_time: 5.0,
            
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
                let player_x = self.world.players.get(self.local_player_id as usize).map(|p| p.x).unwrap_or(0.0);
                window.set_title(&format!(
                    "SAS2 MVP | FPS: {:.0} | X: {:.1}",
                    self.fps, player_x
                ));
            }
        }
    }

    fn frame_for_anim(anim: &AnimRange, time: f32, model: &MD3Model) -> usize {
        let frames_passed = (time * anim.fps as f32).floor() as usize;
        let max_index = model.header.num_bone_frames as usize;
        if max_index == 0 {
            return 0;
        }
        if anim.looping_frames == 0 {
            let last = anim.num_frames.saturating_sub(1);
            let frame = anim.first_frame + frames_passed.min(last);
            return frame.min(max_index - 1);
        }
        let loop_len = anim.looping_frames.min(anim.num_frames).max(1);
        if frames_passed < anim.num_frames {
            let frame = anim.first_frame + frames_passed;
            return frame.min(max_index - 1);
        }
        let loop_start = anim.first_frame + anim.num_frames.saturating_sub(loop_len);
        let loop_index = (frames_passed - anim.num_frames) % loop_len;
        let frame = loop_start + loop_index;
        frame.min(max_index - 1)
    }

    fn calculate_legs_frame(
        anim_config: &Option<AnimConfig>,
        is_moving: bool,
        animation_time: f32,
        model: &MD3Model,
        state: sas2::game::player::PlayerState,
        _is_crouching: bool,
    ) -> usize {
        use sas2::game::player::PlayerState;
        
        if let Some(ref config) = anim_config {
            let anim = match state {
                PlayerState::Air => &config.legs_jump,
                PlayerState::Crouching => {
                    if is_moving {
                        &config.legs_walkcr
                    } else {
                        &config.legs_idlecr
                    }
                }
                PlayerState::Ground => {
                    if is_moving {
                        &config.legs_run
                    } else {
                        &config.legs_idle
                    }
                }
            };
            return Self::frame_for_anim(anim, animation_time, model);
        } else {
            0
        }
    }

    fn calculate_torso_frame(
        anim_config: &Option<AnimConfig>,
        elapsed_time: f32,
        model: &MD3Model,
        is_shooting: bool,
        shoot_anim_time: f32,
    ) -> usize {
        if let Some(ref config) = anim_config {
            let anim = if is_shooting {
                &config.torso_attack
            } else {
                &config.torso_stand
            };
            let time = if is_shooting { shoot_anim_time } else { elapsed_time };
            return Self::frame_for_anim(anim, time, model);
        } else {
            0
        }
    }

    fn calculate_torso_frame_with_gesture(
        anim_config: &Option<AnimConfig>,
        elapsed_time: f32,
        model: &MD3Model,
        is_gesturing: bool,
        gesture_anim_time: f32,
    ) -> usize {
        if let Some(ref config) = anim_config {
            let anim = if is_gesturing {
                &config.torso_gesture
            } else {
                &config.torso_stand
            };
            let time = if is_gesturing { gesture_anim_time } else { elapsed_time };
            return Self::frame_for_anim(anim, time, model);
        } else {
            0
        }
    }

    fn shoot_rocket(&mut self, view_proj: Mat4, weapon_orientation: Option<Orientation>) {
        let player = match self.world.players.get(self.local_player_id as usize) {
            Some(p) => p,
            None => return,
        };

        let rocket_position = if let Some(weapon_orient) = weapon_orientation {
            let scale = 0.04;
            let facing_angle = if player.facing_right { 0.0 } else { std::f32::consts::PI };
            let game_rotation_y = Mat3::from_rotation_y(facing_angle);
            
            let ground_y = self.world.map.ground_y;
            let model_bottom_offset = 0.9;
            let render_y = ground_y + model_bottom_offset + player.y;
            let game_translation = Mat4::from_translation(Vec3::new(player.x, render_y, 0.0));
            let game_rotation = Mat4::from_mat3(game_rotation_y);
            let game_transform = game_translation * game_rotation;
            let scale_mat = Mat4::from_scale(Vec3::splat(scale));
            
            let barrel_offset = Vec3::new(25.0, 0.0, 5.0);
            let barrel_local_pos = weapon_orient.origin + 
                weapon_orient.axis[0] * barrel_offset.x +
                weapon_orient.axis[1] * barrel_offset.y +
                weapon_orient.axis[2] * barrel_offset.z;
            
            let barrel_scaled = scale_mat.transform_point3(barrel_local_pos);
            let barrel_world = game_transform.transform_point3(barrel_scaled);
            
            barrel_world
        } else {
            let ground_y = self.world.map.ground_y;
            let model_bottom_offset = 0.9;
            let render_y = ground_y + model_bottom_offset + player.y;
            Vec3::new(player.x, render_y + 0.5, 0.0)
        };

        let direction = if player.facing_right {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(-1.0, 0.0, 0.0)
        };
        let frustum = Frustum::from_view_proj(view_proj);
        self.world.rockets.push(Rocket::new(rocket_position, direction, 10.0, &frustum));
        let time = self.start_time.elapsed().as_secs_f32();
        self.last_shot_time = time;
        self.is_shooting = true;
        self.shoot_anim_start_time = time;
    }

    fn find_tag<'a>(tags: &'a [sas2::engine::md3::Tag], name: &str) -> Option<&'a sas2::engine::md3::Tag> {
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
        let mut weapon_orientation_result: Option<Orientation> = None;

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
                false,
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
                false,
            );
            shadow_models.push((upper, upper_frame, player_model.upper_textures.as_slice(), model_mat));

            if let Some(tags) = upper.tags.get(upper_frame) {
                if let Some(head_tag) = Self::find_tag(tags, "tag_head") {
                    head_orientation = Some(attach_rotated_entity(&upper_orientation, head_tag));
                }
                if include_weapon {
                    if let Some(weapon_tag) = Self::find_tag(tags, "tag_weapon") {
                        weapon_orientation_result = Some(attach_rotated_entity(&upper_orientation, weapon_tag));
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
                false,
            );
            shadow_models.push((head, 0, player_model.head_textures.as_slice(), model_mat));
        }

        if include_weapon {
            if let (Some(ref weapon), Some(weapon_orient)) = (&player_model.weapon, weapon_orientation_result) {
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
                    false,
                );
                shadow_models.push((weapon, 0, player_model.weapon_textures.as_slice(), model_mat));
            }
        }

        (weapon_orientation_result, shadow_models)
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
                        KeyCode::KeyW => self.jump_pressed = pressed,
                        KeyCode::KeyS => self.crouch_pressed = pressed,
                        KeyCode::KeyQ => self.camera_move_x_neg = pressed,
                        KeyCode::KeyE => self.camera_move_x_pos = pressed,
                        KeyCode::KeyR => self.camera_move_z_neg = pressed,
                        KeyCode::KeyF => self.camera_move_z_pos = pressed,
                        KeyCode::KeyZ => self.camera_move_y_neg = pressed,
                        KeyCode::KeyX => self.camera_move_y_pos = pressed,
                        KeyCode::Space => {
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

                // Update Camera
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

                // Update World
                let (width, height) = if let Some(ref wgpu_renderer) = self.wgpu_renderer {
                    wgpu_renderer.get_viewport_size()
                } else {
                    return;
                };
                let aspect = width as f32 / height as f32;
                let (view_proj, _camera_pos) = self.camera.get_view_proj(aspect);
                let frustum = Frustum::from_view_proj(view_proj);

                if let Some(player) = self.world.players.get_mut(self.local_player_id as usize) {
                    player.update(dt, self.move_left, self.move_right, self.jump_pressed, self.crouch_pressed, self.world.map.ground_y);
                }
                
                self.world.update(dt, &frustum);

                // Rendering
                let player = match self.world.players.get(self.local_player_id as usize) {
                    Some(p) => p,
                    None => return,
                };
                let player_x = player.x;
                let player_y = player.y;
                let player_facing_right = player.facing_right;
                let player_is_moving = player.is_moving;
                let player_animation_time = player.animation_time;
                let player_state = player.state;
                let player_is_crouching = player.is_crouching;

                let elapsed_time = self.start_time.elapsed().as_secs_f32();
                let lower_frame = self.player_model.lower.as_ref()
                    .map(|lower| Self::calculate_legs_frame(
                        &self.player_model.anim_config,
                        player_is_moving,
                        player_animation_time,
                        lower,
                        player_state,
                        player_is_crouching
                    ))
                    .unwrap_or(0);

                let shoot_anim_time = elapsed_time - self.shoot_anim_start_time;
                if self.is_shooting {
                    if let Some(ref config) = self.player_model.anim_config {
                        let anim_duration = config.torso_attack.num_frames as f32 / config.torso_attack.fps as f32;
                        if shoot_anim_time >= anim_duration {
                            self.is_shooting = false;
                        }
                    }
                }

                let upper_frame = self.player_model.upper.as_ref()
                    .map(|upper| Self::calculate_torso_frame(
                        &self.player_model.anim_config,
                        elapsed_time,
                        upper,
                        self.is_shooting,
                        shoot_anim_time
                    ))
                    .unwrap_or(0);

                if elapsed_time >= self.player2_next_gesture_time && !self.player2_is_gesturing {
                    self.player2_is_gesturing = true;
                    self.player2_gesture_start_time = elapsed_time;
                    self.player2_next_gesture_time = elapsed_time + 5.0 + (elapsed_time.sin() * 3.0).abs();
                }

                if self.player2_is_gesturing {
                    if let Some(ref config) = self.player2_model.anim_config {
                        let gesture_time = elapsed_time - self.player2_gesture_start_time;
                        let gesture_duration = config.torso_gesture.num_frames as f32 / config.torso_gesture.fps as f32;
                        if gesture_time >= gesture_duration {
                            self.player2_is_gesturing = false;
                        }
                    }
                }

                let player2_lower_frame = self.player2_model.lower.as_ref()
                    .map(|lower| Self::calculate_legs_frame(
                        &self.player2_model.anim_config,
                        false,
                        elapsed_time,
                        lower,
                        sas2::game::player::PlayerState::Ground,
                        false
                    ))
                    .unwrap_or(0);

                let player2_gesture_time = elapsed_time - self.player2_gesture_start_time;
                let player2_upper_frame = self.player2_model.upper.as_ref()
                    .map(|upper| Self::calculate_torso_frame_with_gesture(
                        &self.player2_model.anim_config,
                        elapsed_time,
                        upper,
                        self.player2_is_gesturing,
                        player2_gesture_time
                    ))
                    .unwrap_or(0);

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

                // Lighting
                let lighting = LightingParams::new();
                let time = self.start_time.elapsed().as_secs_f32();
                
                let mut dynamic_lights = Vec::new();
                
                for rocket in &self.world.rockets {
                    if !rocket.is_visible(&frustum) {
                        continue;
                    }
                    
                    let flame_color = Vec3::new(3.5, 2.0, 0.8);
                    dynamic_lights.push(Light::with_randomized_flicker(
                        rocket.position,
                        flame_color,
                        10.0,
                        41.0,
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

                // Render Local Player
                let correction = Mat3::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                let facing_angle = if player_facing_right {
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
                let ground_y = self.world.map.ground_y;
                let model_bottom_offset = 0.9;
                let render_y = ground_y + model_bottom_offset + player_y;
                let game_translation = Mat4::from_translation(Vec3::new(player_x, render_y, 0.0));
                let game_rotation = Mat4::from_mat3(game_rotation_y);
                let game_transform = game_translation * game_rotation;

                let (weapon_orientation, mut shadow_models) = Self::render_player(
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

                // Render Player 2 (Static dummy for now, but should ideally come from World)
                // For MVP refactor, keeping it as static dummy
                let player2_game_rotation_y = Mat3::from_rotation_y(std::f32::consts::PI);
                let player2_md3_rotation = player2_game_rotation_y * correction;
                let player2_lower_axis = axis_from_mat3(player2_md3_rotation);
                let player2_md3_lower_orientation = Orientation {
                    origin: Vec3::ZERO,
                    axis: player2_lower_axis,
                };
                let ground_y = self.world.map.ground_y;
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

                let should_shoot = self.shoot_pressed && !self.is_shooting;

                // Render Rockets
                if let Some(rocket_model) = rocket_model {
                    for rocket in &self.world.rockets {
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

                let smoke_particles: Vec<(Vec3, f32, f32)> = self.world.smoke_particles.iter()
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

                let flame_particles: Vec<(Vec3, f32, u32)> = self.world.flame_particles.iter()
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

                let shadow_volume_models: Vec<(&MD3Model, usize, Mat4)> = shadow_models.iter()
                    .map(|(model, frame, _textures, matrix)| (*model, *frame, *matrix))
                    .collect();

                md3_renderer.render_planar_shadows(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    &shadow_volume_models,
                    &all_lights,
                );

                // md3_renderer.render_debug_lights(
                //     &mut encoder,
                //     &view,
                //     depth_view,
                //     view_proj,
                //     camera_pos,
                //     &all_lights,
                //     surface_format,
                // );

                // md3_renderer.render_debug_light_rays(
                //     &mut encoder,
                //     &view,
                //     depth_view,
                //     view_proj,
                //     &all_lights,
                //     surface_format,
                // );

                let render_time = frame_start.elapsed();
                
                wgpu_renderer.queue.submit(Some(encoder.finish()));
                wgpu_renderer.end_frame(frame);
                
                if should_shoot {
                    self.shoot_rocket(view_proj, weapon_orientation);
                }
                
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
