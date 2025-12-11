use std::collections::HashMap;
use std::sync::Arc;
use wgpu::*;
use wgpu::util::DeviceExt;
use winit::window::Window;
use glam::{Mat4, Vec3};
use bytemuck::{Pod, Zeroable};
use crate::engine::md3::MD3Model;
use crate::engine::shaders::{MD3_SHADER, GROUND_SHADER, SHADOW_SHADER, WALL_SHADOW_SHADER, PARTICLE_SHADER, FLAME_SHADER, WALL_SHADER, DEBUG_LIGHT_SPHERE_SHADER, DEBUG_LIGHT_RAY_SHADER, SHADOW_VOLUME_SHADER, SHADOW_APPLY_SHADER, SHADOW_PLANAR_SHADER};

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
    logical_size: winit::dpi::PhysicalSize<u32>,
    pixel_ratio: f64,
}

impl WgpuRenderer {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let pixel_ratio = 2.0;
        let logical_size = window.inner_size();
        let size = winit::dpi::PhysicalSize::new(
            (logical_size.width as f64 * pixel_ratio) as u32,
            (logical_size.height as f64 * pixel_ratio) as u32,
        );
        
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
            logical_size,
            pixel_ratio,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.logical_size = new_size;
            let size = winit::dpi::PhysicalSize::new(
                (new_size.width as f64 * self.pixel_ratio) as u32,
                (new_size.height as f64 * self.pixel_ratio) as u32,
            );
            self.size = size;
            self.surface_config.width = size.width;
            self.surface_config.height = size.height;
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
        (self.logical_size.width, self.logical_size.height)
    }

    pub fn get_surface_size(&self) -> (u32, u32) {
        (self.size.width, self.size.height)
    }
}

struct MeshRenderData {
    vertex_buffer: Arc<Buffer>,
    index_buffer: Arc<Buffer>,
    num_indices: u32,
    bind_group: BindGroup,
    shadow_bind_group: Option<BindGroup>,
    uniform_buffer: Arc<Buffer>,
    shadow_uniform_buffer: Option<Arc<Buffer>>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ShadowVolumeVertex {
    pub position: [f32; 3],
    pub extrude_dir: [f32; 3],
}

impl ShadowVolumeVertex {
    pub fn desc() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<ShadowVolumeVertex>() as BufferAddress,
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
                    format: VertexFormat::Float32x3,
                },
            ],
        }
    }
}

struct Edge {
    v0: usize,
    v1: usize,
}

struct SilhouetteEdge {
    v0: Vec3,
    v1: Vec3,
}

struct ModelSilhouetteCache {
    edges: Vec<Edge>,
    triangle_neighbors: Vec<[Option<usize>; 3]>,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct BufferCacheKey {
    model_id: usize,
    mesh_idx: usize,
    frame_idx: usize,
}

struct CachedBuffers {
    vertex_buffer: Arc<Buffer>,
    index_buffer: Arc<Buffer>,
    num_indices: u32,
}

pub struct MD3Renderer {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub pipeline: Option<RenderPipeline>,
    pub ground_pipeline: Option<RenderPipeline>,
    pub wall_pipeline: Option<RenderPipeline>,
    pub shadow_pipeline: Option<RenderPipeline>,
    pub wall_shadow_pipeline: Option<RenderPipeline>,
    pub particle_pipeline: Option<RenderPipeline>,
    pub flame_pipeline: Option<RenderPipeline>,
    pub uniform_buffer: Option<Buffer>,
    pub bind_group_layout: BindGroupLayout,
    pub ground_bind_group_layout: BindGroupLayout,
    pub wall_bind_group_layout: BindGroupLayout,
    pub particle_bind_group_layout: BindGroupLayout,
    pub model_textures: HashMap<String, WgpuTexture>,
    pub ground_vertex_buffer: Option<Buffer>,
    pub ground_index_buffer: Option<Buffer>,
    pub ground_texture: Option<WgpuTexture>,
    pub wall_vertex_buffer: Option<Buffer>,
    pub wall_index_buffer: Option<Buffer>,
    pub wall_texture: Option<WgpuTexture>,
    pub wall_curb_texture: Option<WgpuTexture>,
    buffer_cache: HashMap<BufferCacheKey, CachedBuffers>,
    uniform_buffer_pool: Option<Buffer>,
    shadow_uniform_buffer_pool: Option<Buffer>,
    ground_uniform_buffer: Option<Buffer>,
    wall_uniform_buffer: Option<Buffer>,
    particle_quad_vertex_buffer: Option<Buffer>,
    particle_quad_index_buffer: Option<Buffer>,
    particle_instance_buffer: Option<Buffer>,
    flame_instance_buffer: Option<Buffer>,
    ground_bind_group: Option<BindGroup>,
    wall_bind_group: Option<BindGroup>,
    particle_uniform_buffer: Option<Buffer>,
    particle_bind_group: Option<BindGroup>,
    flame_uniform_buffer: Option<Buffer>,
    flame_bind_group: Option<BindGroup>,
    smoke_texture: Option<WgpuTexture>,
    flame_texture: Option<WgpuTexture>,
    debug_light_sphere_pipeline: Option<RenderPipeline>,
    debug_light_ray_pipeline: Option<RenderPipeline>,
    debug_light_sphere_uniform_buffer: Option<Buffer>,
    debug_light_sphere_bind_group: Option<BindGroup>,
    debug_light_sphere_bind_group_layout: BindGroupLayout,
    debug_light_ray_uniform_buffer: Option<Buffer>,
    debug_light_ray_bind_group: Option<BindGroup>,
    debug_light_ray_bind_group_layout: BindGroupLayout,
    debug_sphere_vertex_buffer: Option<Buffer>,
    debug_sphere_index_buffer: Option<Buffer>,
    debug_sphere_instance_buffer: Option<Buffer>,
    debug_ray_vertex_buffer: Option<Buffer>,
    shadow_volume_front_pipeline: Option<RenderPipeline>,
    shadow_volume_back_pipeline: Option<RenderPipeline>,
    shadow_volume_bind_group_layout: BindGroupLayout,
    shadow_apply_pipeline: Option<RenderPipeline>,
    shadow_apply_vertex_buffer: Option<Buffer>,
    shadow_planar_pipeline: Option<RenderPipeline>,
    silhouette_cache: HashMap<(usize, usize), ModelSilhouetteCache>,
}

impl MD3Renderer {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        let bind_group_layout = Self::create_md3_bind_group_layout(&device);
        let ground_bind_group_layout = Self::create_ground_bind_group_layout(&device);
        let wall_bind_group_layout = Self::create_wall_bind_group_layout(&device);
        let particle_bind_group_layout = Self::create_particle_bind_group_layout(&device);
        let debug_light_sphere_bind_group_layout = Self::create_debug_light_sphere_bind_group_layout(&device);
        let debug_light_ray_bind_group_layout = Self::create_debug_light_ray_bind_group_layout(&device);
        let shadow_volume_bind_group_layout = Self::create_shadow_volume_bind_group_layout(&device);

        let uniform_buffer_pool = Some(device.create_buffer(&BufferDescriptor {
            label: Some("Uniform Buffer Pool"),
            size: std::mem::size_of::<MD3Uniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));

        Self {
            device,
            queue,
            pipeline: None,
            ground_pipeline: None,
            wall_pipeline: None,
            shadow_pipeline: None,
            wall_shadow_pipeline: None,
            particle_pipeline: None,
            flame_pipeline: None,
            uniform_buffer: None,
            bind_group_layout,
            ground_bind_group_layout,
            wall_bind_group_layout,
            particle_bind_group_layout,
            model_textures: HashMap::new(),
            ground_vertex_buffer: None,
            ground_index_buffer: None,
            ground_texture: None,
            wall_vertex_buffer: None,
            wall_index_buffer: None,
            wall_texture: None,
            wall_curb_texture: None,
            buffer_cache: HashMap::new(),
            uniform_buffer_pool,
            shadow_uniform_buffer_pool: None,
            ground_uniform_buffer: None,
            wall_uniform_buffer: None,
            particle_quad_vertex_buffer: None,
            particle_quad_index_buffer: None,
            particle_instance_buffer: None,
            flame_instance_buffer: None,
            ground_bind_group: None,
            wall_bind_group: None,
            particle_uniform_buffer: None,
            particle_bind_group: None,
            flame_uniform_buffer: None,
            flame_bind_group: None,
            smoke_texture: None,
            flame_texture: None,
            debug_light_sphere_pipeline: None,
            debug_light_ray_pipeline: None,
            debug_light_sphere_uniform_buffer: None,
            debug_light_sphere_bind_group: None,
            debug_light_sphere_bind_group_layout,
            debug_light_ray_uniform_buffer: None,
            debug_light_ray_bind_group: None,
            debug_light_ray_bind_group_layout,
            debug_sphere_vertex_buffer: None,
            debug_sphere_index_buffer: None,
            debug_sphere_instance_buffer: None,
            debug_ray_vertex_buffer: None,
            shadow_volume_front_pipeline: None,
            shadow_volume_back_pipeline: None,
            shadow_volume_bind_group_layout,
            shadow_apply_pipeline: None,
            shadow_apply_vertex_buffer: None,
            shadow_planar_pipeline: None,
            silhouette_cache: HashMap::new(),
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

    fn create_wall_bind_group_layout(device: &Device) -> BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Wall Bind Group Layout"),
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
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    fn create_particle_bind_group_layout(device: &Device) -> BindGroupLayout {
        #[repr(C)]
        struct ParticleUniforms {
            view_proj: [[f32; 4]; 4],
            camera_pos: [f32; 4],
        }
        #[repr(C)]
        struct FlameUniforms {
            view_proj: [[f32; 4]; 4],
            camera_pos: [f32; 4],
            time: f32,
            _padding0: f32,
            _padding1: f32,
            _padding2: f32,
        }
        let particle_size = std::mem::size_of::<ParticleUniforms>() as u64;
        let flame_size = std::mem::size_of::<FlameUniforms>() as u64;
        let max_size = particle_size.max(flame_size);
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Particle Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(max_size),
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

    fn create_debug_light_sphere_bind_group_layout(device: &Device) -> BindGroupLayout {
        #[repr(C)]
        struct DebugLightSphereUniforms {
            view_proj: [[f32; 4]; 4],
            camera_pos: [f32; 4],
        }
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Debug Light Sphere Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<DebugLightSphereUniforms>() as u64),
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_debug_light_ray_bind_group_layout(device: &Device) -> BindGroupLayout {
        #[repr(C)]
        struct DebugLightRayUniforms {
            view_proj: [[f32; 4]; 4],
        }
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Debug Light Ray Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<DebugLightRayUniforms>() as u64),
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_shadow_volume_bind_group_layout(device: &Device) -> BindGroupLayout {
        #[repr(C)]
        struct ShadowVolumeUniforms {
            view_proj: [[f32; 4]; 4],
            light_pos: [f32; 4],
            extrude_distance: f32,
            _padding: [f32; 3],
        }
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Shadow Volume Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<ShadowVolumeUniforms>() as u64),
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

        let wall_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Wall Shader"),
            source: ShaderSource::Wgsl(WALL_SHADER.into()),
        });

        let wall_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Wall Pipeline Layout"),
            bind_group_layouts: &[&self.wall_bind_group_layout],
            push_constant_ranges: &[],
        });

