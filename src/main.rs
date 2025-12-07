mod md3;
mod anim;
mod renderer;
mod shaders;
mod math;
mod loader;

use std::sync::Arc;
use std::time::Instant;

use wgpu::*;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::Window,
    keyboard::{Key, NamedKey, PhysicalKey, KeyCode},
};
use glam::{Mat3, Mat4, Vec3};
use pollster::FutureExt;
use wgpu::Texture;

use crate::md3::MD3Model;
use crate::anim::AnimConfig;
use crate::renderer::{WgpuRenderer, MD3Renderer, WgpuTexture};
use crate::math::{Orientation, axis_from_mat3, orientation_to_mat4, attach_rotated_entity};
use crate::loader::{load_textures_for_model_static, load_weapon_textures_static};

struct MD3TestApp {
    window: Option<Arc<Window>>,
    wgpu_renderer: Option<WgpuRenderer>,
    md3_renderer: Option<MD3Renderer>,
    model: Option<MD3Model>,
    player_lower: Option<MD3Model>,
    player_upper: Option<MD3Model>,
    player_head: Option<MD3Model>,
    weapon: Option<MD3Model>,
    anim_config: Option<AnimConfig>,
    lower_textures: Vec<Option<String>>,
    upper_textures: Vec<Option<String>>,
    head_textures: Vec<Option<String>>,
    weapon_textures: Vec<Option<String>>,
    texture_path: String,
    mesh_texture_paths: Vec<Option<String>>,
    depth_texture: Option<Texture>,
    depth_view: Option<TextureView>,
    yaw: f32,
    pitch: f32,
    roll: f32,
    frame_idx: usize,
    auto_rotate: bool,
    start_time: Instant,
    last_fps_update: Instant,
    frame_count: u32,
    fps: f32,
    light0_pos: Vec3,
    light1_pos: Vec3,
    ambient_light: f32,
    num_lights: i32,
    camera_distance: f32,
    zoom_in_pressed: bool,
    zoom_out_pressed: bool,
    yaw_left_pressed: bool,
    yaw_right_pressed: bool,
    pitch_up_pressed: bool,
    pitch_down_pressed: bool,
    roll_left_pressed: bool,
    roll_right_pressed: bool,
}

impl MD3TestApp {
    fn new() -> Self {
        Self {
            window: None,
            wgpu_renderer: None,
            md3_renderer: None,
            model: None,
            player_lower: None,
            player_upper: None,
            player_head: None,
            weapon: None,
            anim_config: None,
            lower_textures: Vec::new(),
            upper_textures: Vec::new(),
            head_textures: Vec::new(),
            weapon_textures: Vec::new(),
            texture_path: String::new(),
            mesh_texture_paths: Vec::new(),
            depth_texture: None,
            depth_view: None,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            frame_idx: 0,
            auto_rotate: true,
            start_time: Instant::now(),
            last_fps_update: Instant::now(),
            frame_count: 0,
            fps: 0.0,
            light0_pos: Vec3::new(2.0, 1.0, 3.0),
            light1_pos: Vec3::new(-2.0, -1.0, 2.0),
            ambient_light: 0.15,
            num_lights: 1,
            camera_distance: 80.0,
            zoom_in_pressed: false,
            zoom_out_pressed: false,
            yaw_left_pressed: false,
            yaw_right_pressed: false,
            pitch_up_pressed: false,
            pitch_down_pressed: false,
            roll_left_pressed: false,
            roll_right_pressed: false,
        }
    }
    
