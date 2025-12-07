use std::collections::HashMap;
use std::sync::Arc;
use wgpu::*;
use wgpu::util::DeviceExt;
use winit::window::Window;
use glam::{Mat4, Vec3};
use bytemuck::{Pod, Zeroable};
use crate::md3::MD3Model;
use crate::shaders::{MD3_SHADER, GROUND_SHADER, SHADOW_SHADER};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexData {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub normal: [f32; 3],
}

impl VertexData {
    pub fn desc() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<VertexData>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x3,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() + std::mem::size_of::<[f32; 2]>()) as BufferAddress,
                    shader_location: 2,
                    format: VertexFormat::Float32x4,
                },
                VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() + std::mem::size_of::<[f32; 2]>() + std::mem::size_of::<[f32; 4]>()) as BufferAddress,
                    shader_location: 3,
                    format: VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MD3Uniforms {
    pub view_proj: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
    pub light_pos0: [f32; 4],
    pub light_color0: [f32; 4],
    pub light_radius0: f32,
    pub _padding0: [f32; 3],
    pub light_pos1: [f32; 4],
    pub light_color1: [f32; 4],
    pub light_radius1: f32,
    pub num_lights: i32,
    pub ambient_light: f32,
    pub _padding1: f32,
    pub _padding2: [f32; 4],
}

pub struct WgpuTexture {
    pub texture: Texture,
    pub view: TextureView,
    pub sampler: Sampler,
}

pub struct WgpuRenderer {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
}

impl WgpuRenderer {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let size = window.inner_size();
        
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())
            .map_err(|e| format!("Failed to create surface: {:?}", e))?;

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        Ok(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            surface,
            surface_config,
            size,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    pub fn begin_frame(&mut self) -> Option<SurfaceTexture> {
        self.surface.get_current_texture().ok()
    }

    pub fn end_frame(&mut self, frame: SurfaceTexture) {
        frame.present();
    }

    pub fn get_viewport_size(&self) -> (u32, u32) {
        (self.size.width, self.size.height)
    }
}

pub struct MD3Renderer {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub pipeline: Option<RenderPipeline>,
    pub ground_pipeline: Option<RenderPipeline>,
    pub shadow_pipeline: Option<RenderPipeline>,
    pub uniform_buffer: Option<Buffer>,
    pub bind_group_layout: BindGroupLayout,
    pub ground_bind_group_layout: BindGroupLayout,
    pub model_textures: HashMap<String, WgpuTexture>,
    pub ground_vertex_buffer: Option<Buffer>,
    pub ground_index_buffer: Option<Buffer>,
}

