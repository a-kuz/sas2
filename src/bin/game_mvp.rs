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
use test_md3_standalone::loader::load_textures_for_model_static;
use test_md3_standalone::math::{axis_from_mat3, attach_rotated_entity, orientation_to_mat4, Orientation};
use test_md3_standalone::md3::MD3Model;
use test_md3_standalone::renderer::{MD3Renderer, WgpuRenderer};

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
        let accel = 800.0;
        let friction = 12.0;
        let max_speed = 200.0;

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

        if self.vx.abs() < 1.0 {
            self.vx = 0.0;
        }

        self.x += self.vx * dt;

        if self.is_moving {
            self.animation_time += dt;
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
    anim_config: Option<AnimConfig>,
    lower_textures: Vec<Option<String>>,
    upper_textures: Vec<Option<String>>,
    head_textures: Vec<Option<String>>,
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
    camera_height: f32,
}

impl GameApp {
    fn new() -> Self {
        Self {
            window: None,
            wgpu_renderer: None,
            md3_renderer: None,
            player_lower: None,
            player_upper: None,
            player_head: None,
            anim_config: None,
            lower_textures: Vec::new(),
            upper_textures: Vec::new(),
            head_textures: Vec::new(),
            depth_texture: None,
            depth_view: None,
            start_time: Instant::now(),
            last_frame_time: Instant::now(),
            last_fps_update: Instant::now(),
            frame_count: 0,
            fps: 0.0,
            player: Player::new(),
            move_left: false,
            move_right: false,
            camera_height: 1.5,
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
                    format: wgpu::TextureFormat::Depth32Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
            let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.depth_texture = Some(depth_texture);
            self.depth_view = Some(depth_view);
        }
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

        let model_paths = [
            ("lower", vec![
                "q3-resources/models/players/sarge/lower.md3",
                "../q3-resources/models/players/sarge/lower.md3",
            ]),
            ("upper", vec![
                "q3-resources/models/players/sarge/upper.md3",
                "../q3-resources/models/players/sarge/upper.md3",
            ]),
            ("head", vec![
                "q3-resources/models/players/sarge/head.md3",
                "../q3-resources/models/players/sarge/head.md3",
            ]),
        ];

        for (part, paths) in &model_paths {
            let path = paths.iter().find(|p| std::path::Path::new(p).exists());
            if let Some(path) = path {
                println!("Loading {}: {}", part, path);
                if let Ok(model) = MD3Model::load(path) {
                    match *part {
                        "lower" => self.player_lower = Some(model),
                        "upper" => self.player_upper = Some(model),
                        "head" => self.player_head = Some(model),
                        _ => {}
                    }
                }
            }
        }

        self.anim_config = AnimConfig::load("sarge").ok();

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
                        KeyCode::Escape if pressed => event_loop.exit(),
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(self.last_frame_time).as_secs_f32();
                self.last_frame_time = now;

                self.frame_count += 1;
                let fps_elapsed = now.duration_since(self.last_fps_update).as_secs_f32();
                if fps_elapsed >= 0.5 {
                    self.fps = self.frame_count as f32 / fps_elapsed;
                    self.frame_count = 0;
                    self.last_fps_update = now;
                    if let Some(ref window) = self.window {
                        window.set_title(&format!(
                            "SAS2 MVP | FPS: {:.0} | X: {:.0}",
                            self.fps, self.player.x
                        ));
                    }
                }

                self.player.update(dt, self.move_left, self.move_right);

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

                let (width, height) = wgpu_renderer.get_viewport_size();
                let aspect = width as f32 / height as f32;

                let camera_distance = 80.0;
                let camera_x = self.player.x * 0.05;
                let camera_target = Vec3::new(camera_x, self.camera_height, 0.0);
                let camera_pos = Vec3::new(camera_x, self.camera_height, camera_distance);

                let view_matrix = Mat4::look_at_rh(camera_pos, camera_target, Vec3::Y);
                let proj_matrix =
                    Mat4::perspective_rh(std::f32::consts::PI / 4.0, aspect, 0.1, 1000.0);
                let view_proj = proj_matrix * view_matrix;

                let player_world_x = self.player.x * 0.05;
                let player_world_y = 0.0;

                let light_pos0 = Vec3::new(player_world_x + 2.0, 3.0, 4.0);
                let light_color0 = Vec3::new(2.5, 2.3, 2.1);
                let light_radius0 = 15.0;
                let light_pos1 = Vec3::new(player_world_x - 2.0, 1.0, 3.0);
                let light_color1 = Vec3::new(0.8, 0.8, 1.2);
                let light_radius1 = 10.0;
                let ambient = 0.2;
                let num_lights = 2;

                md3_renderer.render_ground(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    light_pos0,
                    light_color0,
                    light_radius0,
                    light_pos1,
                    light_color1,
                    light_radius1,
                    num_lights,
                    ambient,
                );

                let correction = Mat3::from_rotation_x(-std::f32::consts::PI / 2.0);
                let facing_angle = if self.player.facing_right {
                    -std::f32::consts::FRAC_PI_2
                } else {
                    std::f32::consts::FRAC_PI_2
                };
                let rotation = Mat3::from_rotation_y(facing_angle) * correction;
                let lower_axis = axis_from_mat3(rotation);
                let lower_origin = Vec3::new(player_world_x, player_world_y, 0.0);
                let lower_orientation = Orientation {
                    origin: lower_origin,
                    axis: lower_axis,
                };

                let scale = 0.05;
                let scale_mat = Mat4::from_scale(Vec3::splat(scale));

                let lower_frame = if let (Some(ref config), Some(ref lower)) =
                    (&self.anim_config, &self.player_lower)
                {
                    let anim = if self.player.is_moving {
                        &config.legs_run
                    } else {
                        &config.legs_idle
                    };
                    let frame_in_anim = if anim.looping_frames > 0 {
                        ((self.player.animation_time * anim.fps as f32) as usize) % anim.looping_frames
                    } else {
                        0
                    };
                    let frame = anim.first_frame + frame_in_anim;
                    frame.min(lower.header.num_bone_frames as usize - 1)
                } else {
                    0
                };

                let upper_frame = if let (Some(ref config), Some(ref upper)) =
                    (&self.anim_config, &self.player_upper)
                {
                    let anim = &config.torso_stand;
                    let frame_in_anim = if anim.looping_frames > 0 {
                        ((self.start_time.elapsed().as_secs_f32() * anim.fps as f32) as usize)
                            % anim.looping_frames
                    } else {
                        0
                    };
                    let frame = anim.first_frame + frame_in_anim;
                    frame.min(upper.header.num_bone_frames as usize - 1)
                } else {
                    0
                };

                let surface_format = wgpu_renderer.surface_config.format;

                let mut upper_orientation = lower_orientation;
                let mut head_orientation: Option<Orientation> = None;

                if let Some(ref lower) = self.player_lower {
                    let model_mat = scale_mat * orientation_to_mat4(&lower_orientation);
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
                        light_pos0,
                        light_color0,
                        light_radius0,
                        light_pos1,
                        light_color1,
                        light_radius1,
                        num_lights,
                        ambient,
                        true,
                    );

                    if let Some(tags) = lower.tags.get(lower_frame) {
                        if let Some(torso_tag) = tags.iter().find(|t| {
                            let name = std::str::from_utf8(&t.name).unwrap_or("");
                            name.trim_end_matches('\0') == "tag_torso"
                        }) {
                            upper_orientation =
                                attach_rotated_entity(&lower_orientation, torso_tag);
                        }
                    }
                }

                if let Some(ref upper) = self.player_upper {
                    let model_mat = scale_mat * orientation_to_mat4(&upper_orientation);
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
                        light_pos0,
                        light_color0,
                        light_radius0,
                        light_pos1,
                        light_color1,
                        light_radius1,
                        num_lights,
                        ambient,
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
                    }
                }

                if let (Some(ref head), Some(head_orient)) =
                    (&self.player_head, head_orientation)
                {
                    let model_mat = scale_mat * orientation_to_mat4(&head_orient);
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
                        light_pos0,
                        light_color0,
                        light_radius0,
                        light_pos1,
                        light_color1,
                        light_radius1,
                        num_lights,
                        ambient,
                        true,
                    );
                }

                wgpu_renderer.queue.submit(Some(encoder.finish()));
                wgpu_renderer.end_frame(frame);

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