        let wall_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Wall Pipeline"),
            layout: Some(&wall_pipeline_layout),
            vertex: VertexState {
                module: &wall_shader,
                entry_point: "vs_main",
                buffers: &[VertexData::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &wall_shader,
                entry_point: "fs_main",
                targets: &[Some(Self::create_color_target_state(surface_format))],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: Self::create_primitive_state(None),
            depth_stencil: Some(Self::create_depth_stencil_state(true)),
            multisample: Self::create_multisample_state(),
            multiview: None,
        });

        self.wall_pipeline = Some(wall_pipeline);

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
        let ground_y = 0.0;
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
        
        self.create_ground_texture();

        let wall_size = 500.0;
        let wall_height = 50.0;
        let wall_z = -3.0;
        let wall_bottom = 0.0;
        let wall_vertices = vec![
            VertexData {
                position: [-wall_size, wall_bottom, wall_z],
                uv: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
            },
            VertexData {
                position: [wall_size, wall_bottom, wall_z],
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

        let instance_buffer_layout = VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 4]>() as BufferAddress * 2,
            step_mode: VertexStepMode::Instance,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 4,
                    format: VertexFormat::Float32x4,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as BufferAddress,
                    shader_location: 5,
                    format: VertexFormat::Float32,
                },
            ],
        };

        let particle_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Particle Pipeline"),
            layout: Some(&particle_pipeline_layout),
            vertex: VertexState {
                module: &particle_shader,
                entry_point: "vs_main",
                buffers: &[VertexData::desc(), instance_buffer_layout],
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

        let flame_instance_buffer_layout = VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 4]>() as BufferAddress,
            step_mode: VertexStepMode::Instance,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 4,
                    format: VertexFormat::Float32x4,
                },
            ],
        };

        let flame_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Flame Pipeline"),
            layout: Some(&flame_pipeline_layout),
            vertex: VertexState {
                module: &flame_shader,
                entry_point: "vs_main",
                buffers: &[VertexData::desc(), flame_instance_buffer_layout],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &flame_shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                    }),
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

        let quad_vertices = vec![
            VertexData {
                position: [-0.5, -0.5, 0.0],
                uv: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
            },
            VertexData {
                position: [0.5, -0.5, 0.0],
                uv: [1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
            },
            VertexData {
                position: [0.5, 0.5, 0.0],
                uv: [1.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
            },
            VertexData {
                position: [-0.5, 0.5, 0.0],
                uv: [0.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
            },
        ];
        let quad_indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];

        let particle_quad_vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(&quad_vertices),
            usage: BufferUsages::VERTEX,
        });

        let particle_quad_index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Quad Index Buffer"),
            contents: bytemuck::cast_slice(&quad_indices),
            usage: BufferUsages::INDEX,
        });

        self.particle_quad_vertex_buffer = Some(particle_quad_vertex_buffer);
        self.particle_quad_index_buffer = Some(particle_quad_index_buffer);

        let max_particles = 1000;
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct ParticleInstance {
            position_size: [f32; 4],
            alpha: f32,
            _padding: [f32; 3],
        }

        let particle_instance_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Particle Instance Buffer"),
            size: (std::mem::size_of::<ParticleInstance>() * max_particles) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.particle_instance_buffer = Some(particle_instance_buffer);

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct FlameInstance {
            position_size: [f32; 4],
            direction: [f32; 4],
        }

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct FlameInstanceData {
            position_size: [f32; 4],
        }

        let flame_instance_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Flame Instance Buffer"),
            size: (std::mem::size_of::<FlameInstanceData>() * max_particles) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.flame_instance_buffer = Some(flame_instance_buffer);

        #[repr(C)]
        struct ParticleUniforms {
            view_proj: [[f32; 4]; 4],
            camera_pos: [f32; 4],
        }

        #[repr(C)]
        struct FlameUniforms {
            view_proj: [[f32; 4]; 4],
            camera_pos: [f32; 4],
            time: f32,
            _padding0: f32,
            _padding1: f32,
            _padding2: f32,
        }
        let particle_size = std::mem::size_of::<ParticleUniforms>() as u64;
        let flame_size = std::mem::size_of::<FlameUniforms>() as u64;
        let max_size = particle_size.max(flame_size);
        let particle_uniform_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Particle Uniform Buffer"),
            size: max_size,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.particle_uniform_buffer = Some(particle_uniform_buffer);

        let flame_uniform_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Flame Uniform Buffer"),
            size: max_size,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.flame_uniform_buffer = Some(flame_uniform_buffer);

        if self.smoke_texture.is_none() {
            self.create_smoke_texture();
        }

        let smoke_tex = self.smoke_texture.as_ref().unwrap();
        let particle_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Particle Bind Group"),
            layout: &self.particle_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.particle_uniform_buffer.as_ref().unwrap().as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&smoke_tex.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&smoke_tex.sampler),
                },
            ],
        });
        self.particle_bind_group = Some(particle_bind_group);

        if self.flame_texture.is_none() {
            self.create_flame_texture();
        }

        let flame_tex = self.flame_texture.as_ref().unwrap();
        let flame_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Flame Bind Group"),
            layout: &self.particle_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.flame_uniform_buffer.as_ref().unwrap().as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&flame_tex.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&flame_tex.sampler),
                },
            ],
        });
        self.flame_bind_group = Some(flame_bind_group);

        let shadow_volume_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shadow Volume Shader"),
            source: ShaderSource::Wgsl(SHADOW_VOLUME_SHADER.into()),
        });

        let shadow_volume_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shadow Volume Pipeline Layout"),
            bind_group_layouts: &[&self.shadow_volume_bind_group_layout],
            push_constant_ranges: &[],
        });

        let mut shadow_volume_depth_stencil_front = DepthStencilState {
            format: TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: false,
            depth_compare: CompareFunction::LessEqual,
            stencil: StencilState {
                front: StencilFaceState {
                    compare: CompareFunction::Always,
                    fail_op: StencilOperation::Keep,
                    depth_fail_op: StencilOperation::DecrementWrap,
                    pass_op: StencilOperation::Keep,
                },
                back: StencilFaceState {
                    compare: CompareFunction::Always,
                    fail_op: StencilOperation::Keep,
                    depth_fail_op: StencilOperation::DecrementWrap,
                    pass_op: StencilOperation::Keep,
                },
                read_mask: 0xff,
                write_mask: 0xff,
            },
            bias: DepthBiasState::default(),
        };

        let shadow_volume_color_target = ColorTargetState {
            format: surface_format,
            blend: None,
            write_mask: ColorWrites::empty(),
        };

        let shadow_volume_front_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Shadow Volume Front Pipeline"),
            layout: Some(&shadow_volume_pipeline_layout),
            vertex: VertexState {
                module: &shadow_volume_shader,
                entry_point: "vs_main",
                buffers: &[ShadowVolumeVertex::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shadow_volume_shader,
                entry_point: "fs_main",
                targets: &[Some(shadow_volume_color_target.clone())],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Front),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(shadow_volume_depth_stencil_front.clone()),
            multisample: Self::create_multisample_state(),
            multiview: None,
        });

        let mut shadow_volume_depth_stencil_back = shadow_volume_depth_stencil_front.clone();
        shadow_volume_depth_stencil_back.stencil.front.depth_fail_op = StencilOperation::IncrementWrap;
        shadow_volume_depth_stencil_back.stencil.back.depth_fail_op = StencilOperation::IncrementWrap;
        shadow_volume_depth_stencil_back.stencil.front.pass_op = StencilOperation::Keep;
        shadow_volume_depth_stencil_back.stencil.back.pass_op = StencilOperation::Keep;

        let shadow_volume_back_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Shadow Volume Back Pipeline"),
            layout: Some(&shadow_volume_pipeline_layout),
            vertex: VertexState {
                module: &shadow_volume_shader,
                entry_point: "vs_main",
                buffers: &[ShadowVolumeVertex::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shadow_volume_shader,
                entry_point: "fs_main",
                targets: &[Some(shadow_volume_color_target)],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(shadow_volume_depth_stencil_back),
            multisample: Self::create_multisample_state(),
            multiview: None,
        });

        self.shadow_volume_front_pipeline = Some(shadow_volume_front_pipeline);
        self.shadow_volume_back_pipeline = Some(shadow_volume_back_pipeline);

        let shadow_apply_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shadow Apply Shader"),
            source: ShaderSource::Wgsl(SHADOW_APPLY_SHADER.into()),
        });

        let shadow_apply_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shadow Apply Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let shadow_apply_blend = BlendState {
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

        let shadow_apply_color_target = ColorTargetState {
            format: surface_format,
            blend: Some(shadow_apply_blend),
            write_mask: ColorWrites::ALL,
        };

        let shadow_apply_depth_stencil = DepthStencilState {
            format: TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: false,
            depth_compare: CompareFunction::LessEqual, // ограничиваем тень поверх ближайшей поверхности (стена/пол)
            stencil: StencilState {
                front: StencilFaceState {
                    compare: CompareFunction::NotEqual,
                    fail_op: StencilOperation::Keep,
                    depth_fail_op: StencilOperation::Keep,
                    pass_op: StencilOperation::Keep,
                },
                back: StencilFaceState {
                    compare: CompareFunction::NotEqual,
                    fail_op: StencilOperation::Keep,
                    depth_fail_op: StencilOperation::Keep,
                    pass_op: StencilOperation::Keep,
                },
                read_mask: 0xff,
                write_mask: 0x00,
            },
            bias: DepthBiasState::default(),
        };

        let shadow_apply_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Shadow Apply Pipeline"),
            layout: Some(&shadow_apply_pipeline_layout),
            vertex: VertexState {
                module: &shadow_apply_shader,
                entry_point: "vs_main",
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 2]>() as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: VertexFormat::Float32x2,
                    }],
                }],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shadow_apply_shader,
                entry_point: "fs_main",
                targets: &[Some(shadow_apply_color_target)],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(shadow_apply_depth_stencil),
            multisample: Self::create_multisample_state(),
            multiview: None,
        });

        self.shadow_apply_pipeline = Some(shadow_apply_pipeline);

        let fullscreen_quad: Vec<[f32; 2]> = vec![
            [-1.0, -1.0],
            [1.0, -1.0],
            [1.0, 1.0],
            [-1.0, -1.0],
            [1.0, 1.0],
            [-1.0, 1.0],
        ];

        let shadow_apply_vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Apply Vertex Buffer"),
            contents: bytemuck::cast_slice(&fullscreen_quad),
            usage: BufferUsages::VERTEX,
        });

        self.shadow_apply_vertex_buffer = Some(shadow_apply_vertex_buffer);

        // Planar shadow pipeline (projected onto ground/wall)
        let shadow_planar_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shadow Planar Shader"),
            source: ShaderSource::Wgsl(SHADOW_PLANAR_SHADER.into()),
        });

        let shadow_planar_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shadow Planar Pipeline Layout"),
            bind_group_layouts: &[&self.shadow_volume_bind_group_layout],
            push_constant_ranges: &[],
        });

        let shadow_planar_blend = BlendState {
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

        let shadow_planar_color_target = ColorTargetState {
            format: surface_format,
            blend: Some(shadow_planar_blend),
            write_mask: ColorWrites::ALL,
        };

        let shadow_planar_depth_stencil = DepthStencilState {
            format: TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: false,
            depth_compare: CompareFunction::LessEqual,
            stencil: StencilState::default(),
            bias: DepthBiasState::default(),
        };

        let shadow_planar_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Shadow Planar Pipeline"),
            layout: Some(&shadow_planar_pipeline_layout),
            vertex: VertexState {
                module: &shadow_planar_shader,
                entry_point: "vs_main",
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 3]>() as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: VertexFormat::Float32x3,
                    }],
                }],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shadow_planar_shader,
                entry_point: "fs_main",
                targets: &[Some(shadow_planar_color_target)],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(shadow_planar_depth_stencil),
            multisample: Self::create_multisample_state(),
            multiview: None,
        });

        self.shadow_planar_pipeline = Some(shadow_planar_pipeline);
    }

    pub fn create_flame_texture(&mut self) {
        let candidates = vec![
            "q3-resources/models/ammo/rocket/rockflar.png",
            "q3-resources/models/ammo/rocket/rockflar.tga",
            "q3-resources/models/ammo/rocket/rockfls1.png",
            "q3-resources/models/ammo/rocket/rockfls1.tga",
            "q3-resources/models/ammo/rocket/rockfls2.png",
            "../q3-resources/models/ammo/rocket/rockflar.png",
            "../q3-resources/models/ammo/rocket/rockflar.tga",
            "../q3-resources/models/ammo/rocket/rockfls1.png",
            "../q3-resources/models/ammo/rocket/rockfls1.tga",
            "../q3-resources/models/ammo/rocket/rockfls2.png",
        ];

        let mut texture_loaded = false;
        for path in candidates {
            if std::path::Path::new(path).exists() {
                if let Ok(data) = std::fs::read(path) {
                    if let Ok(img) = image::load_from_memory(&data) {
                        let img = img.to_rgba8();
                        let size = Extent3d {
                            width: img.width(),
                            height: img.height(),
                            depth_or_array_layers: 1,
                        };
                        let texture = self.device.create_texture(&TextureDescriptor {
                            label: Some("Flame Texture"),
                            size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::Rgba8UnormSrgb,
                            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                            view_formats: &[],
                        });

                        self.queue.write_texture(
                            ImageCopyTexture {
                                texture: &texture,
                                mip_level: 0,
                                origin: Origin3d::ZERO,
                                aspect: TextureAspect::All,
                            },
                            &img,
                            ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * img.width()),
                                rows_per_image: Some(img.height()),
                            },
                            size,
                        );

                        let view = texture.create_view(&TextureViewDescriptor::default());
                        let sampler = self.device.create_sampler(&SamplerDescriptor {
                            address_mode_u: AddressMode::ClampToEdge,
                            address_mode_v: AddressMode::ClampToEdge,
                            address_mode_w: AddressMode::ClampToEdge,
                            mag_filter: FilterMode::Linear,
                            min_filter: FilterMode::Linear,
                            mipmap_filter: FilterMode::Linear,
                            ..Default::default()
                        });

                        self.flame_texture = Some(WgpuTexture {
                            texture,
                            view,
                            sampler,
                        });
                        texture_loaded = true;
                        break;
                    }
                }
            }
        }
    }

    fn get_or_create_buffers(&mut self, model: &MD3Model, mesh_idx: usize, frame_idx: usize) -> Option<(Arc<Buffer>, Arc<Buffer>, u32)> {
        let model_id = std::ptr::addr_of!(*model) as usize;
        let key = BufferCacheKey {
            model_id,
            mesh_idx,
            frame_idx,
        };
        
        if let Some(cached) = self.buffer_cache.get(&key) {
            return Some((cached.vertex_buffer.clone(), cached.index_buffer.clone(), cached.num_indices));
        }
        
        let (vertex_buffer, index_buffer, num_indices) = self.create_buffers_internal(model, mesh_idx, frame_idx)?;
        let cached = CachedBuffers {
            vertex_buffer: Arc::new(vertex_buffer),
            index_buffer: Arc::new(index_buffer),
            num_indices,
        };
        let result = (cached.vertex_buffer.clone(), cached.index_buffer.clone(), cached.num_indices);
        self.buffer_cache.insert(key, cached);
        Some(result)
    }
    
    fn create_buffers_internal(&self, model: &MD3Model, mesh_idx: usize, frame_idx: usize) -> Option<(Buffer, Buffer, u32)> {
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

    fn update_uniform_buffer(&self, uniforms: &MD3Uniforms, buffer: &Buffer) {
        self.queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[*uniforms]));
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
        &mut self,
        model: &MD3Model,
        frame_idx: usize,
        texture_paths: &[Option<String>],
        uniform_buffer: Arc<Buffer>,
        shadow_uniform_buffer: Option<Arc<Buffer>>,
        render_shadow: bool,
    ) -> Vec<MeshRenderData> {
        let mut buffers_vec = Vec::new();
        
        for (mesh_idx, _mesh) in model.meshes.iter().enumerate() {
            let (vertex_buffer, index_buffer, num_indices) = match self.get_or_create_buffers(model, mesh_idx, frame_idx) {
                Some(buffers) => buffers,
                None => continue,
            };
            
            let texture_path = texture_paths.get(mesh_idx).and_then(|p| p.as_ref().map(|s| s.clone()));

            if texture_path.is_some() {
                buffers_vec.push((vertex_buffer, index_buffer, num_indices, texture_path));
            }
        }
        
        let mut mesh_data = Vec::new();
        for (vertex_buffer, index_buffer, num_indices, texture_path) in buffers_vec {
            let texture = texture_path.as_ref().and_then(|path| self.find_texture(path));
            if let Some(texture) = texture {
                let (bind_group, shadow_bind_group) = self.create_mesh_bind_groups(
                    texture,
                    &uniform_buffer,
                    shadow_uniform_buffer.as_ref().map(|b| b.as_ref()),
                    render_shadow,
                );

                mesh_data.push(MeshRenderData {
                    vertex_buffer,
                    index_buffer,
                    num_indices,
                    bind_group,
                    shadow_bind_group,
                    uniform_buffer: uniform_buffer.clone(),
                    shadow_uniform_buffer: shadow_uniform_buffer.clone(),
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
        if self.ground_texture.is_none() {
            self.create_ground_texture();
        }

        if self.ground_uniform_buffer.is_none() {
            self.ground_uniform_buffer = Some(self.device.create_buffer(&BufferDescriptor {
                label: Some("Ground Uniform Buffer"),
                size: std::mem::size_of::<MD3Uniforms>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        if self.ground_bind_group.is_none() {
            let ground_tex = self.ground_texture.as_ref().unwrap();
            let ground_uniform_buffer = self.ground_uniform_buffer.as_ref().unwrap();
            self.ground_bind_group = Some(self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Ground Bind Group"),
                layout: &self.ground_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: ground_uniform_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&ground_tex.view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Sampler(&ground_tex.sampler),
                    },
                ],
            }));
        }

        let uniforms = self.create_uniforms(
            view_proj,
            Mat4::IDENTITY,
            camera_pos,
            lights,
            ambient_light,
        );

        let ground_uniform_buffer = self.ground_uniform_buffer.as_ref().unwrap();
        self.update_uniform_buffer(&uniforms, ground_uniform_buffer);

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
        render_pass.set_bind_group(0, self.ground_bind_group.as_ref().unwrap(), &[]);
        render_pass.set_vertex_buffer(0, self.ground_vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.set_index_buffer(self.ground_index_buffer.as_ref().unwrap().slice(..), IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..1);
    }

    pub fn create_ground_texture(&mut self) {
        let texture_paths = vec![
            "../q3-resources/textures/base_floor/clang_floor3b.png",
            "../q3-resources/textures/base_floor/clang_floor3b.jpg",
            "../q3-resources/textures/base_floor/clang_floor3b.tga",
            "../q3-resources/textures/base_floor/clang_floor3.png",
            "../q3-resources/textures/base_floor/clang_floor3.jpg",
            "../q3-resources/textures/base_floor/clang_floor3.tga",
            "../q3-resources/textures/base_floor/clang_floor2.png",
            "../q3-resources/textures/base_floor/clang_floor2.jpg",
            "../q3-resources/textures/base_floor/clang_floor2.tga",
            "../q3-resources/textures/base_floor/clang_floor1.png",
            "../q3-resources/textures/base_floor/clang_floor1.jpg",
            "../q3-resources/textures/base_floor/clang_floor1.tga",
            "../q3-resources/textures/base_floor/floor1.png",
            "../q3-resources/textures/base_floor/floor1.jpg",
            "../q3-resources/textures/base_floor/floor1.tga",
            "q3-resources/textures/base_floor/clang_floor3b.png",
            "q3-resources/textures/base_floor/clang_floor3b.jpg",
            "q3-resources/textures/base_floor/clang_floor3b.tga",
            "q3-resources/textures/base_floor/clang_floor3.png",
            "q3-resources/textures/base_floor/clang_floor3.jpg",
            "q3-resources/textures/base_floor/clang_floor3.tga",
            "q3-resources/textures/base_floor/clang_floor2.png",
            "q3-resources/textures/base_floor/clang_floor2.jpg",
            "q3-resources/textures/base_floor/clang_floor2.tga",
            "q3-resources/textures/base_floor/clang_floor1.png",
            "q3-resources/textures/base_floor/clang_floor1.jpg",
            "q3-resources/textures/base_floor/clang_floor1.tga",
            "q3-resources/textures/base_floor/floor1.png",
            "q3-resources/textures/base_floor/floor1.jpg",
            "q3-resources/textures/base_floor/floor1.tga",
        ];

        let mut texture_loaded = false;
        for texture_path in texture_paths {
            if std::path::Path::new(&texture_path).exists() {
                if let Ok(data) = std::fs::read(&texture_path) {
                    if let Ok(img) = image::load_from_memory(&data) {
                        let img = img.to_rgba8();
                        let size = Extent3d {
                            width: img.width(),
                            height: img.height(),
                            depth_or_array_layers: 1,
                        };
                        let texture = self.device.create_texture(&TextureDescriptor {
                            label: Some("Ground Texture"),
                            size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::Rgba8UnormSrgb,
                            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                            view_formats: &[],
                        });

                        self.queue.write_texture(
                            ImageCopyTexture {
                                texture: &texture,
                                mip_level: 0,
                                origin: Origin3d::ZERO,
                                aspect: TextureAspect::All,
                            },
                            &img,
                            ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * img.width()),
                                rows_per_image: Some(img.height()),
                            },
                            size,
                        );

                        let view = texture.create_view(&TextureViewDescriptor::default());
                        let sampler = self.device.create_sampler(&SamplerDescriptor {
                            address_mode_u: AddressMode::Repeat,
                            address_mode_v: AddressMode::Repeat,
                            address_mode_w: AddressMode::Repeat,
                            mag_filter: FilterMode::Linear,
                            min_filter: FilterMode::Linear,
                            mipmap_filter: FilterMode::Linear,
                            ..Default::default()
                        });

                        self.ground_texture = Some(WgpuTexture {
                            texture,
                            view,
                            sampler,
                        });
                        texture_loaded = true;
                        println!("Loaded ground texture from: {}", texture_path);
                        break;
                    }
                }
            }
        }

        if !texture_loaded {
            println!("Warning: Could not load ground texture, using fallback");
            let size = 128u32;
            let mut pixels = Vec::with_capacity((size * size * 4) as usize);
            
            for y in 0..size {
                for x in 0..size {
                    let fx = x as f32 / size as f32;
                    let fy = y as f32 / size as f32;
                    
                    let checker = ((fx * 8.0).floor() + (fy * 8.0).floor()) as i32;
                    let is_dark = checker % 2 == 0;
                    let r = if is_dark { 0.25 } else { 0.18 };
                    let g = if is_dark { 0.25 } else { 0.18 };
                    let b = if is_dark { 0.28 } else { 0.2 };
                    
                    pixels.push((r * 255.0) as u8);
                    pixels.push((g * 255.0) as u8);
                    pixels.push((b * 255.0) as u8);
                    pixels.push(255);
                }
            }
            
            let texture = self.device.create_texture(&TextureDescriptor {
                label: Some("Ground Texture Fallback"),
                size: Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });

            self.queue.write_texture(
                ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &pixels,
                ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * size),
                    rows_per_image: Some(size),
                },
                Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );

            let view = texture.create_view(&TextureViewDescriptor::default());
            let sampler = self.device.create_sampler(&SamplerDescriptor {
                address_mode_u: AddressMode::Repeat,
                address_mode_v: AddressMode::Repeat,
                address_mode_w: AddressMode::Repeat,
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                mipmap_filter: FilterMode::Linear,
                ..Default::default()
            });

            self.ground_texture = Some(WgpuTexture {
                texture,
                view,
                sampler,
            });
        }
    }

    pub fn create_wall_texture(&mut self) {
        let texture_paths = vec![
            "../q3-resources/textures/base_wall/atech2_c.png",
            "../q3-resources/textures/base_wall/atech2_c.jpg",
            "../q3-resources/textures/base_wall/atech2_c.tga",
            "../q3-resources/textures/base_wall/atech3_a.png",
            "../q3-resources/textures/base_wall/atech3_a.jpg",
            "../q3-resources/textures/base_wall/atech3_a.tga",
            "../q3-resources/textures/base_wall/basewall04.png",
            "../q3-resources/textures/base_wall/basewall04.jpg",
            "../q3-resources/textures/base_wall/basewall04.tga",
            "../q3-resources/textures/base_wall/concrete.png",
            "../q3-resources/textures/base_wall/concrete.jpg",
            "../q3-resources/textures/base_wall/concrete.tga",
            "../q3-resources/textures/base_wall/atech1_a.png",
            "../q3-resources/textures/base_wall/atech1_a.jpg",
            "q3-resources/textures/base_wall/atech2_c.png",
            "q3-resources/textures/base_wall/atech2_c.jpg",
            "q3-resources/textures/base_wall/atech2_c.tga",
            "q3-resources/textures/base_wall/atech3_a.png",
            "q3-resources/textures/base_wall/atech3_a.jpg",
            "q3-resources/textures/base_wall/atech3_a.tga",
            "q3-resources/textures/base_wall/basewall04.png",
            "q3-resources/textures/base_wall/basewall04.jpg",
            "q3-resources/textures/base_wall/basewall04.tga",
            "q3-resources/textures/base_wall/concrete.png",
            "q3-resources/textures/base_wall/concrete.jpg",
            "q3-resources/textures/base_wall/concrete.tga",
            "q3-resources/textures/base_wall/atech1_a.png",
            "q3-resources/textures/base_wall/atech1_a.jpg",
            "../q3-resources/textures/base_wall/atech1_a.tga",
            "q3-resources/textures/base_wall/atech1_a.tga",
        ];

        let mut texture_loaded = false;
        for texture_path in texture_paths {
            if std::path::Path::new(&texture_path).exists() {
                if let Ok(data) = std::fs::read(&texture_path) {
                    if let Ok(img) = image::load_from_memory(&data) {
                        let img = img.to_rgba8();
                        let size = Extent3d {
                            width: img.width(),
                            height: img.height(),
                            depth_or_array_layers: 1,
                        };
                        let texture = self.device.create_texture(&TextureDescriptor {
                            label: Some("Wall Texture"),
                            size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::Rgba8UnormSrgb,
                            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                            view_formats: &[],
                        });

                        self.queue.write_texture(
                            ImageCopyTexture {
                                texture: &texture,
                                mip_level: 0,
                                origin: Origin3d::ZERO,
                                aspect: TextureAspect::All,
                            },
                            &img,
                            ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * img.width()),
                                rows_per_image: Some(img.height()),
                            },
                            size,
                        );

                        let view = texture.create_view(&TextureViewDescriptor::default());
                        let sampler = self.device.create_sampler(&SamplerDescriptor {
                            address_mode_u: AddressMode::Repeat,
                            address_mode_v: AddressMode::Repeat,
                            address_mode_w: AddressMode::Repeat,
                            mag_filter: FilterMode::Linear,
                            min_filter: FilterMode::Linear,
                            mipmap_filter: FilterMode::Linear,
                            ..Default::default()
                        });

                        self.wall_texture = Some(WgpuTexture {
                            texture,
                            view,
                            sampler,
                        });
                        texture_loaded = true;
                        println!("Loaded wall texture from: {}", texture_path);
                        break;
                    }
                }
            }
        }

        let curb_texture_paths = vec![
            "../q3-resources/textures/base_trim/border11.png",
            "../q3-resources/textures/base_trim/border11.jpg",
            "../q3-resources/textures/base_trim/border11.tga",
            "../q3-resources/textures/base_trim/spiderbit4.png",
            "../q3-resources/textures/base_trim/spiderbit4.jpg",
            "../q3-resources/textures/base_trim/spiderbit4.tga",
            "../q3-resources/textures/base_trim/dirty_pewter_big.png",
            "../q3-resources/textures/base_trim/dirty_pewter_big.jpg",
            "../q3-resources/textures/base_trim/dirty_pewter_big.tga",
            "../q3-resources/textures/base_trim/rusty_pewter_big.png",
            "../q3-resources/textures/base_trim/rusty_pewter_big.jpg",
            "../q3-resources/textures/base_trim/rusty_pewter_big.tga",
            "../q3-resources/textures/base_trim/metal2_2.png",
            "../q3-resources/textures/base_trim/metal2_2.jpg",
            "../q3-resources/textures/base_trim/metal2_2.tga",
            "../q3-resources/textures/base_trim/pewter.png",
            "../q3-resources/textures/base_trim/pewter.jpg",
            "../q3-resources/textures/base_trim/pewter.tga",
            "../q3-resources/textures/base_trim/tin.png",
            "../q3-resources/textures/base_trim/tin.jpg",
            "../q3-resources/textures/base_trim/tin.tga",
            "q3-resources/textures/base_trim/border11.png",
            "q3-resources/textures/base_trim/border11.jpg",
            "q3-resources/textures/base_trim/border11.tga",
            "q3-resources/textures/base_trim/spiderbit4.png",
            "q3-resources/textures/base_trim/spiderbit4.jpg",
            "q3-resources/textures/base_trim/spiderbit4.tga",
            "q3-resources/textures/base_trim/dirty_pewter_big.png",
            "q3-resources/textures/base_trim/dirty_pewter_big.jpg",
            "q3-resources/textures/base_trim/dirty_pewter_big.tga",
            "q3-resources/textures/base_trim/rusty_pewter_big.png",
            "q3-resources/textures/base_trim/rusty_pewter_big.jpg",
            "q3-resources/textures/base_trim/rusty_pewter_big.tga",
            "q3-resources/textures/base_trim/metal2_2.png",
            "q3-resources/textures/base_trim/metal2_2.jpg",
            "q3-resources/textures/base_trim/metal2_2.tga",
            "q3-resources/textures/base_trim/pewter.png",
            "q3-resources/textures/base_trim/pewter.jpg",
            "q3-resources/textures/base_trim/pewter.tga",
            "q3-resources/textures/base_trim/tin.png",
            "q3-resources/textures/base_trim/tin.jpg",
            "q3-resources/textures/base_trim/tin.tga",
        ];

        let mut curb_texture_loaded = false;
        for texture_path in curb_texture_paths {
            if std::path::Path::new(&texture_path).exists() {
                if let Ok(data) = std::fs::read(&texture_path) {
                    if let Ok(img) = image::load_from_memory(&data) {
                        let img = img.to_rgba8();
                        let size = Extent3d {
                            width: img.width(),
                            height: img.height(),
                            depth_or_array_layers: 1,
                        };
                        let texture = self.device.create_texture(&TextureDescriptor {
                            label: Some("Wall Curb Texture"),
                            size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::Rgba8UnormSrgb,
                            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                            view_formats: &[],
                        });

                        self.queue.write_texture(
                            ImageCopyTexture {
                                texture: &texture,
                                mip_level: 0,
                                origin: Origin3d::ZERO,
                                aspect: TextureAspect::All,
                            },
                            &img,
                            ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * img.width()),
                                rows_per_image: Some(img.height()),
                            },
                            size,
                        );

                        let view = texture.create_view(&TextureViewDescriptor::default());
                        let sampler = self.device.create_sampler(&SamplerDescriptor {
                            address_mode_u: AddressMode::Repeat,
                            address_mode_v: AddressMode::Repeat,
                            address_mode_w: AddressMode::Repeat,
                            mag_filter: FilterMode::Linear,
                            min_filter: FilterMode::Linear,
                            mipmap_filter: FilterMode::Linear,
                            ..Default::default()
                        });

                        self.wall_curb_texture = Some(WgpuTexture {
                            texture,
                            view,
                            sampler,
                        });
                        curb_texture_loaded = true;
                        println!("Loaded wall curb texture from: {}", texture_path);
                        break;
                    }
                }
            }
        }

        if !curb_texture_loaded {
            println!("Warning: Could not load wall curb texture, creating fallback");
            let size = 128u32;
            let mut pixels = Vec::with_capacity((size * size * 4) as usize);
            
            for y in 0..size {
                for x in 0..size {
                    let fx = x as f32 / size as f32;
                    let fy = y as f32 / size as f32;
                    
                    let rust_pattern = (fx * 4.0).sin() * (fy * 4.0).cos();
                    let base_rust_r = 0.4;
                    let base_rust_g = 0.25;
                    let base_rust_b = 0.15;
                    let rust_highlight_r = 0.6;
                    let rust_highlight_g = 0.35;
                    let rust_highlight_b = 0.2;
                    let mix_factor = rust_pattern * 0.5 + 0.5;
                    let rust_r = base_rust_r + (rust_highlight_r - base_rust_r) * mix_factor;
                    let rust_g = base_rust_g + (rust_highlight_g - base_rust_g) * mix_factor;
                    let rust_b = base_rust_b + (rust_highlight_b - base_rust_b) * mix_factor;
                    
                    let rivet_dx = fx - 0.5;
                    let rivet_dy = fy - 0.5;
                    let rivet_dist = (rivet_dx * rivet_dx + rivet_dy * rivet_dy).sqrt();
                    let rivet = if rivet_dist < 0.1 {
                        1.0 - (rivet_dist - 0.1) / 0.05
                    } else if rivet_dist < 0.15 {
                        1.0 - (rivet_dist - 0.1) / 0.05
                    } else {
                        0.0
                    };
                    let rivet_r = rust_r + (0.7 - rust_r) * rivet;
                    let rivet_g = rust_g + (0.6 - rust_g) * rivet;
                    let rivet_b = rust_b + (0.4 - rust_b) * rivet;
                    
                    pixels.push((rivet_r * 255.0) as u8);
                    pixels.push((rivet_g * 255.0) as u8);
                    pixels.push((rivet_b * 255.0) as u8);
                    pixels.push(255);
                }
            }
            
            let texture = self.device.create_texture(&TextureDescriptor {
                label: Some("Wall Curb Texture Fallback"),
                size: Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });

            self.queue.write_texture(
                ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &pixels,
                ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * size),
                    rows_per_image: Some(size),
                },
                Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );

            let view = texture.create_view(&TextureViewDescriptor::default());
            let sampler = self.device.create_sampler(&SamplerDescriptor {
                address_mode_u: AddressMode::Repeat,
                address_mode_v: AddressMode::Repeat,
                address_mode_w: AddressMode::Repeat,
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                mipmap_filter: FilterMode::Linear,
                ..Default::default()
            });

            self.wall_curb_texture = Some(WgpuTexture {
                texture,
                view,
                sampler,
            });
        }

        if !texture_loaded {
            println!("Warning: Could not load wall texture, using fallback");
            let size = 128u32;
            let mut pixels = Vec::with_capacity((size * size * 4) as usize);
            
            for y in 0..size {
                for x in 0..size {
                    let fx = x as f32 / size as f32;
                    let fy = y as f32 / size as f32;
                    
                    let noise_x = (fx * 8.0 + (fy * 3.14159).sin() * 0.3).fract();
                    let noise_y = (fy * 8.0 + (fx * 2.71828).cos() * 0.3).fract();
                    
                    let stone_pattern = (noise_x * 3.14159).sin() * (noise_y * 2.71828).cos();
                    let base_gray = 0.4 + stone_pattern * 0.15;
                    
                    let r = base_gray + (fx * 10.0).sin() * 0.05;
                    let g = base_gray + (fy * 7.0).cos() * 0.05;
                    let b = base_gray + ((fx + fy) * 5.0).sin() * 0.05;
                    
                    pixels.push((r * 255.0) as u8);
                    pixels.push((g * 255.0) as u8);
                    pixels.push((b * 255.0) as u8);
                    pixels.push(255);
                }
            }
            
            let texture = self.device.create_texture(&TextureDescriptor {
                label: Some("Wall Texture Fallback"),
                size: Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });

            self.queue.write_texture(
                ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &pixels,
                ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * size),
                    rows_per_image: Some(size),
                },
                Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );

            let view = texture.create_view(&TextureViewDescriptor::default());
            let sampler = self.device.create_sampler(&SamplerDescriptor {
                address_mode_u: AddressMode::Repeat,
                address_mode_v: AddressMode::Repeat,
                address_mode_w: AddressMode::Repeat,
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                mipmap_filter: FilterMode::Linear,
                ..Default::default()
            });

            self.wall_texture = Some(WgpuTexture {
                texture,
                view,
                sampler,
            });
        }
    }

    fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    pub fn create_smoke_texture(&mut self) {
        let candidates = vec![
            "q3-resources/gfx/misc/smokepuff2b.png",
            "q3-resources/gfx/misc/smokepuff2b.tga",
            "q3-resources/gfx/misc/smokepuff3.png",
            "q3-resources/gfx/misc/smokepuff3.tga",
            "../q3-resources/gfx/misc/smokepuff2b.png",
            "../q3-resources/gfx/misc/smokepuff2b.tga",
            "../q3-resources/gfx/misc/smokepuff3.png",
            "../q3-resources/gfx/misc/smokepuff3.tga",
        ];

        let mut texture_loaded = false;
        for path in candidates {
            if std::path::Path::new(path).exists() {
                if let Ok(data) = std::fs::read(path) {
                    if let Ok(img) = image::load_from_memory(&data) {
                        let img = img.to_rgba8();
                        let size = Extent3d {
                            width: img.width(),
                            height: img.height(),
                            depth_or_array_layers: 1,
                        };
                        let texture = self.device.create_texture(&TextureDescriptor {
                            label: Some("Smoke Texture"),
                            size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::Rgba8Unorm,
                            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                            view_formats: &[],
                        });

                        self.queue.write_texture(
                            ImageCopyTexture {
                                texture: &texture,
                                mip_level: 0,
                                origin: Origin3d::ZERO,
                                aspect: TextureAspect::All,
                            },
                            &img,
                            ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * img.width()),
                                rows_per_image: Some(img.height()),
                            },
                            size,
                        );

                        let view = texture.create_view(&TextureViewDescriptor::default());
                        let sampler = self.device.create_sampler(&SamplerDescriptor {
                            address_mode_u: AddressMode::ClampToEdge,
                            address_mode_v: AddressMode::ClampToEdge,
                            address_mode_w: AddressMode::ClampToEdge,
                            mag_filter: FilterMode::Linear,
                            min_filter: FilterMode::Linear,
                            mipmap_filter: FilterMode::Linear,
                            ..Default::default()
                        });

                        self.smoke_texture = Some(WgpuTexture {
                            texture,
                            view,
                            sampler,
                        });
                        texture_loaded = true;
                        break;
                    }
                }
            }
        }

        if !texture_loaded {
            let size = 64u32;
            let mut pixels = Vec::with_capacity((size * size * 4) as usize);
            let center = size as f32 / 2.0;
            for y in 0..size {
                for x in 0..size {
                    let fx = x as f32;
                    let fy = y as f32;
                    let dx = fx - center;
                    let dy = fy - center;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let max_dist = center * 0.9;
                    let normalized_dist = (dist / max_dist).min(1.0);
                    let alpha = Self::smoothstep(1.0, 0.3, normalized_dist);
                    let base_color = 0.8;
                    pixels.push((base_color * 255.0) as u8);
                    pixels.push((base_color * 255.0) as u8);
                    pixels.push((base_color * 255.0) as u8);
                    pixels.push((alpha.min(1.0) * 255.0) as u8);
                }
            }
            let texture = self.device.create_texture(&TextureDescriptor {
                label: Some("Smoke Texture Fallback"),
                size: Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &pixels,
                ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * size),
                    rows_per_image: Some(size),
                },
                Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );
            let view = texture.create_view(&TextureViewDescriptor::default());
            let sampler = self.device.create_sampler(&SamplerDescriptor {
                address_mode_u: AddressMode::ClampToEdge,
                address_mode_v: AddressMode::ClampToEdge,
                address_mode_w: AddressMode::ClampToEdge,
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                mipmap_filter: FilterMode::Linear,
                ..Default::default()
            });
            self.smoke_texture = Some(WgpuTexture {
                texture,
                view,
                sampler,
            });
        }
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
        if self.wall_texture.is_none() {
            self.create_wall_texture();
        }

        let uniforms = self.create_uniforms(
            view_proj,
            Mat4::IDENTITY,
            camera_pos,
            lights,
            ambient_light,
        );

        if self.wall_uniform_buffer.is_none() {
            self.wall_uniform_buffer = Some(self.device.create_buffer(&BufferDescriptor {
                label: Some("Wall Uniform Buffer"),
                size: std::mem::size_of::<MD3Uniforms>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if self.wall_curb_texture.is_none() {
            self.create_wall_texture();
        }

        if self.wall_bind_group.is_none() {
            let wall_uniform_buffer = self.wall_uniform_buffer.as_ref().unwrap();
            let wall_tex = self.wall_texture.as_ref().unwrap();
            let curb_tex = self.wall_curb_texture.as_ref().unwrap_or_else(|| {
                println!("Error: wall_curb_texture is None, using wall_texture as fallback");
                wall_tex
            });
            self.wall_bind_group = Some(self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Wall Bind Group"),
                layout: &self.wall_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: wall_uniform_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&wall_tex.view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Sampler(&wall_tex.sampler),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::TextureView(&curb_tex.view),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: BindingResource::Sampler(&curb_tex.sampler),
                    },
                ],
            }));
        }

        let wall_uniform_buffer = self.wall_uniform_buffer.as_ref().unwrap();
        self.update_uniform_buffer(&uniforms, wall_uniform_buffer);

        let pipeline = self.wall_pipeline.as_ref().unwrap();
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
        render_pass.set_bind_group(0, self.wall_bind_group.as_ref().unwrap(), &[]);
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

        let uniform_buffer = Arc::new(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Model Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: BufferUsages::UNIFORM,
        }));

        let shadow_uniform_buffer = if render_shadow {
            Some(Arc::new(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Model Shadow Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: BufferUsages::UNIFORM,
            })))
        } else {
            None
        };
        
        let mesh_data = self.prepare_mesh_data(
            model,
            frame_idx,
            texture_paths,
            uniform_buffer.clone(),
            shadow_uniform_buffer,
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

        if render_shadow && !lights.is_empty() {
            for light_idx in 0..lights.len() {
                let single_light = &[lights[light_idx]];
                let shadow_uniforms = self.create_uniforms(
                    view_proj,
                    model_matrix,
                    camera_pos,
                    single_light,
                    ambient_light,
                );
                
                let shadow_buffer = Arc::new(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Model Shadow Uniform Buffer"),
                    contents: bytemuck::cast_slice(&[shadow_uniforms]),
                    usage: BufferUsages::UNIFORM,
                }));
                
                let shadow_mesh_data = self.prepare_mesh_data(
                    model,
                    frame_idx,
                    texture_paths,
                    uniform_buffer.clone(),
                    Some(shadow_buffer),
                    true,
                );
                
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
                            load: if light_idx == 0 { LoadOp::Clear(0) } else { LoadOp::Load },
                            store: StoreOp::Store,
                        }),
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                shadow_pass.set_pipeline(shadow_pipeline);
                shadow_pass.set_stencil_reference(0);

                for mesh in &shadow_mesh_data {
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
        if self.wall_shadow_pipeline.is_none() || models.is_empty() || lights.is_empty() {
            return;
        }

        for light_idx in 0..lights.len() {
            let single_light = &[lights[light_idx]];
            let mut all_mesh_data = Vec::new();

            for (model, frame_idx, texture_paths, model_matrix) in models {
                let uniforms = self.create_uniforms(
                    view_proj,
                    *model_matrix,
                    camera_pos,
                    single_light,
                    ambient_light,
                );

                let uniform_buffer = Arc::new(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Wall Shadow Model Uniform Buffer"),
                    contents: bytemuck::cast_slice(&[uniforms]),
                    usage: BufferUsages::UNIFORM,
                }));

                let mesh_data = self.prepare_mesh_data(
                    model,
                    *frame_idx,
                    texture_paths,
                    uniform_buffer,
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
                        load: if light_idx == 0 { LoadOp::Clear(0) } else { LoadOp::Load },
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
        if self.particle_pipeline.is_none() 
            || self.particle_quad_vertex_buffer.is_none()
            || self.particle_quad_index_buffer.is_none()
            || self.particle_instance_buffer.is_none()
            || self.particle_uniform_buffer.is_none()
            || particles.is_empty() {
            return;
        }

        if self.smoke_texture.is_none() {
            self.create_smoke_texture();
        }

        if self.particle_bind_group.is_none() {
            let smoke_tex = self.smoke_texture.as_ref().unwrap();
            let particle_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Particle Bind Group"),
                layout: &self.particle_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: self.particle_uniform_buffer.as_ref().unwrap().as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&smoke_tex.view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Sampler(&smoke_tex.sampler),
                    },
                ],
            });
            self.particle_bind_group = Some(particle_bind_group);
        }

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct ParticleUniforms {
            view_proj: [[f32; 4]; 4],
            camera_pos: [f32; 4],
        }

        let uniforms = ParticleUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 0.0],
        };

        if let Some(ref uniform_buffer) = self.particle_uniform_buffer {
            self.queue.write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        }

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct ParticleInstance {
            position_size: [f32; 4],
            alpha: f32,
            _padding: [f32; 3],
        }

        let mut instance_data: Vec<ParticleInstance> = Vec::with_capacity(particles.len());
        for (position, size, alpha) in particles {
            instance_data.push(ParticleInstance {
                position_size: [position.x, position.y, position.z, *size],
                alpha: *alpha,
                _padding: [0.0; 3],
            });
        }

        if !instance_data.is_empty() {
            self.queue.write_buffer(
                self.particle_instance_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&instance_data),
            );
        }

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
        render_pass.set_bind_group(0, self.particle_bind_group.as_ref().unwrap(), &[]);
        render_pass.set_vertex_buffer(0, self.particle_quad_vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.set_vertex_buffer(1, self.particle_instance_buffer.as_ref().unwrap().slice(..));
        render_pass.set_index_buffer(self.particle_quad_index_buffer.as_ref().unwrap().slice(..), IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..particles.len() as u32);
    }

    pub fn render_flames(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        camera_pos: Vec3,
        flames: &[(Vec3, f32, u32)],
    ) {
        if self.flame_pipeline.is_none()
            || self.particle_quad_vertex_buffer.is_none()
            || self.particle_quad_index_buffer.is_none()
            || self.flame_instance_buffer.is_none()
            || self.flame_uniform_buffer.is_none()
            || self.flame_bind_group.is_none()
            || flames.is_empty() {
            return;
        }

        if self.flame_texture.is_none() {
            self.create_flame_texture();
        }

        if self.flame_bind_group.is_none() && self.flame_texture.is_some() {
            let flame_tex = self.flame_texture.as_ref().unwrap();
            let flame_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Flame Bind Group"),
                layout: &self.particle_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: self.flame_uniform_buffer.as_ref().unwrap().as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&flame_tex.view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Sampler(&flame_tex.sampler),
                    },
                ],
            });
            self.flame_bind_group = Some(flame_bind_group);
        }

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct FlameUniforms {
            view_proj: [[f32; 4]; 4],
            camera_pos: [f32; 4],
        }

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct FlameInstance {
            position_size: [f32; 4],
        }

        let uniforms = FlameUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 0.0],
        };

        if let Some(ref uniform_buffer) = self.flame_uniform_buffer {
            self.queue.write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        }

        let mut instance_data: Vec<FlameInstance> = Vec::with_capacity(flames.len());
        for (position, size, _texture_index) in flames {
            instance_data.push(FlameInstance {
                position_size: [position.x, position.y, position.z, *size],
            });
        }

        if !instance_data.is_empty() {
            self.queue.write_buffer(
                self.flame_instance_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&instance_data),
            );
        }

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
        render_pass.set_bind_group(0, self.flame_bind_group.as_ref().unwrap(), &[]);
        render_pass.set_vertex_buffer(0, self.particle_quad_vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.set_vertex_buffer(1, self.flame_instance_buffer.as_ref().unwrap().slice(..));
        render_pass.set_index_buffer(self.particle_quad_index_buffer.as_ref().unwrap().slice(..), IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..flames.len() as u32);
    }

    fn init_debug_light_sphere(&mut self, surface_format: TextureFormat) {
        if self.debug_sphere_vertex_buffer.is_some() {
            return;
        }

        let segments = 16;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for i in 0..=segments {
            let theta = std::f32::consts::PI * i as f32 / segments as f32;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            for j in 0..=segments {
                let phi = 2.0 * std::f32::consts::PI * j as f32 / segments as f32;
                let sin_phi = phi.sin();
                let cos_phi = phi.cos();

                let x = cos_phi * sin_theta;
                let y = cos_theta;
                let z = sin_phi * sin_theta;

                vertices.push(VertexData {
                    position: [x, y, z],
                    uv: [j as f32 / segments as f32, i as f32 / segments as f32],
                    color: [1.0, 1.0, 1.0, 1.0],
                    normal: [x, y, z],
                });
            }
        }

        for i in 0..segments {
            for j in 0..segments {
                let first = (i * (segments + 1) + j) as u16;
                let second = (first + segments + 1) as u16;

                indices.push(first);
                indices.push(second);
                indices.push(first + 1);

                indices.push(second);
                indices.push(second + 1);
                indices.push(first + 1);
            }
        }

        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Debug Sphere Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Debug Sphere Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: BufferUsages::INDEX,
        });

        let instance_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Debug Sphere Instance Buffer"),
            size: 1024 * std::mem::size_of::<[f32; 8]>() as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.debug_sphere_vertex_buffer = Some(vertex_buffer);
        self.debug_sphere_index_buffer = Some(index_buffer);
        self.debug_sphere_instance_buffer = Some(instance_buffer);

        let uniform_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Debug Light Sphere Uniform Buffer"),
            size: std::mem::size_of::<[[f32; 4]; 4]>() as u64 * 2,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Debug Light Sphere Bind Group"),
            layout: &self.debug_light_sphere_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        self.debug_light_sphere_uniform_buffer = Some(uniform_buffer);
        self.debug_light_sphere_bind_group = Some(bind_group);

        let shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Debug Light Sphere Shader"),
            source: ShaderSource::Wgsl(DEBUG_LIGHT_SPHERE_SHADER.into()),
        });

        let pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Debug Light Sphere Pipeline Layout"),
            bind_group_layouts: &[&self.debug_light_sphere_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Debug Light Sphere Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    VertexData::desc(),
                    VertexBufferLayout {
                        array_stride: std::mem::size_of::<[f32; 8]>() as BufferAddress,
                        step_mode: VertexStepMode::Instance,
                        attributes: &[
                            VertexAttribute {
                                offset: 0,
                                shader_location: 4,
                                format: VertexFormat::Float32x4,
                            },
                            VertexAttribute {
                                offset: std::mem::size_of::<[f32; 4]>() as BufferAddress,
                                shader_location: 5,
                                format: VertexFormat::Float32x4,
                            },
                        ],
                    },
                ],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(Self::create_color_target_state(surface_format))],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: Self::create_primitive_state(Some(Face::Back)),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: true,
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: Self::create_multisample_state(),
            multiview: None,
        });

        self.debug_light_sphere_pipeline = Some(pipeline);
    }

    fn init_debug_light_ray(&mut self, surface_format: TextureFormat) {
        if self.debug_ray_vertex_buffer.is_some() {
            return;
        }

        let uniform_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Debug Light Ray Uniform Buffer"),
            size: std::mem::size_of::<[[f32; 4]; 4]>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Debug Light Ray Bind Group"),
            layout: &self.debug_light_ray_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        self.debug_light_ray_uniform_buffer = Some(uniform_buffer);
        self.debug_light_ray_bind_group = Some(bind_group);

        let shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Debug Light Ray Shader"),
            source: ShaderSource::Wgsl(DEBUG_LIGHT_RAY_SHADER.into()),
        });

        let pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Debug Light Ray Pipeline Layout"),
            bind_group_layouts: &[&self.debug_light_ray_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Debug Light Ray Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 7]>() as BufferAddress,
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
                            format: VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(Self::create_color_target_state(surface_format))],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: true,
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: Self::create_multisample_state(),
            multiview: None,
        });

        self.debug_light_ray_pipeline = Some(pipeline);
    }

    pub fn render_debug_lights(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        camera_pos: Vec3,
        lights: &[(Vec3, Vec3, f32)],
        surface_format: TextureFormat,
    ) {
        if lights.is_empty() {
            return;
        }

        self.init_debug_light_sphere(surface_format);
        self.init_debug_light_ray(surface_format);

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct DebugLightSphereUniforms {
            view_proj: [[f32; 4]; 4],
            camera_pos: [f32; 4],
        }

        let uniforms = DebugLightSphereUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 0.0],
        };

        if let Some(ref uniform_buffer) = self.debug_light_sphere_uniform_buffer {
            self.queue.write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        }

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct SphereInstance {
            position_radius: [f32; 4],
            light_color: [f32; 4],
        }

        let mut instance_data: Vec<SphereInstance> = Vec::with_capacity(lights.len());
        for (position, color, radius) in lights {
            instance_data.push(SphereInstance {
                position_radius: [position.x, position.y, position.z, *radius * 0.1],
                light_color: [color.x, color.y, color.z, 1.0],
            });
        }

        if !instance_data.is_empty() {
            self.queue.write_buffer(
                self.debug_sphere_instance_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&instance_data),
            );
        }

        let pipeline = self.debug_light_sphere_pipeline.as_ref().unwrap();
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Debug Light Sphere Render Pass"),
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
        render_pass.set_bind_group(0, self.debug_light_sphere_bind_group.as_ref().unwrap(), &[]);
        render_pass.set_vertex_buffer(0, self.debug_sphere_vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.set_vertex_buffer(1, self.debug_sphere_instance_buffer.as_ref().unwrap().slice(..));
        render_pass.set_index_buffer(self.debug_sphere_index_buffer.as_ref().unwrap().slice(..), IndexFormat::Uint16);
        
        let num_indices = 16 * 16 * 6;
        render_pass.draw_indexed(0..num_indices, 0, 0..lights.len() as u32);
    }

    pub fn render_debug_light_rays(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        lights: &[(Vec3, Vec3, f32)],
        surface_format: TextureFormat,
    ) {
        if lights.is_empty() {
            return;
        }

        self.init_debug_light_ray(surface_format);

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct DebugLightRayUniforms {
            view_proj: [[f32; 4]; 4],
        }

        let uniforms = DebugLightRayUniforms {
            view_proj: view_proj.to_cols_array_2d(),
        };

        if let Some(ref uniform_buffer) = self.debug_light_ray_uniform_buffer {
            self.queue.write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        }

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct RayVertex {
            position: [f32; 3],
            color: [f32; 4],
        }

        let mut vertices = Vec::new();
        let ground_y = 0.0;
        
        for (light_pos, light_color, radius) in lights {
            let ray_color = [light_color.x * 0.5, light_color.y * 0.5, light_color.z * 0.5, 0.6];
            
            let num_rays = 8;
            for i in 0..num_rays {
                let angle = 2.0 * std::f32::consts::PI * i as f32 / num_rays as f32;
                let dir_x = angle.cos();
                let dir_z = angle.sin();
                
                let end_pos = Vec3::new(
                    light_pos.x + dir_x * radius * 0.5,
                    light_pos.y + 0.01,
                    light_pos.z + dir_z * radius * 0.5,
                );
                
                vertices.push(RayVertex {
                    position: [light_pos.x, light_pos.y, light_pos.z],
                    color: ray_color,
                });
                vertices.push(RayVertex {
                    position: [end_pos.x, end_pos.y, end_pos.z],
                    color: [ray_color[0], ray_color[1], ray_color[2], 0.0],
                });
            }
        }

        if vertices.is_empty() {
            return;
        }

        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Debug Ray Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX,
        });

        self.debug_ray_vertex_buffer = Some(vertex_buffer);

        let pipeline = self.debug_light_ray_pipeline.as_ref().unwrap();
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Debug Light Ray Render Pass"),
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
        render_pass.set_bind_group(0, self.debug_light_ray_bind_group.as_ref().unwrap(), &[]);
        render_pass.set_vertex_buffer(0, self.debug_ray_vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.draw(0..vertices.len() as u32, 0..1);
    }

    fn build_silhouette_cache(&mut self, model: &MD3Model, mesh_idx: usize) -> Option<()> {
        if mesh_idx >= model.meshes.len() {
            return None;
        }

        let model_id = std::ptr::addr_of!(*model) as usize;
        let cache_key = (model_id, mesh_idx);

        if self.silhouette_cache.contains_key(&cache_key) {
            return Some(());
        }

        let mesh = &model.meshes[mesh_idx];
        let triangles = &mesh.triangles;

        let mut edge_map: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        let mut triangle_neighbors = vec![[None; 3]; triangles.len()];

        for (tri_idx, triangle) in triangles.iter().enumerate() {
            let v0 = triangle.vertex[0] as usize;
            let v1 = triangle.vertex[1] as usize;
            let v2 = triangle.vertex[2] as usize;

            let edges = [
                (v0.min(v1), v0.max(v1)),
                (v1.min(v2), v1.max(v2)),
                (v2.min(v0), v2.max(v0)),
            ];

            for (edge_idx, edge) in edges.iter().enumerate() {
                edge_map.entry(*edge).or_insert_with(Vec::new).push(tri_idx);
                
                if let Some(neighbors) = edge_map.get(edge) {
                    if neighbors.len() > 1 {
                        for &neighbor_tri in neighbors {
                            if neighbor_tri != tri_idx {
                                triangle_neighbors[tri_idx][edge_idx] = Some(neighbor_tri);
                            }
                        }
                    }
                }
            }
        }

        let mut edges = Vec::new();
        for ((v0, v1), tris) in edge_map.iter() {
            if tris.len() == 1 {
                edges.push(Edge { v0: *v0, v1: *v1 });
            }
        }

        self.silhouette_cache.insert(cache_key, ModelSilhouetteCache {
            edges,
            triangle_neighbors,
        });

        Some(())
    }

    fn extract_silhouette_edges(
        &mut self,
        model: &MD3Model,
        mesh_idx: usize,
        frame_idx: usize,
        model_matrix: Mat4,
        light_pos: Vec3,
    ) -> Vec<SilhouetteEdge> {
        if mesh_idx >= model.meshes.len() {
            return Vec::new();
        }

        let mesh = &model.meshes[mesh_idx];
        if frame_idx >= mesh.vertices.len() {
            return Vec::new();
        }

        self.build_silhouette_cache(model, mesh_idx);

        let model_id = std::ptr::addr_of!(*model) as usize;
        let cache_key = (model_id, mesh_idx);
        
        let cache = match self.silhouette_cache.get(&cache_key) {
            Some(c) => c,
            None => return Vec::new(),
        };

        let frame_vertices = &mesh.vertices[frame_idx];
        let mut world_positions = Vec::with_capacity(frame_vertices.len());
        
        for vertex in frame_vertices {
            let vertex_data = vertex.vertex;
            let x = vertex_data[0] as f32 * (1.0 / 64.0);
            let y = vertex_data[1] as f32 * (1.0 / 64.0);
            let z = vertex_data[2] as f32 * (1.0 / 64.0);
            let local_pos = Vec3::new(x, y, z);
            let world_pos = model_matrix.transform_point3(local_pos);
            world_positions.push(world_pos);
        }

        let triangles = &mesh.triangles;
        let mut triangle_facing = vec![false; triangles.len()];

        for (tri_idx, triangle) in triangles.iter().enumerate() {
            let v0 = world_positions[triangle.vertex[0] as usize];
            let v1 = world_positions[triangle.vertex[1] as usize];
            let v2 = world_positions[triangle.vertex[2] as usize];

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let normal = edge1.cross(edge2);

            let to_light = light_pos - v0;
            triangle_facing[tri_idx] = normal.dot(to_light) > 0.0;
        }

        let mut silhouette_edges = Vec::new();

        for (tri_idx, triangle) in triangles.iter().enumerate() {
            let v0_idx = triangle.vertex[0] as usize;
            let v1_idx = triangle.vertex[1] as usize;
            let v2_idx = triangle.vertex[2] as usize;

            let edges = [
                (v0_idx, v1_idx, 0),
                (v1_idx, v2_idx, 1),
                (v2_idx, v0_idx, 2),
            ];

            for (edge_v0, edge_v1, edge_idx) in edges {
                if let Some(neighbor_tri) = cache.triangle_neighbors[tri_idx][edge_idx] {
                    if triangle_facing[tri_idx] != triangle_facing[neighbor_tri] {
                        silhouette_edges.push(SilhouetteEdge {
                            v0: world_positions[edge_v0],
                            v1: world_positions[edge_v1],
                        });
                    }
                } else if triangle_facing[tri_idx] {
                    silhouette_edges.push(SilhouetteEdge {
                        v0: world_positions[edge_v0],
                        v1: world_positions[edge_v1],
                    });
                }
            }
        }

        silhouette_edges
    }

    fn build_shadow_volume(
        &self,
        silhouette_edges: &[SilhouetteEdge],
        cap_triangles: &[[Vec3; 3]],
        light_pos: Vec3,
        extrude_distance: f32,
    ) -> (Vec<ShadowVolumeVertex>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for edge in silhouette_edges {
            let v0_near = edge.v0;
            let v1_near = edge.v1;

            let dir0 = (v0_near - light_pos).normalize();
            let dir1 = (v1_near - light_pos).normalize();

            let base_idx = vertices.len() as u16;

            vertices.push(ShadowVolumeVertex {
                position: [v0_near.x, v0_near.y, v0_near.z],
                extrude_dir: [0.0, 0.0, 0.0],
            });
            vertices.push(ShadowVolumeVertex {
                position: [v1_near.x, v1_near.y, v1_near.z],
                extrude_dir: [0.0, 0.0, 0.0],
            });
            vertices.push(ShadowVolumeVertex {
                position: [v0_near.x, v0_near.y, v0_near.z],
                extrude_dir: [dir0.x, dir0.y, dir0.z],
            });
            vertices.push(ShadowVolumeVertex {
                position: [v1_near.x, v1_near.y, v1_near.z],
                extrude_dir: [dir1.x, dir1.y, dir1.z],
            });

            indices.push(base_idx);
            indices.push(base_idx + 1);
            indices.push(base_idx + 2);

            indices.push(base_idx + 1);
            indices.push(base_idx + 3);
            indices.push(base_idx + 2);
        }

        // caps
        for tri in cap_triangles {
            let base_near = vertices.len() as u16;
            vertices.push(ShadowVolumeVertex { position: [tri[0].x, tri[0].y, tri[0].z], extrude_dir: [0.0, 0.0, 0.0] });
            vertices.push(ShadowVolumeVertex { position: [tri[1].x, tri[1].y, tri[1].z], extrude_dir: [0.0, 0.0, 0.0] });
            vertices.push(ShadowVolumeVertex { position: [tri[2].x, tri[2].y, tri[2].z], extrude_dir: [0.0, 0.0, 0.0] });
            indices.push(base_near);
            indices.push(base_near + 1);
            indices.push(base_near + 2);

            let base_far = vertices.len() as u16;
            let extr0 = tri[0] + (tri[0] - light_pos).normalize() * extrude_distance;
            let extr1 = tri[1] + (tri[1] - light_pos).normalize() * extrude_distance;
            let extr2 = tri[2] + (tri[2] - light_pos).normalize() * extrude_distance;
            vertices.push(ShadowVolumeVertex { position: [extr0.x, extr0.y, extr0.z], extrude_dir: [0.0, 0.0, 0.0] });
            vertices.push(ShadowVolumeVertex { position: [extr1.x, extr1.y, extr1.z], extrude_dir: [0.0, 0.0, 0.0] });
            vertices.push(ShadowVolumeVertex { position: [extr2.x, extr2.y, extr2.z], extrude_dir: [0.0, 0.0, 0.0] });
            // reverse winding for far cap
            indices.push(base_far);
            indices.push(base_far + 2);
            indices.push(base_far + 1);
        }

        (vertices, indices)
    }

    fn project_triangles_to_plane(
        triangles: &[[Vec3; 3]],
        light_pos: Vec3,
        plane_normal: Vec3,
        plane_d: f32,
    ) -> Vec<[f32; 3]> {
        let eps = 0.002;
        let mut out = Vec::new();
        for tri in triangles {
            let mut projected = Vec::new();
            for v in tri {
                let dir = *v - light_pos;
                let denom = plane_normal.dot(dir);
                if denom.abs() < 1e-4 {
                    continue;
                }
                let t = -(plane_normal.dot(light_pos) + plane_d) / denom;
                if t <= 0.0 {
                    continue;
                }
                let mut p = light_pos + dir * t;
                p += plane_normal * eps;
                projected.push(p);
            }
            if projected.len() == 3 {
                out.push([projected[0].x, projected[0].y, projected[0].z]);
                out.push([projected[1].x, projected[1].y, projected[1].z]);
                out.push([projected[2].x, projected[2].y, projected[2].z]);
            }
        }
        out
    }

    pub fn render_planar_shadows(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        models: &[(
            &MD3Model,
            usize,
            Mat4,
        )],
        lights: &[(Vec3, Vec3, f32)],
    ) {
        if self.shadow_planar_pipeline.is_none() || lights.is_empty() || models.is_empty() {
            return;
        }

        let pipeline = self.shadow_planar_pipeline.as_ref().unwrap();

        for (light_pos, _light_color, _radius) in lights {
            let mut triangles = Vec::new();

            for (model, frame_idx, model_matrix) in models {
                for mesh in &model.meshes {
                    if *frame_idx >= mesh.vertices.len() {
                        continue;
                    }
                    let frame_vertices = &mesh.vertices[*frame_idx];
                    let mut world_positions = Vec::with_capacity(frame_vertices.len());
                    for vertex in frame_vertices {
                        let v = vertex.vertex;
                        let lp = Vec3::new(v[0] as f32 * (1.0 / 64.0), v[1] as f32 * (1.0 / 64.0), v[2] as f32 * (1.0 / 64.0));
                        let wp = (*model_matrix).transform_point3(lp);
                        world_positions.push(wp);
                    }
                    for tri in &mesh.triangles {
                        let a = world_positions[tri.vertex[0] as usize];
                        let b = world_positions[tri.vertex[1] as usize];
                        let c = world_positions[tri.vertex[2] as usize];
                        triangles.push([a, b, c]);
                    }
                }
            }

            if triangles.is_empty() {
                continue;
            }

            // ground y=0
            let ground_proj = Self::project_triangles_to_plane(&triangles, *light_pos, Vec3::new(0.0, 1.0, 0.0), 0.0);
            // wall z=-3.0
            let wall_proj = Self::project_triangles_to_plane(&triangles, *light_pos, Vec3::new(0.0, 0.0, 1.0), 3.0);

            let mut all_proj = Vec::new();
            all_proj.extend(ground_proj);
            all_proj.extend(wall_proj);

            if all_proj.is_empty() {
                continue;
            }

            let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Planar Shadow Vertex Buffer"),
                contents: bytemuck::cast_slice(&all_proj),
                usage: BufferUsages::VERTEX,
            });

            #[repr(C)]
            #[derive(Copy, Clone, Pod, Zeroable)]
            struct ShadowPlanarUniforms {
                view_proj: [[f32; 4]; 4],
                light_pos: [f32; 4],
                extrude_distance: f32,
                _pad: [f32; 3],
            }

            let uniforms = ShadowPlanarUniforms {
                view_proj: view_proj.to_cols_array_2d(),
                light_pos: [light_pos.x, light_pos.y, light_pos.z, 1.0],
                extrude_distance: 0.0,
                _pad: [0.0; 3],
            };

            let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Planar Shadow Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: BufferUsages::UNIFORM,
            });

            let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Planar Shadow Bind Group"),
                layout: &self.shadow_volume_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Planar Shadow Pass"),
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

            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..(all_proj.len() as u32), 0..1);
        }
    }

    pub fn render_shadow_volumes(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        models: &[(
            &MD3Model,
            usize,
            Mat4,
        )],
        lights: &[(Vec3, Vec3, f32)],
    ) {
        if self.shadow_volume_front_pipeline.is_none() || self.shadow_volume_back_pipeline.is_none() {
            println!("Shadow volume pipeline is None!");
            return;
        }
        if models.is_empty() {
            println!("No models for shadows!");
            return;
        }
        if lights.is_empty() {
            println!("No lights for shadows!");
            return;
        }

        println!("render_shadow_volumes: {} models, {} lights", models.len(), lights.len());

        for (light_idx, (light_pos, _light_color, light_radius)) in lights.iter().enumerate() {
            let mut all_silhouette_edges = Vec::new();
            let mut cap_triangles = Vec::new();

            for (model_idx, (model, frame_idx, model_matrix)) in models.iter().enumerate() {
                println!("  Light {}, Model {}: {} meshes, frame={}", light_idx, model_idx, model.meshes.len(), frame_idx);
                
                for mesh_idx in 0..model.meshes.len() {
                    let edges = self.extract_silhouette_edges(
                        model,
                        mesh_idx,
                        *frame_idx,
                        *model_matrix,
                        *light_pos,
                    );
                    println!("    Mesh {}: {} silhouette edges", mesh_idx, edges.len());
                    all_silhouette_edges.extend(edges);

                    // caps: collect triangles in world space
                    let mesh = &model.meshes[mesh_idx];
                    if *frame_idx >= mesh.vertices.len() {
                        continue;
                    }
                    let frame_vertices = &mesh.vertices[*frame_idx];
                    let mut world_positions = Vec::with_capacity(frame_vertices.len());
                    for vertex in frame_vertices {
                        let v = vertex.vertex;
                        let lp = Vec3::new(v[0] as f32 * (1.0 / 64.0), v[1] as f32 * (1.0 / 64.0), v[2] as f32 * (1.0 / 64.0));
                        let wp = (*model_matrix).transform_point3(lp);
                        world_positions.push(wp);
                    }
                    for tri in &mesh.triangles {
                        let a = world_positions[tri.vertex[0] as usize];
                        let b = world_positions[tri.vertex[1] as usize];
                        let c = world_positions[tri.vertex[2] as usize];
                        cap_triangles.push([a, b, c]);
                    }
                }
            }

            println!("  Total silhouette edges: {}", all_silhouette_edges.len());

            if all_silhouette_edges.is_empty() {
                println!("  Skipping light {} - no silhouette edges", light_idx);
                continue;
            }

            let extrude_dist = light_radius.max(20.0) * 4.0;
            let (vertices, indices) = self.build_shadow_volume(&all_silhouette_edges, &cap_triangles, *light_pos, extrude_dist);

            println!("  Shadow volume: {} vertices, {} indices", vertices.len(), indices.len());

            if vertices.is_empty() || indices.is_empty() {
                println!("  Skipping light {} - empty geometry", light_idx);
                continue;
            }

            let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Shadow Volume Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: BufferUsages::VERTEX,
            });

            let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Shadow Volume Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: BufferUsages::INDEX,
            });

            #[repr(C)]
            #[derive(Copy, Clone, Pod, Zeroable)]
            struct ShadowVolumeUniforms {
                view_proj: [[f32; 4]; 4],
                light_pos: [f32; 4],
                extrude_distance: f32,
                _padding: [f32; 3],
            }

            let uniforms = ShadowVolumeUniforms {
                view_proj: view_proj.to_cols_array_2d(),
                light_pos: [light_pos.x, light_pos.y, light_pos.z, 1.0],
                extrude_distance: 100.0,
                _padding: [0.0; 3],
            };

            let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Shadow Volume Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: BufferUsages::UNIFORM,
            });

            let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Shadow Volume Bind Group"),
                layout: &self.shadow_volume_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Shadow Volume Render Pass"),
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

            let pipeline_front = self.shadow_volume_front_pipeline.as_ref().unwrap();
            render_pass.set_pipeline(pipeline_front);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint16);
            render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);

            let pipeline_back = self.shadow_volume_back_pipeline.as_ref().unwrap();
            render_pass.set_pipeline(pipeline_back);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint16);
            render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }

        if self.shadow_apply_pipeline.is_none() || self.shadow_apply_vertex_buffer.is_none() {
            return;
        }

        let mut shadow_apply_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Shadow Apply Pass"),
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
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                }),
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        let shadow_apply_pipeline = self.shadow_apply_pipeline.as_ref().unwrap();
        shadow_apply_pass.set_pipeline(shadow_apply_pipeline);
        shadow_apply_pass.set_stencil_reference(0);
        shadow_apply_pass.set_vertex_buffer(0, self.shadow_apply_vertex_buffer.as_ref().unwrap().slice(..));
        shadow_apply_pass.draw(0..6, 0..1);
    }
}
