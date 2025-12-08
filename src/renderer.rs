use std::collections::HashMap;
use std::sync::Arc;
use wgpu::*;
use wgpu::util::DeviceExt;
use winit::window::Window;
use glam::{Mat4, Vec3};
use bytemuck::{Pod, Zeroable};
use crate::md3::MD3Model;
use crate::shaders::{MD3_SHADER, GROUND_SHADER, SHADOW_SHADER, WALL_SHADOW_SHADER, PARTICLE_SHADER, FLAME_SHADER};

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

const MAX_LIGHTS: usize = 8;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LightData {
    pub position: [f32; 4],
    pub color: [f32; 4],
    pub radius: f32,
    pub _padding: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MD3Uniforms {
    pub view_proj: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
    pub lights: [LightData; MAX_LIGHTS],
    pub num_lights: i32,
    pub ambient_light: f32,
    pub _padding: [f32; 2],
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

struct MeshRenderData {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    num_indices: u32,
    bind_group: BindGroup,
    shadow_bind_group: Option<BindGroup>,
}

pub struct MD3Renderer {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub pipeline: Option<RenderPipeline>,
    pub ground_pipeline: Option<RenderPipeline>,
    pub shadow_pipeline: Option<RenderPipeline>,
    pub wall_shadow_pipeline: Option<RenderPipeline>,
    pub particle_pipeline: Option<RenderPipeline>,
    pub flame_pipeline: Option<RenderPipeline>,
    pub uniform_buffer: Option<Buffer>,
    pub bind_group_layout: BindGroupLayout,
    pub ground_bind_group_layout: BindGroupLayout,
    pub particle_bind_group_layout: BindGroupLayout,
    pub model_textures: HashMap<String, WgpuTexture>,
    pub ground_vertex_buffer: Option<Buffer>,
    pub ground_index_buffer: Option<Buffer>,
    pub wall_vertex_buffer: Option<Buffer>,
    pub wall_index_buffer: Option<Buffer>,
}

impl MD3Renderer {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        let bind_group_layout = Self::create_md3_bind_group_layout(&device);
        let ground_bind_group_layout = Self::create_ground_bind_group_layout(&device);
        let particle_bind_group_layout = Self::create_particle_bind_group_layout(&device);

        Self {
            device,
            queue,
            pipeline: None,
            ground_pipeline: None,
            shadow_pipeline: None,
            wall_shadow_pipeline: None,
            particle_pipeline: None,
            flame_pipeline: None,
            uniform_buffer: None,
            bind_group_layout,
            ground_bind_group_layout,
            particle_bind_group_layout,
            model_textures: HashMap::new(),
            ground_vertex_buffer: None,
            ground_index_buffer: None,
            wall_vertex_buffer: None,
            wall_index_buffer: None,
        }
    }

    fn create_md3_bind_group_layout(device: &Device) -> BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("MD3 Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<MD3Uniforms>() as u64),
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
        })
    }

    fn create_ground_bind_group_layout(device: &Device) -> BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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
        })
    }

    fn create_particle_bind_group_layout(device: &Device) -> BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Particle Bind Group Layout"),
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
        })
    }

    pub fn load_texture(&mut self, path: &str, texture: WgpuTexture) {
        self.model_textures.insert(path.to_string(), texture);
    }

    fn create_depth_stencil_state(depth_write_enabled: bool) -> DepthStencilState {
        DepthStencilState {
            format: TextureFormat::Depth24PlusStencil8,
            depth_write_enabled,
            depth_compare: CompareFunction::Less,
            stencil: StencilState::default(),
            bias: DepthBiasState::default(),
        }
    }

    fn create_primitive_state(cull_mode: Option<Face>) -> PrimitiveState {
        PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Cw,
            cull_mode,
            polygon_mode: PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        }
    }

    fn create_multisample_state() -> MultisampleState {
        MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        }
    }

    fn create_color_target_state(surface_format: TextureFormat) -> ColorTargetState {
        ColorTargetState {
            format: surface_format,
            blend: Some(BlendState::ALPHA_BLENDING),
            write_mask: ColorWrites::ALL,
        }
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
                targets: &[Some(Self::create_color_target_state(surface_format))],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: Self::create_primitive_state(Some(Face::Back)),
            depth_stencil: Some(Self::create_depth_stencil_state(true)),
            multisample: Self::create_multisample_state(),
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
                targets: &[Some(Self::create_color_target_state(surface_format))],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: Self::create_primitive_state(None),
            depth_stencil: Some(Self::create_depth_stencil_state(true)),
            multisample: Self::create_multisample_state(),
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

        let shadow_blend_state = BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
        };

        let shadow_color_target = ColorTargetState {
            format: surface_format,
            blend: Some(shadow_blend_state),
            write_mask: ColorWrites::ALL,
        };

        let shadow_depth_stencil = DepthStencilState {
            format: TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: false,
            depth_compare: CompareFunction::Less,
            stencil: StencilState {
                front: StencilFaceState {
                    compare: CompareFunction::Equal,
                    fail_op: StencilOperation::Keep,
                    depth_fail_op: StencilOperation::Keep,
                    pass_op: StencilOperation::IncrementClamp,
                },
                back: StencilFaceState {
                    compare: CompareFunction::Equal,
                    fail_op: StencilOperation::Keep,
                    depth_fail_op: StencilOperation::Keep,
                    pass_op: StencilOperation::IncrementClamp,
                },
                read_mask: 0xff,
                write_mask: 0xff,
            },
            bias: DepthBiasState::default(),
        };

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
                targets: &[Some(shadow_color_target.clone())],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: Self::create_primitive_state(None),
            depth_stencil: Some(shadow_depth_stencil),
            multisample: Self::create_multisample_state(),
            multiview: None,
        });

        self.shadow_pipeline = Some(shadow_pipeline);

        let wall_shadow_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Wall Shadow Shader"),
            source: ShaderSource::Wgsl(WALL_SHADOW_SHADER.into()),
        });

        let wall_shadow_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Wall Shadow Pipeline Layout"),
            bind_group_layouts: &[&self.bind_group_layout],
            push_constant_ranges: &[],
        });

        let wall_shadow_depth_stencil = DepthStencilState {
            format: TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: false,
            depth_compare: CompareFunction::Less,
            stencil: StencilState {
                front: StencilFaceState {
                    compare: CompareFunction::Equal,
                    fail_op: StencilOperation::Keep,
                    depth_fail_op: StencilOperation::Keep,
                    pass_op: StencilOperation::IncrementClamp,
                },
                back: StencilFaceState {
                    compare: CompareFunction::Equal,
                    fail_op: StencilOperation::Keep,
                    depth_fail_op: StencilOperation::Keep,
                    pass_op: StencilOperation::IncrementClamp,
                },
                read_mask: 0xff,
                write_mask: 0xff,
            },
            bias: DepthBiasState::default(),
        };

        let wall_shadow_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Wall Shadow Pipeline"),
            layout: Some(&wall_shadow_pipeline_layout),
            vertex: VertexState {
                module: &wall_shadow_shader,
                entry_point: "vs_main",
                buffers: &[VertexData::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &wall_shadow_shader,
                entry_point: "fs_main",
                targets: &[Some(shadow_color_target.clone())],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: Self::create_primitive_state(None),
            depth_stencil: Some(wall_shadow_depth_stencil),
            multisample: Self::create_multisample_state(),
            multiview: None,
        });

        self.wall_shadow_pipeline = Some(wall_shadow_pipeline);

        let ground_size = 500.0;
        let ground_y = -1.50;        let ground_vertices = vec![
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

        let wall_size = 500.0;
        let wall_height = 50.0;
        let wall_z = -3.0;
        let wall_vertices = vec![
            VertexData {
                position: [-wall_size, -1.5, wall_z],
                uv: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
            },
            VertexData {
                position: [wall_size, -1.5, wall_z],
                uv: [1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
            },
            VertexData {
                position: [wall_size, wall_height, wall_z],
                uv: [1.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
            },
            VertexData {
                position: [-wall_size, wall_height, wall_z],
                uv: [0.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
            },
        ];
        let wall_indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];

        let wall_vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Wall Vertex Buffer"),
            contents: bytemuck::cast_slice(&wall_vertices),
            usage: BufferUsages::VERTEX,
        });

        let wall_index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Wall Index Buffer"),
            contents: bytemuck::cast_slice(&wall_indices),
            usage: BufferUsages::INDEX,
        });

        self.wall_vertex_buffer = Some(wall_vertex_buffer);
        self.wall_index_buffer = Some(wall_index_buffer);

        let particle_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Particle Shader"),
            source: ShaderSource::Wgsl(PARTICLE_SHADER.into()),
        });

        let particle_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[&self.particle_bind_group_layout],
            push_constant_ranges: &[],
        });

        let particle_blend_state = BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
        };

        let particle_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Particle Pipeline"),
            layout: Some(&particle_pipeline_layout),
            vertex: VertexState {
                module: &particle_shader,
                entry_point: "vs_main",
                buffers: &[VertexData::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &particle_shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(particle_blend_state),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: Self::create_primitive_state(None),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: Self::create_multisample_state(),
            multiview: None,
        });

        self.particle_pipeline = Some(particle_pipeline);

        let flame_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Flame Shader"),
            source: ShaderSource::Wgsl(FLAME_SHADER.into()),
        });

        let flame_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Flame Pipeline Layout"),
            bind_group_layouts: &[&self.particle_bind_group_layout],
            push_constant_ranges: &[],
        });

        let flame_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Flame Pipeline"),
            layout: Some(&flame_pipeline_layout),
            vertex: VertexState {
                module: &flame_shader,
                entry_point: "vs_main",
                buffers: &[VertexData::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &flame_shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: Self::create_primitive_state(None),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: Self::create_multisample_state(),
            multiview: None,
        });

        self.flame_pipeline = Some(flame_pipeline);
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

    fn create_uniforms(
        &self,
        view_proj: Mat4,
        model: Mat4,
        camera_pos: Vec3,
        lights: &[(Vec3, Vec3, f32)],
        ambient_light: f32,
    ) -> MD3Uniforms {
        let mut light_data = [LightData {
            position: [0.0; 4],
            color: [0.0; 4],
            radius: 0.0,
            _padding: [0.0; 3],
        }; MAX_LIGHTS];

        for (i, (pos, color, radius)) in lights.iter().enumerate().take(MAX_LIGHTS) {
            light_data[i] = LightData {
                position: [pos.x, pos.y, pos.z, 0.0],
                color: [color.x, color.y, color.z, 0.0],
                radius: *radius,
                _padding: [0.0; 3],
            };
        }

        MD3Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
            model: model.to_cols_array_2d(),
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 0.0],
            lights: light_data,
            num_lights: lights.len().min(MAX_LIGHTS) as i32,
            ambient_light,
            _padding: [0.0; 2],
        }
    }

    fn create_uniform_buffer(&self, uniforms: &MD3Uniforms, label: &str) -> Buffer {
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&[*uniforms]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        })
    }

    fn find_texture(&self, path: &str) -> Option<&WgpuTexture> {
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
    }

    fn create_mesh_bind_groups(
        &self,
        texture: &WgpuTexture,
        uniform_buffer: &Buffer,
        shadow_uniform_buffer: Option<&Buffer>,
        render_shadow: bool,
    ) -> (BindGroup, Option<BindGroup>) {
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
                        resource: shadow_uniform_buffer.unwrap().as_entire_binding(),
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

        (bind_group, shadow_bind_group)
    }

    fn prepare_mesh_data(
        &self,
        model: &MD3Model,
        frame_idx: usize,
        texture_paths: &[Option<String>],
        uniform_buffer: &Buffer,
        shadow_uniform_buffer: Option<&Buffer>,
        render_shadow: bool,
    ) -> Vec<MeshRenderData> {
        let mut mesh_data = Vec::new();

        for (mesh_idx, _mesh) in model.meshes.iter().enumerate() {
            let (vertex_buffer, index_buffer, num_indices) = match self.create_buffers(model, mesh_idx, frame_idx) {
                Some(buffers) => buffers,
                None => continue,
            };
            
            let texture_path = texture_paths.get(mesh_idx).and_then(|p| p.as_ref().map(|s| s.as_str()));
            let texture = texture_path.and_then(|path| self.find_texture(path));

            if let Some(texture) = texture {
                let (bind_group, shadow_bind_group) = self.create_mesh_bind_groups(
                    texture,
                    uniform_buffer,
                    shadow_uniform_buffer,
                    render_shadow,
                );

                mesh_data.push(MeshRenderData {
                    vertex_buffer,
                    index_buffer,
                    num_indices,
                    bind_group,
                    shadow_bind_group,
                });
            }
        }

        mesh_data
    }

    pub fn render_ground(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        camera_pos: Vec3,
        lights: &[(Vec3, Vec3, f32)],
        ambient_light: f32,
    ) {
        let uniforms = self.create_uniforms(
            view_proj,
            Mat4::IDENTITY,
            camera_pos,
            lights,
            ambient_light,
        );

        let ground_uniform_buffer = self.create_uniform_buffer(&uniforms, "Ground Uniform Buffer");

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Ground Bind Group"),
            layout: &self.ground_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: ground_uniform_buffer.as_entire_binding(),
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

    pub fn render_wall(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        camera_pos: Vec3,
        lights: &[(Vec3, Vec3, f32)],
        ambient_light: f32,
    ) {
        let uniforms = self.create_uniforms(
            view_proj,
            Mat4::IDENTITY,
            camera_pos,
            lights,
            ambient_light,
        );

        let wall_uniform_buffer = self.create_uniform_buffer(&uniforms, "Wall Uniform Buffer");

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Wall Bind Group"),
            layout: &self.ground_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wall_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline = self.ground_pipeline.as_ref().unwrap();
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Wall Render Pass"),
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
        render_pass.set_vertex_buffer(0, self.wall_vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.set_index_buffer(self.wall_index_buffer.as_ref().unwrap().slice(..), IndexFormat::Uint16);
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
        lights: &[(Vec3, Vec3, f32)],
        ambient_light: f32,
        render_shadow: bool,
    ) {
        if self.pipeline.is_none() {
            self.create_pipeline(surface_format);
        }

        let uniforms = self.create_uniforms(
            view_proj,
            model_matrix,
            camera_pos,
            lights,
            ambient_light,
        );

        let uniform_buffer = self.create_uniform_buffer(&uniforms, "MD3 Uniform Buffer");

        let shadow_uniform_buffer = if render_shadow {
            Some(self.create_uniform_buffer(&uniforms, "Shadow Uniform Buffer"))
        } else {
            None
        };

        let mesh_data = self.prepare_mesh_data(
            model,
            frame_idx,
            texture_paths,
            &uniform_buffer,
            shadow_uniform_buffer.as_ref(),
            render_shadow,
        );

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
                    stencil_ops: Some(Operations {
                        load: LoadOp::Clear(0),
                        store: StoreOp::Store,
                    }),
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            shadow_pass.set_pipeline(shadow_pipeline);
            shadow_pass.set_stencil_reference(0);

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

    pub fn render_wall_shadows_batch(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        camera_pos: Vec3,
        lights: &[(Vec3, Vec3, f32)],
        ambient_light: f32,
        models: &[(
            &MD3Model,
            usize,
            &[Option<String>],
            Mat4,
        )],
    ) {
        if self.wall_shadow_pipeline.is_none() || models.is_empty() {
            return;
        }

        let mut all_mesh_data = Vec::new();

        for (model, frame_idx, texture_paths, model_matrix) in models {
            let uniforms = self.create_uniforms(
                view_proj,
                *model_matrix,
                camera_pos,
                lights,
                ambient_light,
            );

            let uniform_buffer = self.create_uniform_buffer(&uniforms, "Wall Shadow Uniform Buffer");

            let mesh_data = self.prepare_mesh_data(
                model,
                *frame_idx,
                texture_paths,
                &uniform_buffer,
                None,
                false,
            );

            all_mesh_data.extend(mesh_data);
        }

        let wall_shadow_pipeline = self.wall_shadow_pipeline.as_ref().unwrap();

        let mut shadow_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Wall Shadow Render Pass"),
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
                stencil_ops: Some(Operations {
                    load: LoadOp::Clear(0),
                    store: StoreOp::Store,
                }),
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        shadow_pass.set_pipeline(wall_shadow_pipeline);
        shadow_pass.set_stencil_reference(0);

        for mesh in &all_mesh_data {
            shadow_pass.set_bind_group(0, &mesh.bind_group, &[]);
            shadow_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            shadow_pass.set_index_buffer(mesh.index_buffer.slice(..), IndexFormat::Uint16);
            shadow_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
    }

    pub fn render_particles(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        camera_pos: Vec3,
        particles: &[(Vec3, f32, f32)],
    ) {
        if self.particle_pipeline.is_none() || particles.is_empty() {
            return;
        }

        for (position, size, alpha) in particles {
            let vertices = vec![
                VertexData {
                    position: [position.x, position.y, position.z],
                    uv: [0.0, 0.0],
                    color: [1.0, 1.0, 1.0, *alpha],
                    normal: [0.0, 1.0, 0.0],
                },
                VertexData {
                    position: [position.x, position.y, position.z],
                    uv: [1.0, 0.0],
                    color: [1.0, 1.0, 1.0, *alpha],
                    normal: [0.0, 1.0, 0.0],
                },
                VertexData {
                    position: [position.x, position.y, position.z],
                    uv: [1.0, 1.0],
                    color: [1.0, 1.0, 1.0, *alpha],
                    normal: [0.0, 1.0, 0.0],
                },
                VertexData {
                    position: [position.x, position.y, position.z],
                    uv: [0.0, 1.0],
                    color: [1.0, 1.0, 1.0, *alpha],
                    normal: [0.0, 1.0, 0.0],
                },
            ];
            let indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];

            let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Particle Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: BufferUsages::VERTEX,
            });

            let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Particle Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: BufferUsages::INDEX,
            });

            #[repr(C)]
            #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
            struct ParticleUniforms {
                view_proj: [[f32; 4]; 4],
                model: [[f32; 4]; 4],
                camera_pos: [f32; 4],
            }

            let model = Mat4::from_scale(Vec3::splat(*size));
            let uniforms = ParticleUniforms {
                view_proj: view_proj.to_cols_array_2d(),
                model: model.to_cols_array_2d(),
                camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 0.0],
            };

            let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Particle Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: BufferUsages::UNIFORM,
            });

            let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Particle Bind Group"),
                layout: &self.particle_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            let pipeline = self.particle_pipeline.as_ref().unwrap();
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Particle Render Pass"),
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
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..1);
        }
    }

    pub fn render_flames(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        camera_pos: Vec3,
        flames: &[(Vec3, f32)],
        time: f32,
    ) {
        if self.flame_pipeline.is_none() || flames.is_empty() {
            return;
        }

        for (position, size) in flames {
            let vertices = vec![
                VertexData {
                    position: [position.x, position.y, position.z],
                    uv: [0.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                    normal: [0.0, 1.0, 0.0],
                },
                VertexData {
                    position: [position.x, position.y, position.z],
                    uv: [1.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                    normal: [0.0, 1.0, 0.0],
                },
                VertexData {
                    position: [position.x, position.y, position.z],
                    uv: [1.0, 1.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                    normal: [0.0, 1.0, 0.0],
                },
                VertexData {
                    position: [position.x, position.y, position.z],
                    uv: [0.0, 1.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                    normal: [0.0, 1.0, 0.0],
                },
            ];
            let indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];

            let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Flame Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: BufferUsages::VERTEX,
            });

            let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Flame Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: BufferUsages::INDEX,
            });

            #[repr(C)]
            #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
            struct FlameUniforms {
                view_proj: [[f32; 4]; 4],
                model: [[f32; 4]; 4],
                camera_pos: [f32; 4],
                time: f32,
                _padding0: f32,
                _padding1: f32,
                _padding2: f32,
            }

            let model = Mat4::from_scale(Vec3::splat(*size));
            let uniforms = FlameUniforms {
                view_proj: view_proj.to_cols_array_2d(),
                model: model.to_cols_array_2d(),
                camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 0.0],
                time,
                _padding0: 0.0,
                _padding1: 0.0,
                _padding2: 0.0,
            };

            let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Flame Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: BufferUsages::UNIFORM,
            });

            let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Flame Bind Group"),
                layout: &self.particle_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            let pipeline = self.flame_pipeline.as_ref().unwrap();
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Flame Render Pass"),
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
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..1);
        }
    }
}
