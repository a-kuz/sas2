use std::sync::Arc;
use std::time::Instant;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

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

use sas2::engine::loader::load_md3_textures_guess_static;
use sas2::engine::md3::MD3Model;
use sas2::engine::renderer::{MD3Renderer, WgpuRenderer};
use sas2::render::TextRenderer;

fn find_all_md3_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let search_paths = [
        "q3-resources",
        "../q3-resources",
        "../../q3-resources",
    ];
    
    for base_path in &search_paths {
        let path = Path::new(base_path);
        if path.exists() {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_file() && entry_path.extension().map(|e| e == "md3").unwrap_or(false) {
                        files.push(entry_path);
                    } else if entry_path.is_dir() {
                        collect_md3_files_recursive(&entry_path, &mut files);
                    }
                }
            }
        }
    }
    
    files.sort();
    files
}

fn collect_md3_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() && entry_path.extension().map(|e| e == "md3").unwrap_or(false) {
                files.push(entry_path);
            } else if entry_path.is_dir() {
                collect_md3_files_recursive(&entry_path, files);
            }
        }
    }
}

struct MD3ViewerApp {
    window: Option<Arc<Window>>,
    wgpu_renderer: Option<WgpuRenderer>,
    md3_renderer: Option<MD3Renderer>,
    text_renderer: Option<TextRenderer>,
    depth_texture: Option<Texture>,
    depth_view: Option<wgpu::TextureView>,
    
    md3_files: Vec<PathBuf>,
    current_file_index: usize,
    current_model: Option<MD3Model>,
    current_textures: Vec<Option<String>>,
    
    camera_distance: f32,
    camera_yaw: f32,
    camera_pitch: f32,
    
    show_file_list: bool,
    scroll_offset: usize,
    
    start_time: Instant,
    last_frame_time: Instant,
}

impl MD3ViewerApp {
    fn new() -> Self {
        let md3_files = find_all_md3_files();
        println!("Found {} MD3 files", md3_files.len());
        
        Self {
            window: None,
            wgpu_renderer: None,
            md3_renderer: None,
            text_renderer: None,
            depth_texture: None,
            depth_view: None,
            md3_files,
            current_file_index: 0,
            current_model: None,
            current_textures: Vec::new(),
            camera_distance: 100.0,
            camera_yaw: 0.0,
            camera_pitch: 0.3,
            show_file_list: true,
            scroll_offset: 0,
            start_time: Instant::now(),
            last_frame_time: Instant::now(),
        }
    }
    