impl MD3Renderer {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("MD3 Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(256),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let ground_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Ground Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        Self {
            device,
            queue,
            pipeline: None,
            ground_pipeline: None,
            shadow_pipeline: None,
            uniform_buffer: None,
            bind_group_layout,
            ground_bind_group_layout,
            model_textures: HashMap::new(),
            ground_vertex_buffer: None,
            ground_index_buffer: None,
        }
    }

    pub fn load_texture(&mut self, path: &str, texture: WgpuTexture) {
        self.model_textures.insert(path.to_string(), texture);
    }

    pub fn create_pipeline(&mut self, surface_format: TextureFormat) {
        let shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("MD3 Shader"),
            source: ShaderSource::Wgsl(MD3_SHADER.into()),
        });

        let pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("MD3 Pipeline Layout"),
            bind_group_layouts: &[&self.bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("MD3 Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[VertexData::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        self.pipeline = Some(pipeline);

        let ground_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Ground Shader"),
            source: ShaderSource::Wgsl(GROUND_SHADER.into()),
        });

        let ground_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Ground Pipeline Layout"),
            bind_group_layouts: &[&self.ground_bind_group_layout],
            push_constant_ranges: &[],
        });

        let ground_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Ground Pipeline"),
            layout: Some(&ground_pipeline_layout),
            vertex: VertexState {
                module: &ground_shader,
                entry_point: "vs_main",
                buffers: &[VertexData::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &ground_shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        self.ground_pipeline = Some(ground_pipeline);

        let shadow_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shadow Shader"),
            source: ShaderSource::Wgsl(SHADOW_SHADER.into()),
        });

        let shadow_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[&self.bind_group_layout],
            push_constant_ranges: &[],
        });

        let shadow_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Shadow Pipeline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: VertexState {
                module: &shadow_shader,
                entry_point: "vs_main",
                buffers: &[VertexData::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shadow_shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        self.shadow_pipeline = Some(shadow_pipeline);

        let ground_size = 500.0;
        let ground_y = -1.5;
        let ground_vertices = vec![
            VertexData {
                position: [-ground_size, ground_y, -ground_size],
                uv: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
            },
            VertexData {
                position: [ground_size, ground_y, -ground_size],
                uv: [1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
            },
            VertexData {
                position: [ground_size, ground_y, ground_size],
                uv: [1.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
            },
            VertexData {
                position: [-ground_size, ground_y, ground_size],
                uv: [0.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
            },
        ];
        let ground_indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];

        let ground_vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ground Vertex Buffer"),
            contents: bytemuck::cast_slice(&ground_vertices),
            usage: BufferUsages::VERTEX,
        });

        let ground_index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ground Index Buffer"),
            contents: bytemuck::cast_slice(&ground_indices),
            usage: BufferUsages::INDEX,
        });

        self.ground_vertex_buffer = Some(ground_vertex_buffer);
        self.ground_index_buffer = Some(ground_index_buffer);
    }

    fn create_buffers(&self, model: &MD3Model, mesh_idx: usize, frame_idx: usize) -> Option<(Buffer, Buffer, u32)> {
        if mesh_idx >= model.meshes.len() {
            return None;
        }
        
        let mesh = &model.meshes[mesh_idx];
        if frame_idx >= mesh.vertices.len() {
            return None;
        }
        
        let frame_vertices = &mesh.vertices[frame_idx];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for (i, vertex) in frame_vertices.iter().enumerate() {
            let vertex_data = vertex.vertex;
            let x = vertex_data[0] as f32 * (1.0 / 64.0);
            let y = vertex_data[1] as f32 * (1.0 / 64.0);
            let z = vertex_data[2] as f32 * (1.0 / 64.0);

            let normal_encoded = vertex.normal;
            let lat = ((normal_encoded >> 8) & 0xFF) as f32 * 2.0 * std::f32::consts::PI / 255.0;
            let lng = (normal_encoded & 0xFF) as f32 * 2.0 * std::f32::consts::PI / 255.0;
            let nx = lat.cos() * lng.sin();
            let ny = lat.sin() * lng.sin();
            let nz = lng.cos();

            let tex_coord = if i < mesh.tex_coords.len() {
                mesh.tex_coords[i].coord
            } else {
                [0.0, 0.0]
            };

            vertices.push(VertexData {
                position: [x, y, z],
                uv: [tex_coord[0], tex_coord[1]],
                color: [1.0, 1.0, 1.0, 1.0], // White color - texture will provide the actual color
                normal: [nx, ny, nz],
            });
        }

        for triangle in &mesh.triangles {
            indices.push(triangle.vertex[0] as u16);
            indices.push(triangle.vertex[1] as u16);
            indices.push(triangle.vertex[2] as u16);
        }
        
        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MD3 Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX,
        });
        
        let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MD3 Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: BufferUsages::INDEX,
        });
        
        let num_indices = indices.len() as u32;
        
        Some((vertex_buffer, index_buffer, num_indices))
    }

    pub fn render_ground(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        camera_pos: Vec3,
        light_pos0: Vec3,
        light_color0: Vec3,
        light_radius0: f32,
        light_pos1: Vec3,
        light_color1: Vec3,
        light_radius1: f32,
        num_lights: i32,
        ambient_light: f32,
    ) {
            let uniforms = MD3Uniforms {
                view_proj: view_proj.to_cols_array_2d(),
                model: Mat4::IDENTITY.to_cols_array_2d(),
                camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 0.0],
                light_pos0: [light_pos0.x, light_pos0.y, light_pos0.z, 0.0],
                light_color0: [light_color0.x, light_color0.y, light_color0.z, 0.0],
                light_radius0,
                _padding0: [0.0; 3],
                light_pos1: [light_pos1.x, light_pos1.y, light_pos1.z, 0.0],
                light_color1: [light_color1.x, light_color1.y, light_color1.z, 0.0],
                light_radius1,
                num_lights,
                ambient_light,
                _padding1: 0.0,
            _padding2: [0.0, 0.0, 0.0, 0.0],
            };

        if self.uniform_buffer.is_none() {
            self.uniform_buffer = Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("MD3 Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            }));
        } else {
            self.queue.write_buffer(
                self.uniform_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&[uniforms]),
            );
        }

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Ground Bind Group"),
            layout: &self.ground_bind_group_layout,
            entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: self.uniform_buffer.as_ref().unwrap().as_entire_binding(),
                        },
            ],
        });

        let pipeline = self.ground_pipeline.as_ref().unwrap();
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Ground Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.ground_vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.set_index_buffer(self.ground_index_buffer.as_ref().unwrap().slice(..), IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..1);
    }

    pub fn render_model(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        surface_format: TextureFormat,
        model: &MD3Model,
        frame_idx: usize,
        texture_paths: &[Option<String>],
        model_matrix: Mat4,
        view_proj: Mat4,
        camera_pos: Vec3,
        light_pos0: Vec3,
        light_color0: Vec3,
        light_radius0: f32,
        light_pos1: Vec3,
        light_color1: Vec3,
        light_radius1: f32,
        num_lights: i32,
        ambient_light: f32,
        render_shadow: bool,
    ) {
        if self.pipeline.is_none() {
            self.create_pipeline(surface_format);
        }

        let model_array = model_matrix.to_cols_array_2d();
        
        let uniforms = MD3Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
            model: model_array,
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 0.0],
            light_pos0: [light_pos0.x, light_pos0.y, light_pos0.z, 0.0],
            light_color0: [light_color0.x, light_color0.y, light_color0.z, 0.0],
            light_radius0,
            _padding0: [0.0; 3],
            light_pos1: [light_pos1.x, light_pos1.y, light_pos1.z, 0.0],
            light_color1: [light_color1.x, light_color1.y, light_color1.z, 0.0],
            light_radius1,
            num_lights,
            ambient_light,
            _padding1: 0.0,
            _padding2: [0.0, 0.0, 0.0, 0.0],
        };

        let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MD3 Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let shadow_uniforms = MD3Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
            model: model_array,
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 0.0],
            light_pos0: [light_pos0.x, light_pos0.y, light_pos0.z, 0.0],
            light_color0: [light_color0.x, light_color0.y, light_color0.z, 0.0],
            light_radius0,
            _padding0: [0.0; 3],
            light_pos1: [light_pos1.x, light_pos1.y, light_pos1.z, 0.0],
            light_color1: [light_color1.x, light_color1.y, light_color1.z, 0.0],
            light_radius1,
            num_lights,
            ambient_light,
            _padding1: 0.0,
            _padding2: [0.0, 0.0, 0.0, 0.0],
        };

        let shadow_uniform_buffer = if render_shadow {
            Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Shadow Uniform Buffer"),
                contents: bytemuck::cast_slice(&[shadow_uniforms]),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            }))
        } else {
            None
        };

        struct MeshRenderData {
            vertex_buffer: Buffer,
            index_buffer: Buffer,
            num_indices: u32,
            bind_group: BindGroup,
            shadow_bind_group: Option<BindGroup>,
        }

        let mut mesh_data = Vec::new();

        for (mesh_idx, _mesh) in model.meshes.iter().enumerate() {
            let (vertex_buffer, index_buffer, num_indices) = match self.create_buffers(model, mesh_idx, frame_idx) {
                Some(buffers) => buffers,
                None => continue,
            };
            
            let texture_path = texture_paths.get(mesh_idx).and_then(|p| p.as_ref().map(|s| s.as_str()));
            let texture = texture_path.and_then(|path| {
                let mut alt_paths = vec![
                    path.to_string(),
                    format!("../{}", path),
                    path.replace("../", ""),
                ];
                
                if path.ends_with(".tga") {
                    alt_paths.push(path.replace(".tga", ".png"));
                    alt_paths.push(path.replace(".tga", ".jpg"));
                    alt_paths.push(format!("../{}", path.replace(".tga", ".png")));
                    alt_paths.push(format!("../{}", path.replace(".tga", ".jpg")));
                } else if path.ends_with(".png") {
                    alt_paths.push(path.replace(".png", ".tga"));
                    alt_paths.push(path.replace(".png", ".jpg"));
                    alt_paths.push(format!("../{}", path.replace(".png", ".tga")));
                    alt_paths.push(format!("../{}", path.replace(".png", ".jpg")));
                } else if path.ends_with(".jpg") {
                    alt_paths.push(path.replace(".jpg", ".tga"));
                    alt_paths.push(path.replace(".jpg", ".png"));
                    alt_paths.push(format!("../{}", path.replace(".jpg", ".tga")));
                    alt_paths.push(format!("../{}", path.replace(".jpg", ".png")));
                }
                
                for alt_path in &alt_paths {
                    if let Some(tex) = self.model_textures.get(alt_path) {
                        return Some(tex);
                    }
                }
                
                println!("Warning: texture not found in HashMap for path: {:?}", path);
                println!("Tried paths: {:?}", alt_paths);
                println!("Available texture keys: {:?}", self.model_textures.keys().collect::<Vec<_>>());
                None
            });

            if let Some(texture) = texture {
                let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                    label: Some("MD3 Bind Group"),
                    layout: &self.bind_group_layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: uniform_buffer.as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::TextureView(&texture.view),
                        },
                        BindGroupEntry {
                            binding: 2,
                            resource: BindingResource::Sampler(&texture.sampler),
                        },
                    ],
                });

                let shadow_bind_group = if render_shadow {
                    Some(self.device.create_bind_group(&BindGroupDescriptor {
                        label: Some("Shadow Bind Group"),
                        layout: &self.bind_group_layout,
                        entries: &[
                            BindGroupEntry {
                                binding: 0,
                                resource: shadow_uniform_buffer.as_ref().unwrap().as_entire_binding(),
                            },
                            BindGroupEntry {
                                binding: 1,
                                resource: BindingResource::TextureView(&texture.view),
                            },
                            BindGroupEntry {
                                binding: 2,
                                resource: BindingResource::Sampler(&texture.sampler),
                            },
                        ],
                    }))
                } else {
                    None
                };

                mesh_data.push(MeshRenderData {
                    vertex_buffer,
                    index_buffer,
                    num_indices,
                    bind_group,
                    shadow_bind_group,
                });
            }
        }

        let pipeline = self.pipeline.as_ref().unwrap();
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("MD3 Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(pipeline);
        
        for mesh in &mesh_data {
            render_pass.set_bind_group(0, &mesh.bind_group, &[]);
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), IndexFormat::Uint16);
            render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }

        drop(render_pass);

        if render_shadow {
            let shadow_pipeline = self.shadow_pipeline.as_ref().unwrap();
            let mut shadow_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Shadow Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            shadow_pass.set_pipeline(shadow_pipeline);

            for mesh in &mesh_data {
                if let Some(ref shadow_bind_group) = mesh.shadow_bind_group {
                    shadow_pass.set_bind_group(0, shadow_bind_group, &[]);
                    shadow_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    shadow_pass.set_index_buffer(mesh.index_buffer.slice(..), IndexFormat::Uint16);
                    shadow_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                }
            }
        }
    }
}