    fn create_depth(&mut self) {
        if let Some(ref mut wgpu_renderer) = self.wgpu_renderer {
            let (width, height) = wgpu_renderer.get_viewport_size();
            let depth_texture = wgpu_renderer.device.create_texture(&wgpu::TextureDescriptor {
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
    
    fn load_textures_for_model_static(
        wgpu_renderer: &mut WgpuRenderer,
        md3_renderer: &mut MD3Renderer,
        model: &MD3Model,
        model_name: &str,
        part: &str,
    ) -> Vec<Option<String>> {
        let mut texture_paths = Vec::new();
        let mut mesh_texture_map = std::collections::HashMap::new();
        
        let skin_candidates = vec![
            format!("q3-resources/models/players/{}/{}_default.skin", model_name, part),
            format!("../q3-resources/models/players/{}/{}_default.skin", model_name, part),
            format!("q3-resources/models/players/{}/{}.skin", model_name, part),
            format!("../q3-resources/models/players/{}/{}.skin", model_name, part),
        ];
        
        for skin_path in skin_candidates {
            if let Ok(content) = std::fs::read_to_string(&skin_path) {
                println!("Loaded skin file: {}", skin_path);
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with("//") {
                        continue;
                    }
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() == 2 {
                        let mesh_name = parts[0].trim().to_string();
                        let mut texture_path = parts[1].trim().to_string();
                        if !texture_path.is_empty() {
                            if !texture_path.starts_with("q3-resources/") {
                                texture_path = format!("q3-resources/{}", texture_path);
                            }
                            let texture_path_clone = texture_path.clone();
                            mesh_texture_map.insert(mesh_name.clone(), texture_path);
                            println!("  Mesh '{}' -> texture '{}'", mesh_name, texture_path_clone);
                        }
                    }
                }
                break;
            }
        }
        
        for (_mesh_idx, mesh) in model.meshes.iter().enumerate() {
            let mesh_name = std::str::from_utf8(&mesh.header.name)
                .unwrap_or("")
                .trim_end_matches('\0')
                .to_string();
            
            let texture_path = mesh_texture_map.get(&mesh_name)
                .cloned()
                .or_else(|| {
                    let candidates = vec![
                        format!("q3-resources/models/players/{}/{}_{}.tga", model_name, part, mesh_name),
                        format!("q3-resources/models/players/{}/{}_{}.png", model_name, part, mesh_name),
                        format!("q3-resources/models/players/{}/{}_{}.jpg", model_name, part, mesh_name),
                        format!("../q3-resources/models/players/{}/{}_{}.tga", model_name, part, mesh_name),
                        format!("../q3-resources/models/players/{}/{}_{}.png", model_name, part, mesh_name),
                        format!("../q3-resources/models/players/{}/{}_{}.jpg", model_name, part, mesh_name),
                        format!("q3-resources/models/players/{}/{}.tga", model_name, mesh_name),
                        format!("q3-resources/models/players/{}/{}.png", model_name, mesh_name),
                        format!("q3-resources/models/players/{}/{}.jpg", model_name, mesh_name),
                        format!("../q3-resources/models/players/{}/{}.tga", model_name, mesh_name),
                        format!("../q3-resources/models/players/{}/{}.png", model_name, mesh_name),
                        format!("../q3-resources/models/players/{}/{}.jpg", model_name, mesh_name),
                    ];
                    candidates.iter()
                        .find(|p| std::path::Path::new(p).exists())
                        .map(|s| s.to_string())
                });
            
            let mut texture_loaded = false;
            let mut actual_texture_path = None;
            if let Some(ref path) = texture_path {
                let mut alt_paths = vec![
                    path.clone(),
                    format!("../{}", path),
                ];
                if path.ends_with(".png") {
                    alt_paths.push(path.replace(".png", ".jpg"));
                    alt_paths.push(path.replace(".png", ".tga"));
                    alt_paths.push(format!("../{}", path.replace(".png", ".jpg")));
                    alt_paths.push(format!("../{}", path.replace(".png", ".tga")));
                } else if path.ends_with(".jpg") {
                    alt_paths.push(path.replace(".jpg", ".png"));
                    alt_paths.push(path.replace(".jpg", ".tga"));
                    alt_paths.push(format!("../{}", path.replace(".jpg", ".png")));
                    alt_paths.push(format!("../{}", path.replace(".jpg", ".tga")));
                } else if path.ends_with(".tga") {
                    alt_paths.push(path.replace(".tga", ".png"));
                    alt_paths.push(path.replace(".tga", ".jpg"));
                    alt_paths.push(format!("../{}", path.replace(".tga", ".png")));
                    alt_paths.push(format!("../{}", path.replace(".tga", ".jpg")));
                }
                
                for alt_path in alt_paths {
                    if std::path::Path::new(&alt_path).exists() {
                        if let Ok(data) = std::fs::read(&alt_path) {
                            if let Ok(img) = image::load_from_memory(&data) {
                                let img = img.to_rgba8();
                                let size = wgpu::Extent3d {
                                    width: img.width(),
                                    height: img.height(),
                                    depth_or_array_layers: 1,
                                };
                                let texture = wgpu_renderer.device.create_texture(&wgpu::TextureDescriptor {
                                    label: Some("MD3 Texture"),
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

                                let wgpu_tex = WgpuTexture {
                                    texture,
                                    view,
                                    sampler,
                                };

                                md3_renderer.load_texture(&alt_path, wgpu_tex);
                                println!("Loaded texture: {} for mesh: {} (saving as key: {})", alt_path, mesh_name, alt_path);
                                actual_texture_path = Some(alt_path.clone());
                                texture_loaded = true;
                                break;
                            }
                        }
                    }
                }
                if !texture_loaded {
                    println!("Warning: texture not found for mesh: {} (path: {:?})", mesh_name, path);
                }
            } else {
                println!("Warning: no texture path for mesh: {}", mesh_name);
            }
            
            if let Some(ref actual_path) = actual_texture_path {
                println!("Saving texture_path for mesh {}: {}", mesh_name, actual_path);
            } else {
                println!("Warning: no actual_texture_path for mesh: {}", mesh_name);
            }
            texture_paths.push(actual_texture_path);
        }
        
        println!("Total textures loaded: {}/{}", texture_paths.iter().filter(|p| p.is_some()).count(), texture_paths.len());
        texture_paths
    }
    
    fn load_weapon_textures_static(
        wgpu_renderer: &mut WgpuRenderer,
        md3_renderer: &mut MD3Renderer,
        model: &MD3Model,
    ) -> Vec<Option<String>> {
        let mut texture_paths = Vec::new();
        
        let weapon_candidates: Vec<Vec<&str>> = vec![
            vec![
                "q3-resources/models/weapons2/rocketl/rocketl.png",
                "q3-resources/models/weapons2/rocketl/rocketl.jpg",
                "../q3-resources/models/weapons2/rocketl/rocketl.png",
                "../q3-resources/models/weapons2/rocketl/rocketl.jpg",
            ],
            vec![
                "q3-resources/models/weapons2/rocketl/rocketl2.png",
                "q3-resources/models/weapons2/rocketl/rocketl2.jpg",
                "../q3-resources/models/weapons2/rocketl/rocketl2.png",
                "../q3-resources/models/weapons2/rocketl/rocketl2.jpg",
            ],
        ];

        for (_mesh_idx, candidates) in weapon_candidates.iter().take(model.meshes.len()).enumerate() {
            let texture_path = candidates
                .iter()
                .find(|p| std::path::Path::new(p).exists())
                .map(|s| s.to_string());

            if let Some(ref path) = texture_path {
                if let Ok(data) = std::fs::read(path) {
                    if let Ok(img) = image::load_from_memory(&data) {
                        let img = img.to_rgba8();
                        let size = wgpu::Extent3d {
                            width: img.width(),
                            height: img.height(),
                            depth_or_array_layers: 1,
                        };
                        let texture = wgpu_renderer.device.create_texture(&wgpu::TextureDescriptor {
                            label: Some("Weapon Texture"),
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

                        let wgpu_tex = WgpuTexture {
                            texture,
                            view,
                            sampler,
                        };

                        md3_renderer.load_texture(path, wgpu_tex);
                    }
                }
            }

            texture_paths.push(texture_path);
        }
        
        texture_paths
    }
}

impl ApplicationHandler for MD3TestApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = winit::window::Window::default_attributes()
                .with_title("MD3 Test Renderer - WGPU")
                .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            
            let mut wgpu_renderer = WgpuRenderer::new(window.clone()).block_on().unwrap();
            let mut md3_renderer = MD3Renderer::new(
                wgpu_renderer.device.clone(),
                wgpu_renderer.queue.clone(),
            );

            let lower_paths = vec![
                "q3-resources/models/players/sarge/lower.md3",
                "../q3-resources/models/players/sarge/lower.md3",
            ];
            let upper_paths = vec![
                "q3-resources/models/players/sarge/upper.md3",
                "../q3-resources/models/players/sarge/upper.md3",
            ];
            let head_paths = vec![
                "q3-resources/models/players/sarge/head.md3",
                "../q3-resources/models/players/sarge/head.md3",
            ];
            let weapon_paths = vec![
                "q3-resources/models/weapons2/rocketl/rocketl.md3",
                "../q3-resources/models/weapons2/rocketl/rocketl.md3",
            ];
            
            let lower_path = lower_paths.iter().find(|p| std::path::Path::new(p).exists()).copied();
            let upper_path = upper_paths.iter().find(|p| std::path::Path::new(p).exists()).copied();
            let head_path = head_paths.iter().find(|p| std::path::Path::new(p).exists()).copied();
            let weapon_path = weapon_paths.iter().find(|p| std::path::Path::new(p).exists()).copied();
            
            if let Some(path) = lower_path {
                println!("Loading lower: {}", path);
                let model = MD3Model::load(path).unwrap();
                println!("  Lower: {} meshes, {} frames, {} tags", model.meshes.len(), model.header.num_bone_frames, model.tags.len());
                if !model.tags.is_empty() && !model.tags[0].is_empty() {
                    for tag in &model.tags[0] {
                        let name = std::str::from_utf8(&tag.name).unwrap_or("");
                        println!("    Tag: {}", name.trim_end_matches('\0'));
                    }
                }
                self.player_lower = Some(model);
            }
            if let Some(path) = upper_path {
                println!("Loading upper: {}", path);
                let model = MD3Model::load(path).unwrap();
                println!("  Upper: {} meshes, {} frames, {} tags", model.meshes.len(), model.header.num_bone_frames, model.tags.len());
                if !model.tags.is_empty() && !model.tags[0].is_empty() {
                    for tag in &model.tags[0] {
                        let name = std::str::from_utf8(&tag.name).unwrap_or("");
                        println!("    Tag: {}", name.trim_end_matches('\0'));
                    }
                }
                self.player_upper = Some(model);
            }
            if let Some(path) = head_path {
                println!("Loading head: {}", path);
                let model = MD3Model::load(path).unwrap();
                println!("  Head: {} meshes, {} frames, {} tags", model.meshes.len(), model.header.num_bone_frames, model.tags.len());
                self.player_head = Some(model);
            }
            if let Some(path) = weapon_path {
                println!("Loading weapon: {}", path);
                let model = MD3Model::load(path).unwrap();
                println!("  Weapon: {} meshes, {} frames, {} tags", model.meshes.len(), model.header.num_bone_frames, model.tags.len());
                self.weapon = Some(model);
            }
            
            let model_name = "sarge";
            self.anim_config = AnimConfig::load(model_name).ok();
            if self.anim_config.is_some() {
                println!("Loaded animation.cfg");
            } else {
                println!("No animation.cfg found");
            }

            let surface_format = wgpu_renderer.surface_config.format;
            md3_renderer.create_pipeline(surface_format);

            {
                let lower_model = self.player_lower.as_ref();
                let upper_model = self.player_upper.as_ref();
                let head_model = self.player_head.as_ref();
                let weapon_model = self.weapon.as_ref();
                
                if let Some(lower) = lower_model {
                    let textures = load_textures_for_model_static(
                        &mut wgpu_renderer,
                        &mut md3_renderer,
                        lower,
                        model_name,
                        "lower",
                    );
                    self.lower_textures = textures;
                }
                
                if let Some(upper) = upper_model {
                    let textures = load_textures_for_model_static(
                        &mut wgpu_renderer,
                        &mut md3_renderer,
                        upper,
                        model_name,
                        "upper",
                    );
                    self.upper_textures = textures;
                }
                
                if let Some(head) = head_model {
                    let textures = load_textures_for_model_static(
                        &mut wgpu_renderer,
                        &mut md3_renderer,
                        head,
                        model_name,
                        "head",
                    );
                    self.head_textures = textures;
                }
                
                if let Some(weapon) = weapon_model {
                    let textures = load_weapon_textures_static(
                        &mut wgpu_renderer,
                        &mut md3_renderer,
                        weapon,
                    );
                    self.weapon_textures = textures;
                }
            }
            
            self.texture_path.clear();
            window.request_redraw();
            
            self.window = Some(window);
            self.wgpu_renderer = Some(wgpu_renderer);
            self.md3_renderer = Some(md3_renderer);
            
            self.create_depth();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: winit::window::WindowId, event: WindowEvent) {
        if let Some(ref window) = self.window {
            if window.id() != window_id {
                return;
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(ref mut wgpu_renderer) = self.wgpu_renderer {
                    wgpu_renderer.resize(physical_size);
                    // Recreate depth texture on resize
                    self.create_depth();
                }
            }
            WindowEvent::RedrawRequested => {
                self.frame_count += 1;
                let now = Instant::now();
                let elapsed_since_fps_update = now.duration_since(self.last_fps_update).as_secs_f32();
                
                if elapsed_since_fps_update >= 0.5 {
                    self.fps = self.frame_count as f32 / elapsed_since_fps_update;
                    self.frame_count = 0;
                    self.last_fps_update = now;
                    
                    if let Some(ref window) = self.window {
                        window.set_title(&format!("MD3 Test Renderer - WGPU | FPS: {:.1}", self.fps));
                    }
                }
                
                if self.auto_rotate {
                    let elapsed = self.start_time.elapsed().as_secs_f32();
                    self.yaw = elapsed * 0.5;
                }

                if self.zoom_in_pressed {
                    self.camera_distance = (self.camera_distance - 0.5).max(1.0);
                }
                if self.zoom_out_pressed {
                    self.camera_distance = (self.camera_distance + 0.5).min(200.0);
                }

                if self.yaw_left_pressed {
                    self.yaw -= 0.1;
                    self.auto_rotate = false;
                }
                if self.yaw_right_pressed {
                    self.yaw += 0.1;
                    self.auto_rotate = false;
                }
                if self.pitch_up_pressed {
                    self.pitch += 0.1;
                    self.auto_rotate = false;
                }
                if self.pitch_down_pressed {
                    self.pitch -= 0.1;
                    self.auto_rotate = false;
                }
                if self.roll_left_pressed {
                    self.roll -= 0.1;
                    self.auto_rotate = false;
                }
                if self.roll_right_pressed {
                    self.roll += 0.1;
                    self.auto_rotate = false;
                }

                if let (Some(ref mut wgpu_renderer), Some(ref mut md3_renderer)) = 
                    (self.wgpu_renderer.as_mut(), self.md3_renderer.as_mut()) {
                    
                    let frame = match wgpu_renderer.begin_frame() {
                        Some(f) => f,
                        None => {
                            if let Some(ref window) = self.window {
                                window.request_redraw();
                            }
                            return;
                        }
                    };
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

                    let mut encoder = wgpu_renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("MD3 Test Encoder"),
                    });

                    {
                        let depth_view = self.depth_view.as_ref().unwrap();
                        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Clear Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.1,
                                        g: 0.1,
                                        b: 0.15,
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
                            occlusion_query_set: None,
                            timestamp_writes: None,
                        });
                    }

                    let (width, height) = wgpu_renderer.get_viewport_size();
                    let aspect = width as f32 / height as f32;

                    let view_matrix = Mat4::look_at_rh(
                        Vec3::new(0.0, 0.0, self.camera_distance),
                        Vec3::ZERO,
                        Vec3::Y,
                    );

                    let proj_matrix = Mat4::perspective_rh(
                        std::f32::consts::PI / 4.0,
                        aspect,
                        0.1,
                        100.0,
                    );

                    let view_proj = proj_matrix * view_matrix;

                    let correction_rotation = Mat3::from_rotation_x(-std::f32::consts::PI / 2.0);
                    let rotation_y = Mat3::from_rotation_y(self.yaw);
                    let rotation_x = Mat3::from_rotation_x(self.pitch);
                    let rotation_z = Mat3::from_rotation_z(self.roll);
                    let rotation = rotation_y * rotation_x * rotation_z * correction_rotation;

                    let lower_axis = axis_from_mat3(rotation);
                    let lower_orientation = Orientation {
                        origin: Vec3::ZERO,
                        axis: lower_axis,
                    };

                    let camera_pos = Vec3::new(0.0, 0.0, self.camera_distance);
                    let light_pos0 = self.light0_pos;
                    let light_color0 = Vec3::new(3.0, 2.8, 2.6);
                    let light_radius0 = 10.0;
                    let light_pos1 = self.light1_pos;
                    let light_color1 = Vec3::new(1.2, 1.2, 1.8);
                    let light_radius1 = 10.0;

                    let surface_format = wgpu_renderer.surface_config.format;
                    let depth_view = self.depth_view.as_ref().unwrap();

                    let scale = 0.05;
                    let scale_mat = Mat4::from_scale(Vec3::splat(scale));

                    let elapsed = self.start_time.elapsed().as_secs_f32();
                    let lower_frame = if let Some(ref config) = self.anim_config {
                        let anim = &config.legs_idle;
                        let frame_in_anim = if anim.looping_frames > 0 {
                            ((elapsed * anim.fps as f32) as usize) % anim.looping_frames
                        } else {
                            0
                        };
                        anim.first_frame + frame_in_anim
                    } else {
                        self.frame_idx % 191
                    };
                    
                    let upper_frame = if let Some(ref config) = self.anim_config {
                        let anim = &config.torso_stand;
                        let frame_in_anim = if anim.looping_frames > 0 {
                            ((elapsed * anim.fps as f32) as usize) % anim.looping_frames
                        } else {
                            0
                        };
                        anim.first_frame + frame_in_anim
                    } else {
                        self.frame_idx % 153
                    };
                    
                    let mut upper_orientation = lower_orientation;
                    let mut head_orientation_opt: Option<Orientation> = None;
                    let mut weapon_orientation_opt: Option<Orientation> = None;
                    
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
                        self.num_lights,
                        self.ambient_light,
                    );

                    if let Some(ref lower) = self.player_lower {
                        let frame_idx = lower_frame.min(lower.header.num_bone_frames as usize - 1);
                        
                        if self.frame_count % 300 == 0 {
                            for mesh in &lower.meshes {
                                if let Some(frame_verts) = mesh.vertices.get(frame_idx) {
                                    let mut min_z = f32::MAX;
                                    let mut max_z = f32::MIN;
                                    for v in frame_verts {
                                        let z = v.vertex[2] as f32 / 64.0;
                                        min_z = min_z.min(z);
                                        max_z = max_z.max(z);
                                    }
                                    println!("LOWER mesh Z range: {} to {}", min_z, max_z);
                                }
                            }
                        }
                        
                        let lower_model_mat = scale_mat * orientation_to_mat4(&lower_orientation);
                        md3_renderer.render_model(
                            &mut encoder,
                            &view,
                            depth_view,
                            surface_format,
                            lower,
                            frame_idx,
                            &self.lower_textures,
                            lower_model_mat,
                            view_proj,
                            camera_pos,
                            light_pos0,
                            light_color0,
                            light_radius0,
                            light_pos1,
                            light_color1,
                            light_radius1,
                            self.num_lights,
                            self.ambient_light,
                            true,
                        );
                        
                        if let Some(tags) = lower.tags.get(frame_idx) {
                            if let Some(torso_tag) = tags.iter().find(|t| {
                                let name = std::str::from_utf8(&t.name).unwrap_or("");
                                name.trim_end_matches('\0') == "tag_torso"
                            }) {
                                upper_orientation = attach_rotated_entity(
                                    &lower_orientation,
                                    torso_tag,
                                );
                            } else {
                                println!("Warning: tag_torso not found in lower model");
                                upper_orientation = lower_orientation;
                            }
                        } else {
                            upper_orientation = lower_orientation;
                        }
                    }
                    
                    if let Some(ref upper) = self.player_upper {
                        let frame_idx = upper_frame.min(upper.header.num_bone_frames as usize - 1);
                        
                        if self.frame_count % 300 == 0 {
                            for mesh in &upper.meshes {
                                if let Some(frame_verts) = mesh.vertices.get(frame_idx) {
                                    let mut min_z = f32::MAX;
                                    let mut max_z = f32::MIN;
                                    for v in frame_verts {
                                        let z = v.vertex[2] as f32 / 64.0;
                                        min_z = min_z.min(z);
                                        max_z = max_z.max(z);
                                    }
                                    println!("UPPER mesh Z range: {} to {}", min_z, max_z);
                                }
                            }
                        }
                        
                        let upper_orientation_mat = orientation_to_mat4(&upper_orientation);
                        let upper_model_mat = scale_mat * upper_orientation_mat;
                        
                        if self.frame_count % 300 == 0 {
                            println!("=== UPPER DEBUG ===");
                            println!("upper_orientation.origin = {:?}", upper_orientation.origin);
                            println!("upper_orientation.axis[0] = {:?}", upper_orientation.axis[0]);
                            println!("upper_orientation.axis[1] = {:?}", upper_orientation.axis[1]);
                            println!("upper_orientation.axis[2] = {:?}", upper_orientation.axis[2]);
                            println!("upper_orientation_mat translation (w_axis) = {:?}", upper_orientation_mat.w_axis);
                            println!("scale_mat = {:?}", scale_mat);
                            println!("upper_model_mat translation (w_axis) = {:?}", upper_model_mat.w_axis);
                            let mat_array = upper_model_mat.to_cols_array();
                            println!("upper_model_mat[3] (last column) = [{}, {}, {}, {}]", 
                                mat_array[12], mat_array[13], mat_array[14], mat_array[15]);
                            println!("===================");
                        }
                        md3_renderer.render_model(
                            &mut encoder,
                            &view,
                            depth_view,
                            surface_format,
                            upper,
                            frame_idx,
                            &self.upper_textures,
                            upper_model_mat,
                            view_proj,
                            camera_pos,
                            light_pos0,
                            light_color0,
                            light_radius0,
                            light_pos1,
                            light_color1,
                            light_radius1,
                            self.num_lights,
                            self.ambient_light,
                            true,
                        );
                        
                        if let Some(tags) = upper.tags.get(frame_idx) {
                            if let Some(head_tag) = tags.iter().find(|t| {
                                let name = std::str::from_utf8(&t.name).unwrap_or("");
                                name.trim_end_matches('\0') == "tag_head"
                            }) {
                                if self.frame_count % 300 == 0 {
                                    println!("tag_head position: {:?}", head_tag.position);
                                }
                                head_orientation_opt = Some(attach_rotated_entity(
                                    &upper_orientation,
                                    head_tag,
                                ));
                            } else {
                                println!("Warning: tag_head not found in upper model");
                            }
                            
                            if let Some(weapon_tag) = tags.iter().find(|t| {
                                let name = std::str::from_utf8(&t.name).unwrap_or("");
                                name.trim_end_matches('\0') == "tag_weapon"
                            }) {
                                if self.frame_count % 300 == 0 {
                                    println!("tag_weapon position: {:?}", weapon_tag.position);
                                }
                                weapon_orientation_opt = Some(attach_rotated_entity(
                                    &upper_orientation,
                                    weapon_tag,
                                ));
                            } else {
                                println!("Warning: tag_weapon not found in upper model");
                            }
                        }
                    }
                    
                    if let (Some(ref head), Some(head_orientation)) = (self.player_head.as_ref(), head_orientation_opt) {
                        if self.frame_count % 300 == 0 {
                            for mesh in &head.meshes {
                                if let Some(frame_verts) = mesh.vertices.get(0) {
                                    let mut min_z = f32::MAX;
                                    let mut max_z = f32::MIN;
                                    for v in frame_verts {
                                        let z = v.vertex[2] as f32 / 64.0;
                                        min_z = min_z.min(z);
                                        max_z = max_z.max(z);
                                    }
                                    println!("HEAD mesh Z range: {} to {}", min_z, max_z);
                                }
                            }
                        }
                        
                        let head_model_mat = scale_mat * orientation_to_mat4(&head_orientation);
                        md3_renderer.render_model(
                            &mut encoder,
                            &view,
                            depth_view,
                            surface_format,
                            head,
                            0,
                            &self.head_textures,
                            head_model_mat,
                            view_proj,
                            camera_pos,
                            light_pos0,
                            light_color0,
                            light_radius0,
                            light_pos1,
                            light_color1,
                            light_radius1,
                            self.num_lights,
                            self.ambient_light,
                            true,
                        );
                    } else if self.player_head.is_some() {
                        println!("Warning: head model exists but head_matrix not set");
                    }
                    
                    if let (Some(ref weapon), Some(weapon_orientation)) = (self.weapon.as_ref(), weapon_orientation_opt) {
                        let weapon_model_mat = scale_mat * orientation_to_mat4(&weapon_orientation);
                        md3_renderer.render_model(
                            &mut encoder,
                            &view,
                            depth_view,
                            surface_format,
                            weapon,
                            0,
                            &self.weapon_textures,
                            weapon_model_mat,
                            view_proj,
                            camera_pos,
                            light_pos0,
                            light_color0,
                            light_radius0,
                            light_pos1,
                            light_color1,
                            light_radius1,
                            self.num_lights,
                            self.ambient_light,
                            true,
                        );
                    } else if self.weapon.is_some() {
                        println!("Warning: weapon model exists but weapon_matrix not set");
                    }

                    wgpu_renderer.queue.submit(Some(encoder.finish()));
                    wgpu_renderer.end_frame(frame);
                }

                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let is_pressed = event.state == winit::event::ElementState::Pressed;
                
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    match keycode {
                        KeyCode::PageUp => {
                            self.zoom_out_pressed = is_pressed;
                        }
                        KeyCode::PageDown => {
                            self.zoom_in_pressed = is_pressed;
                        }
                        KeyCode::Equal | KeyCode::NumpadAdd => {
                            self.zoom_out_pressed = is_pressed;
                        }
                        KeyCode::Minus | KeyCode::NumpadSubtract => {
                            self.zoom_in_pressed = is_pressed;
                        }
                        _ => {}
                    }
                }
                
                match &event.logical_key {
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.yaw_left_pressed = is_pressed;
                        if is_pressed {
                            self.auto_rotate = false;
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        self.yaw_right_pressed = is_pressed;
                        if is_pressed {
                            self.auto_rotate = false;
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.pitch_up_pressed = is_pressed;
                        if is_pressed {
                            self.auto_rotate = false;
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.pitch_down_pressed = is_pressed;
                        if is_pressed {
                            self.auto_rotate = false;
                        }
                    }
                    Key::Character(c) if c == "q" || c == "Q" => {
                        self.roll_left_pressed = is_pressed;
                        if is_pressed {
                            self.auto_rotate = false;
                        }
                    }
                    Key::Character(c) if c == "e" || c == "E" => {
                        self.roll_right_pressed = is_pressed;
                        if is_pressed {
                            self.auto_rotate = false;
                        }
                    }
                    _ => {}
                }
                
                if is_pressed {
                    match &event.logical_key {
                        Key::Named(NamedKey::Space) => {
                            self.auto_rotate = !self.auto_rotate;
                        }
                        Key::Character(c) if c == "r" || c == "R" => {
                            self.yaw = 0.0;
                            self.pitch = 0.0;
                            self.roll = 0.0;
                            self.light0_pos = Vec3::new(2.0, 1.0, 3.0);
                            self.light1_pos = Vec3::new(-2.0, -1.0, 2.0);
                            self.ambient_light = 0.15;
                            self.num_lights = 1;
                            self.camera_distance = 80.0;
                            println!("Reset: light0={:?}, ambient={}", self.light0_pos, self.ambient_light);
                        }
                        Key::Character(c) if c == "i" || c == "I" => {
                            self.light0_pos.z += 0.2;
                            println!("Light0 pos: {:?}", self.light0_pos);
                        }
                        Key::Character(c) if c == "k" || c == "K" => {
                            self.light0_pos.z -= 0.2;
                            println!("Light0 pos: {:?}", self.light0_pos);
                        }
                        Key::Character(c) if c == "j" || c == "J" => {
                            self.light0_pos.x -= 0.2;
                            println!("Light0 pos: {:?}", self.light0_pos);
                        }
                        Key::Character(c) if c == "l" || c == "L" => {
                            self.light0_pos.x += 0.2;
                            println!("Light0 pos: {:?}", self.light0_pos);
                        }
                        Key::Character(c) if c == "u" || c == "U" => {
                            self.light0_pos.y += 0.2;
                            println!("Light0 pos: {:?}", self.light0_pos);
                        }
                        Key::Character(c) if c == "o" || c == "O" => {
                            self.light0_pos.y -= 0.2;
                            println!("Light0 pos: {:?}", self.light0_pos);
                        }
                        Key::Character(c) if c == "z" || c == "Z" => {
                            self.ambient_light = (self.ambient_light - 0.05).max(0.0);
                            println!("Ambient: {}", self.ambient_light);
                        }
                        Key::Character(c) if c == "x" || c == "X" => {
                            self.ambient_light = (self.ambient_light + 0.05).min(1.0);
                            println!("Ambient: {}", self.ambient_light);
                        }
                        Key::Character(c) if c == "0" => {
                            self.num_lights = 0;
                            println!("Num lights: 0");
                        }
                        Key::Character(c) if c == "1" => {
                            self.num_lights = 1;
                            println!("Num lights: 1");
                        }
                        Key::Character(c) if c == "2" => {
                            self.num_lights = 2;
                            println!("Num lights: 2");
                        }
                        _ => {}
                    }
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
    let mut app = MD3TestApp::new();
    event_loop.run_app(&mut app).unwrap();
}