    fn load_current_model(&mut self) {
        if self.md3_files.is_empty() {
            return;
        }
        
        let file_path = &self.md3_files[self.current_file_index];
        println!("Loading: {}", file_path.display());
        
        if let Some(ref mut md3_renderer) = self.md3_renderer.as_mut() {
            md3_renderer.clear_model_cache();
        }
        
        match MD3Model::load(file_path) {
            Ok(model) => {
                println!("Model loaded: {} meshes, {} frames", model.meshes.len(), model.header.num_bone_frames);
                let (min_x, max_x, min_y, max_y, min_z, max_z) = model.get_bounds(0);
                let size_x = max_x - min_x;
                let size_y = max_y - min_y;
                let size_z = max_z - min_z;
                let max_size = size_x.max(size_y).max(size_z);
                println!("Model bounds: {:.2} x {:.2} x {:.2}", size_x, size_y, size_z);
                
                self.current_model = Some(model.clone());
                
                if max_size > 0.0 {
                    self.camera_distance = max_size * 2.5;
                }
                
                if let (Some(ref mut wgpu_renderer), Some(ref mut md3_renderer)) = 
                    (self.wgpu_renderer.as_mut(), self.md3_renderer.as_mut()) {
                    self.current_textures = load_md3_textures_guess_static(
                        wgpu_renderer,
                        md3_renderer,
                        &model,
                        file_path.to_string_lossy().as_ref(),
                    );
                    println!("Loaded {} textures", self.current_textures.len());
                }
                
                if let Some(ref window) = self.window {
                    let file_name = file_path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    window.set_title(&format!("MD3 Viewer - {}", file_name));
                }
            }
            Err(e) => {
                println!("Failed to load model: {}", e);
                self.current_model = None;
                self.current_textures.clear();
            }
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
    
    fn get_camera_matrix(&self, aspect: f32) -> (Mat4, Vec3) {
        let camera_pos = Vec3::new(
            self.camera_distance * self.camera_yaw.cos() * self.camera_pitch.cos(),
            self.camera_distance * self.camera_yaw.sin() * self.camera_pitch.cos(),
            self.camera_distance * self.camera_pitch.sin(),
        );
        
        let target = Vec3::ZERO;
        let up = Vec3::new(0.0, 0.0, 1.0);
        
        let view = Mat4::look_at_rh(camera_pos, target, up);
        let proj = Mat4::perspective_rh(std::f32::consts::PI / 4.0, aspect, 0.1, 1000.0);
        let view_proj = proj * view;
        
        (view_proj, camera_pos)
    }
}

impl ApplicationHandler for MD3ViewerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        
        let window_attributes = Window::default_attributes()
            .with_title("MD3 Viewer")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        
        let mut wgpu_renderer = WgpuRenderer::new(window.clone()).block_on().unwrap();
        let mut md3_renderer = MD3Renderer::new(
            wgpu_renderer.device.clone(),
            wgpu_renderer.queue.clone(),
        );
        
        md3_renderer.create_pipeline(wgpu_renderer.surface_config.format);
        
        let text_renderer = TextRenderer::new(
            wgpu_renderer.device.clone(),
            wgpu_renderer.queue.clone(),
            wgpu_renderer.surface_config.format,
        );
        
        self.window = Some(window.clone());
        self.wgpu_renderer = Some(wgpu_renderer);
        self.md3_renderer = Some(md3_renderer);
        self.text_renderer = Some(text_renderer);
        self.create_depth();
        self.last_frame_time = Instant::now();
        
        if !self.md3_files.is_empty() {
            self.load_current_model();
        }
        
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
                if !pressed {
                    return;
                }
                
                if let PhysicalKey::Code(code) = event.physical_key {
                    match code {
                        KeyCode::Escape => {
                            if self.show_file_list {
                                event_loop.exit();
                            } else {
                                self.show_file_list = true;
                            }
                        }
                        KeyCode::Tab => {
                            self.show_file_list = !self.show_file_list;
                        }
                        KeyCode::ArrowUp if self.show_file_list => {
                            if self.current_file_index > 0 {
                                self.current_file_index -= 1;
                                if self.current_file_index < self.scroll_offset {
                                    self.scroll_offset = self.current_file_index;
                                }
                                self.load_current_model();
                            }
                        }
                        KeyCode::ArrowDown if self.show_file_list => {
                            if self.current_file_index < self.md3_files.len().saturating_sub(1) {
                                self.current_file_index += 1;
                                let visible_lines = 20;
                                if self.current_file_index >= self.scroll_offset + visible_lines {
                                    self.scroll_offset = self.current_file_index - visible_lines + 1;
                                }
                                self.load_current_model();
                            }
                        }
                        KeyCode::Enter if self.show_file_list => {
                            self.show_file_list = false;
                        }
                        KeyCode::ArrowLeft => {
                            self.camera_yaw -= 0.1;
                        }
                        KeyCode::ArrowRight => {
                            self.camera_yaw += 0.1;
                        }
                        KeyCode::ArrowUp if !self.show_file_list => {
                            self.camera_pitch = (self.camera_pitch + 0.1).min(1.5);
                        }
                        KeyCode::ArrowDown if !self.show_file_list => {
                            self.camera_pitch = (self.camera_pitch - 0.1).max(-1.5);
                        }
                        KeyCode::KeyQ => {
                            self.camera_distance = (self.camera_distance * 1.1).min(500.0);
                        }
                        KeyCode::KeyE => {
                            self.camera_distance = (self.camera_distance / 1.1).max(10.0);
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        self.camera_distance = (self.camera_distance * (1.0 - y * 0.1))
                            .clamp(10.0, 500.0);
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        self.camera_distance = (self.camera_distance * (1.0 - pos.y as f32 * 0.01))
                            .clamp(10.0, 500.0);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let _dt = now.duration_since(self.last_frame_time).as_secs_f32();
                self.last_frame_time = now;
                
                let (width, height) = if let Some(ref wgpu_renderer) = self.wgpu_renderer {
                    wgpu_renderer.get_viewport_size()
                } else {
                    return;
                };
                let aspect = width as f32 / height as f32;
                let (view_proj, camera_pos) = self.get_camera_matrix(aspect);
                
                let (wgpu_renderer, md3_renderer) = match (
                    self.wgpu_renderer.as_mut(),
                    self.md3_renderer.as_mut(),
                ) {
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
                
                let mut encoder = wgpu_renderer
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("MD3 Viewer Encoder"),
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
                        ..Default::default()
                    });
                }
                
                let lights = vec![(
                    Vec3::new(50.0, 50.0, 100.0),
                    Vec3::new(1.0, 1.0, 1.0),
                    200.0,
                )];
                let ambient = 0.3;
                
                if let Some(ref model) = self.current_model {
                    let (min_x, max_x, min_y, max_y, min_z, max_z) = model.get_bounds(0);
                    let center_x = (min_x + max_x) * 0.5;
                    let center_y = (min_y + max_y) * 0.5;
                    let center_z = (min_z + max_z) * 0.5;
                    
                    let size_x = max_x - min_x;
                    let size_y = max_y - min_y;
                    let size_z = max_z - min_z;
                    let max_size = size_x.max(size_y).max(size_z);
                    
                    if max_size > 0.0 && self.camera_distance == 100.0 {
                        self.camera_distance = max_size * 2.5;
                    }
                    
                    let md3_correction = Mat3::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                    let translation = Mat4::from_translation(Vec3::new(-center_x, -center_y, -center_z));
                    let rotation = Mat4::from_mat3(md3_correction);
                    let model_mat = rotation * translation;
                    
                    md3_renderer.render_model(
                        &mut encoder,
                        &view,
                        depth_view,
                        wgpu_renderer.surface_config.format,
                        model,
                        0,
                        &self.current_textures,
                        model_mat,
                        view_proj,
                        camera_pos,
                        &lights,
                        ambient,
                        false,
                    );
                }
                
                if let Some(ref text_renderer) = self.text_renderer {
                    let mut text_encoder = wgpu_renderer.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor {
                            label: Some("Text Encoder"),
                        },
                    );
                    
                    if self.show_file_list {
                        let start_y = 50.0;
                        let line_height = 30.0;
                        let visible_lines = 20;
                        
                        let start_idx = self.scroll_offset;
                        let end_idx = (start_idx + visible_lines).min(self.md3_files.len());
                        
                        for (i, idx) in (start_idx..end_idx).enumerate() {
                            let file_path = &self.md3_files[idx];
                            let file_name = file_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown");
                            
                            let y = start_y + (i as f32 * line_height);
                            let color = if idx == self.current_file_index {
                                [1.0, 1.0, 0.0, 1.0]
                            } else {
                                [0.8, 0.8, 0.8, 1.0]
                            };
                            
                            let display_path = if file_path.to_string_lossy().len() > 80 {
                                format!("...{}", &file_name)
                            } else {
                                file_path.to_string_lossy().to_string()
                            };
                            
                            text_renderer.render_text(
                                &mut text_encoder,
                                &view,
                                &display_path,
                                20.0,
                                y,
                                24.0,
                                color,
                                width,
                                height,
                            );
                        }
                        
                        text_renderer.render_text(
                            &mut text_encoder,
                            &view,
                            &format!("File {}/{}", self.current_file_index + 1, self.md3_files.len()),
                            20.0,
                            height as f32 - 60.0,
                            28.0,
                            [1.0, 1.0, 1.0, 1.0],
                            width,
                            height,
                        );
                        
                        text_renderer.render_text(
                            &mut text_encoder,
                            &view,
                            "Arrow Keys: Navigate | Enter: View | Tab: Toggle List | ESC: Exit",
                            20.0,
                            height as f32 - 30.0,
                            20.0,
                            [0.7, 0.7, 0.7, 1.0],
                            width,
                            height,
                        );
                    } else {
                        if let Some(ref model) = self.current_model {
                            let info_text = format!(
                                "Meshes: {} | Frames: {} | Tags: {}",
                                model.meshes.len(),
                                model.header.num_bone_frames,
                                model.header.num_tags
                            );
                            text_renderer.render_text(
                                &mut text_encoder,
                                &view,
                                &info_text,
                                20.0,
                                30.0,
                                24.0,
                                [1.0, 1.0, 1.0, 1.0],
                                width,
                                height,
                            );
                        }
                        
                        text_renderer.render_text(
                            &mut text_encoder,
                            &view,
                            "Arrow Keys: Rotate Camera | Q/E: Zoom | Tab: Show List | ESC: Exit",
                            20.0,
                            height as f32 - 30.0,
                            20.0,
                            [0.7, 0.7, 0.7, 1.0],
                            width,
                            height,
                        );
                    }
                    
                    wgpu_renderer.queue.submit(Some(text_encoder.finish()));
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
    let mut app = MD3ViewerApp::new();
    event_loop.run_app(&mut app).unwrap();
}
