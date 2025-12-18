use std::collections::HashMap;
use std::sync::Arc;
use wgpu::*;
use wgpu::util::DeviceExt;
use glam::{Mat4, Vec3};
use crate::engine::md3::MD3Model;
use crate::render::types::*;
use crate::engine::shaders::{MD3_SHADER, MD3_ADDITIVE_SHADER, GROUND_SHADER, SHADOW_SHADER, WALL_SHADOW_SHADER, WALL_SHADER, SHADOW_VOLUME_SHADER, SHADOW_APPLY_SHADER, SHADOW_PLANAR_SHADER, COORDINATE_GRID_SHADER, TILE_SHADER};

use super::buffers::{BufferCacheKey, CachedBuffers};
use super::layouts::*;
use super::pipelines::*;
use super::textures;
use super::shadows::ShadowRenderer;
use super::particles::ParticleRenderer;
use super::debug::DebugRenderer;

pub struct MD3Renderer {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub pipeline: Option<RenderPipeline>,
    pub additive_pipeline: Option<RenderPipeline>,
    pub ground_pipeline: Option<RenderPipeline>,
    pub wall_pipeline: Option<RenderPipeline>,
    pub shadow_pipeline: Option<RenderPipeline>,
    pub wall_shadow_pipeline: Option<RenderPipeline>,
    pub uniform_buffer: Option<Buffer>,
    pub bind_group_layout: BindGroupLayout,
    pub ground_bind_group_layout: BindGroupLayout,
    pub wall_bind_group_layout: BindGroupLayout,
    pub tile_bind_group_layout: BindGroupLayout,
    particle_bind_group_layout: BindGroupLayout,
    pub model_textures: HashMap<String, WgpuTexture>,
    pub ground_vertex_buffer: Option<Buffer>,
    pub ground_index_buffer: Option<Buffer>,
    pub ground_texture: Option<WgpuTexture>,
    pub wall_vertex_buffer: Option<Buffer>,
    pub wall_index_buffer: Option<Buffer>,
    pub wall_texture: Option<WgpuTexture>,
    pub wall_curb_texture: Option<WgpuTexture>,
    pub tile_vertex_buffer: Option<Buffer>,
    pub tile_index_buffer: Option<Buffer>,
    pub tile_num_indices: u32,
    pub tile_texture: Option<WgpuTexture>,
    tile_uniform_buffer: Option<Buffer>,
    tile_bind_group: Option<BindGroup>,
    pub tile_pipeline: Option<RenderPipeline>,
    buffer_cache: HashMap<BufferCacheKey, CachedBuffers>,
    ground_uniform_buffer: Option<Buffer>,
    wall_uniform_buffer: Option<Buffer>,
    ground_bind_group: Option<BindGroup>,
    wall_bind_group: Option<BindGroup>,
    smoke_texture: Option<WgpuTexture>,
    flame_texture: Option<WgpuTexture>,
    debug_light_sphere_bind_group_layout: BindGroupLayout,
    debug_light_ray_bind_group_layout: BindGroupLayout,
    shadow_renderer: Option<ShadowRenderer>,
    particle_renderer: Option<ParticleRenderer>,
    debug_renderer: Option<DebugRenderer>,
    coordinate_grid_pipeline: Option<RenderPipeline>,
    coordinate_grid_vertex_buffer: Option<Buffer>,
    coordinate_grid_index_buffer: Option<Buffer>,
    coordinate_grid_uniform_buffer: Option<Buffer>,
    coordinate_grid_bind_group: Option<BindGroup>,
    coordinate_grid_bind_group_layout: BindGroupLayout,
}

impl MD3Renderer {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        let bind_group_layout = create_md3_bind_group_layout(&device);
        let ground_bind_group_layout = create_ground_bind_group_layout(&device);
        let wall_bind_group_layout = create_wall_bind_group_layout(&device);
        let tile_bind_group_layout = create_tile_bind_group_layout(&device);
        let particle_bind_group_layout = create_particle_bind_group_layout(&device);
        let debug_light_sphere_bind_group_layout = create_debug_light_sphere_bind_group_layout(&device);
        let debug_light_ray_bind_group_layout = create_debug_light_ray_bind_group_layout(&device);

        let coordinate_grid_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Coordinate Grid Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let debug_renderer = Some(DebugRenderer::new(
            device.clone(),
            queue.clone(),
            &debug_light_sphere_bind_group_layout,
            &debug_light_ray_bind_group_layout,
        ));

        Self {
            device,
            queue,
            pipeline: None,
            additive_pipeline: None,
            ground_pipeline: None,
            wall_pipeline: None,
            shadow_pipeline: None,
            wall_shadow_pipeline: None,
            uniform_buffer: None,
            bind_group_layout,
            ground_bind_group_layout,
            wall_bind_group_layout,
            tile_bind_group_layout,
            particle_bind_group_layout,
            model_textures: HashMap::new(),
            ground_vertex_buffer: None,
            ground_index_buffer: None,
            ground_texture: None,
            wall_vertex_buffer: None,
            wall_index_buffer: None,
            wall_texture: None,
            wall_curb_texture: None,
            tile_vertex_buffer: None,
            tile_index_buffer: None,
            tile_num_indices: 0,
            tile_texture: None,
            tile_uniform_buffer: None,
            tile_bind_group: None,
            tile_pipeline: None,
            buffer_cache: HashMap::new(),
            ground_uniform_buffer: None,
            wall_uniform_buffer: None,
            ground_bind_group: None,
            wall_bind_group: None,
            smoke_texture: None,
            flame_texture: None,
            debug_light_sphere_bind_group_layout,
            debug_light_ray_bind_group_layout,
            shadow_renderer: None,
            particle_renderer: None,
            debug_renderer,
            coordinate_grid_pipeline: None,
            coordinate_grid_vertex_buffer: None,
            coordinate_grid_index_buffer: None,
            coordinate_grid_uniform_buffer: None,
            coordinate_grid_bind_group: None,
            coordinate_grid_bind_group_layout,
        }
    }

    pub fn clear_model_cache(&mut self) {
        self.buffer_cache.clear();
        if let Some(ref mut shadow_renderer) = self.shadow_renderer {
            shadow_renderer.clear_cache();
    }
    }

    fn create_uniforms(
        &self,
        view_proj: Mat4,
        model: Mat4,
        camera_pos: Vec3,
        lights: &[(Vec3, Vec3, f32)],
        ambient_light: f32,
    ) -> MD3Uniforms {
        super::buffers::create_uniforms(view_proj, model, camera_pos, lights, ambient_light)
    }

    fn update_uniform_buffer(&self, uniforms: &MD3Uniforms, buffer: &Buffer) {
        super::buffers::update_uniform_buffer(&self.queue, uniforms, buffer);
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
        super::buffers::prepare_mesh_data(
            &mut self.buffer_cache,
            &self.device,
            &self.bind_group_layout,
            &self.model_textures,
            model,
            frame_idx,
            texture_paths,
            uniform_buffer,
            shadow_uniform_buffer,
            render_shadow,
        )
    }

    pub fn load_texture(&mut self, path: &str, texture: WgpuTexture) {
        self.model_textures.insert(path.to_string(), texture);
    }

    fn create_ground_texture(&mut self) {
        self.ground_texture = Some(textures::create_ground_texture(&self.device, &self.queue));
    }

    fn create_wall_texture(&mut self) {
        let (wall_texture, curb_texture) = textures::create_wall_texture(&self.device, &self.queue);
        self.wall_texture = Some(wall_texture);
        self.wall_curb_texture = Some(curb_texture);
    }

    fn create_smoke_texture(&mut self) {
        self.smoke_texture = Some(textures::create_smoke_texture(&self.device, &self.queue));
    }

    fn create_flame_texture(&mut self) {
        self.flame_texture = Some(textures::create_flame_texture(&self.device, &self.queue));
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
                targets: &[Some(create_color_target_state(surface_format))],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: create_primitive_state(Some(Face::Back)),
            depth_stencil: Some(create_depth_stencil_state(true)),
            multisample: create_multisample_state(),
            multiview: None,
        });

        self.pipeline = Some(pipeline);

        let additive_color_target = ColorTargetState {
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
        };

        let additive_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("MD3 Additive Shader"),
            source: ShaderSource::Wgsl(MD3_ADDITIVE_SHADER.into()),
        });

        let additive_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("MD3 Additive Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &additive_shader,
                entry_point: "vs_main",
                buffers: &[VertexData::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &additive_shader,
                entry_point: "fs_main",
                targets: &[Some(additive_color_target)],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: create_primitive_state(None),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: create_multisample_state(),
            multiview: None,
        });

        self.additive_pipeline = Some(additive_pipeline);

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
                targets: &[Some(create_color_target_state(surface_format))],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: create_primitive_state(None),
            depth_stencil: Some(create_depth_stencil_state(true)),
            multisample: create_multisample_state(),
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
                targets: &[Some(create_color_target_state(surface_format))],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: create_primitive_state(None),
            depth_stencil: Some(create_depth_stencil_state(true)),
            multisample: create_multisample_state(),
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
            primitive: create_primitive_state(None),
            depth_stencil: Some(shadow_depth_stencil),
            multisample: create_multisample_state(),
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
            primitive: create_primitive_state(None),
            depth_stencil: Some(wall_shadow_depth_stencil),
            multisample: create_multisample_state(),
            multiview: None,
        });

        self.wall_shadow_pipeline = Some(wall_shadow_pipeline);

        let tile_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Tile Shader"),
            source: ShaderSource::Wgsl(TILE_SHADER.into()),
        });

        let tile_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Tile Pipeline Layout"),
            bind_group_layouts: &[&self.tile_bind_group_layout],
            push_constant_ranges: &[],
        });

        let tile_pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Tile Pipeline"),
            layout: Some(&tile_pipeline_layout),
            vertex: VertexState {
                module: &tile_shader,
                entry_point: "vs_main",
                buffers: &[VertexData::desc()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &tile_shader,
                entry_point: "fs_main",
                targets: &[Some(create_color_target_state(surface_format))],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: create_primitive_state(None),
            depth_stencil: Some(create_depth_stencil_state(true)),
            multisample: create_multisample_state(),
            multiview: None,
        });

        self.tile_pipeline = Some(tile_pipeline);

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
        let wall_height = 500.0;
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

        if self.smoke_texture.is_none() {
            self.create_smoke_texture();
        }
        if self.flame_texture.is_none() {
            self.create_flame_texture();
        }

        let smoke_tex = self.smoke_texture.as_ref().unwrap();
        let flame_tex = self.flame_texture.as_ref().unwrap();

        self.particle_renderer = Some(ParticleRenderer::new(
            self.device.clone(),
            self.queue.clone(),
            &self.particle_bind_group_layout,
            smoke_tex,
            flame_tex,
            surface_format,
        ));

        self.init_shadow_pipelines(surface_format);
    }

    fn init_shadow_pipelines(&mut self, surface_format: TextureFormat) {
        use crate::render::shadows::ShadowVolumeVertex;

        let shadow_volume_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shadow Volume Shader"),
            source: ShaderSource::Wgsl(SHADOW_VOLUME_SHADER.into()),
        });

        let shadow_volume_bind_group_layout = create_shadow_volume_bind_group_layout(&self.device);

        let shadow_volume_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shadow Volume Pipeline Layout"),
            bind_group_layouts: &[&shadow_volume_bind_group_layout],
            push_constant_ranges: &[],
        });

        let shadow_volume_depth_stencil_front = DepthStencilState {
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
            multisample: create_multisample_state(),
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
            multisample: create_multisample_state(),
            multiview: None,
        });

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
            depth_compare: CompareFunction::LessEqual,
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
            multisample: create_multisample_state(),
            multiview: None,
        });

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

        let shadow_planar_shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shadow Planar Shader"),
            source: ShaderSource::Wgsl(SHADOW_PLANAR_SHADER.into()),
        });

        let shadow_planar_pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shadow Planar Pipeline Layout"),
            bind_group_layouts: &[&shadow_volume_bind_group_layout],
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
            multisample: create_multisample_state(),
            multiview: None,
        });

        let mut shadow_renderer = ShadowRenderer::new(
            self.device.clone(),
            shadow_volume_bind_group_layout,
        );
        shadow_renderer.set_volume_pipelines(shadow_volume_front_pipeline, shadow_volume_back_pipeline);
        shadow_renderer.set_apply_pipeline(shadow_apply_pipeline, shadow_apply_vertex_buffer);
        shadow_renderer.set_planar_pipeline(shadow_planar_pipeline);
        self.shadow_renderer = Some(shadow_renderer);
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
        let additive_pipeline = self.additive_pipeline.as_ref().unwrap();
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
        
        for mesh in &mesh_data {
            if mesh.is_additive {
                render_pass.set_pipeline(additive_pipeline);
            } else {
                render_pass.set_pipeline(pipeline);
            }
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
        if let Some(ref mut particle_renderer) = self.particle_renderer {
            particle_renderer.render_particles(encoder, output_view, depth_view, view_proj, camera_pos, particles);
        }
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
        if let Some(ref mut particle_renderer) = self.particle_renderer {
            particle_renderer.render_flames(encoder, output_view, depth_view, view_proj, camera_pos, flames);
        }
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
        if let Some(ref mut debug_renderer) = self.debug_renderer {
            debug_renderer.render_debug_lights(
                encoder,
                output_view,
                depth_view,
                view_proj,
                camera_pos,
                lights,
                surface_format,
                &self.debug_light_sphere_bind_group_layout,
            );
        }
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
        if let Some(ref mut debug_renderer) = self.debug_renderer {
            debug_renderer.render_debug_light_rays(
                encoder,
                output_view,
                depth_view,
                view_proj,
                lights,
                surface_format,
                &self.debug_light_ray_bind_group_layout,
            );
        }
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
        if let Some(ref mut shadow_renderer) = self.shadow_renderer {
            shadow_renderer.render_planar_shadows(encoder, output_view, depth_view, view_proj, models, lights);
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
        if let Some(ref mut shadow_renderer) = self.shadow_renderer {
            shadow_renderer.render_shadow_volumes(encoder, output_view, depth_view, view_proj, models, lights);
        }
    }

    fn init_coordinate_grid(&mut self, surface_format: TextureFormat) {
        if self.coordinate_grid_pipeline.is_some() {
            return;
        }

        let wall_size = 500.0;
        let wall_height = 50.0;
        let wall_z = -2.9;
        let wall_bottom = 0.0;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut index_offset = 0u16;

        let grid_color = [1.0, 1.0, 0.0, 1.0];
        let major_color = [1.0, 0.0, 0.0, 1.0];

        let step = 1.0;
        let major_step = 5.0;

        for x in (-wall_size as i32..=wall_size as i32).step_by(step as usize) {
            let x_f = x as f32;
            let is_major = (x as f32).abs() % major_step < 0.1;
            let color = if is_major { major_color } else { grid_color };

            vertices.push(VertexData {
                position: [x_f, wall_bottom, wall_z],
                uv: [0.0, 0.0],
                color,
                normal: [0.0, 0.0, 1.0],
            });
            vertices.push(VertexData {
                position: [x_f, wall_height, wall_z],
                uv: [0.0, 1.0],
                color,
                normal: [0.0, 0.0, 1.0],
            });
            indices.push(index_offset);
            indices.push(index_offset + 1);
            index_offset += 2;
        }

        for y in (wall_bottom as i32..=wall_height as i32).step_by(step as usize) {
            let y_f = y as f32;
            let is_major = (y as f32) % major_step < 0.1;
            let color = if is_major { major_color } else { grid_color };

            vertices.push(VertexData {
                position: [-wall_size, y_f, wall_z],
                uv: [0.0, 0.0],
                color,
                normal: [0.0, 0.0, 1.0],
            });
            vertices.push(VertexData {
                position: [wall_size, y_f, wall_z],
                uv: [1.0, 0.0],
                color,
                normal: [0.0, 0.0, 1.0],
            });
            indices.push(index_offset);
            indices.push(index_offset + 1);
            index_offset += 2;
        }

        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Coordinate Grid Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Coordinate Grid Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: BufferUsages::INDEX,
        });

        self.coordinate_grid_vertex_buffer = Some(vertex_buffer);
        self.coordinate_grid_index_buffer = Some(index_buffer);

        #[repr(C)]
        #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
        struct CoordinateGridUniforms {
            view_proj: [[f32; 4]; 4],
            model: [[f32; 4]; 4],
        }

        let uniform_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Coordinate Grid Uniform Buffer"),
            size: std::mem::size_of::<CoordinateGridUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Coordinate Grid Bind Group"),
            layout: &self.coordinate_grid_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        self.coordinate_grid_uniform_buffer = Some(uniform_buffer);
        self.coordinate_grid_bind_group = Some(bind_group);

        let shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Coordinate Grid Shader"),
            source: ShaderSource::Wgsl(COORDINATE_GRID_SHADER.into()),
        });

        let pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Coordinate Grid Pipeline Layout"),
            bind_group_layouts: &[&self.coordinate_grid_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Coordinate Grid Pipeline"),
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
                targets: &[Some(create_color_target_state(surface_format))],
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
                depth_write_enabled: false,
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: create_multisample_state(),
            multiview: None,
        });

        self.coordinate_grid_pipeline = Some(pipeline);
    }

    pub fn render_coordinate_grid(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        surface_format: TextureFormat,
    ) {
        self.init_coordinate_grid(surface_format);

        #[repr(C)]
        #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
        struct CoordinateGridUniforms {
            view_proj: [[f32; 4]; 4],
            model: [[f32; 4]; 4],
        }

        let uniforms = CoordinateGridUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            model: Mat4::IDENTITY.to_cols_array_2d(),
        };

        if let Some(ref uniform_buffer) = self.coordinate_grid_uniform_buffer {
            self.queue.write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        }

        let pipeline = self.coordinate_grid_pipeline.as_ref().unwrap();
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Coordinate Grid Render Pass"),
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
        render_pass.set_bind_group(0, self.coordinate_grid_bind_group.as_ref().unwrap(), &[]);
        render_pass.set_vertex_buffer(0, self.coordinate_grid_vertex_buffer.as_ref().unwrap().slice(..));
        
        if let Some(ref index_buffer) = self.coordinate_grid_index_buffer {
            render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint16);
            let num_indices = index_buffer.size() as u32 / std::mem::size_of::<u16>() as u32;
            render_pass.draw_indexed(0..num_indices, 0, 0..1);
        }
    }

    pub fn load_map_tiles(&mut self, map: &crate::game::map::Map) {
        use crate::render::map_meshes::TileMeshes;
        use crate::render::textures_tile::create_tile_texture;

        let tile_meshes = TileMeshes::generate_from_map(map);

        if tile_meshes.vertices.is_empty() {
            return;
        }

        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tile Vertex Buffer"),
            contents: bytemuck::cast_slice(&tile_meshes.vertices),
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tile Index Buffer"),
            contents: bytemuck::cast_slice(&tile_meshes.indices),
            usage: BufferUsages::INDEX,
        });

        self.tile_vertex_buffer = Some(vertex_buffer);
        self.tile_index_buffer = Some(index_buffer);
        self.tile_num_indices = tile_meshes.indices.len() as u32;

        if self.tile_texture.is_none() {
            self.tile_texture = Some(create_tile_texture(&self.device, &self.queue));
        }

        println!("Loaded map tiles: {} vertices, {} indices", tile_meshes.vertices.len(), tile_meshes.indices.len());
    }

    pub fn render_tiles(
        &mut self,
        encoder: &mut CommandEncoder,
        output_view: &TextureView,
        depth_view: &TextureView,
        view_proj: Mat4,
        camera_pos: Vec3,
        lights: &[(Vec3, Vec3, f32)],
        ambient_light: f32,
        surface_format: TextureFormat,
    ) {
        if self.tile_pipeline.is_none() {
            self.create_pipeline(surface_format);
        }

        if self.tile_vertex_buffer.is_none() || self.tile_index_buffer.is_none() {
            return;
        }

        let uniforms = self.create_uniforms(
            view_proj,
            Mat4::IDENTITY,
            camera_pos,
            lights,
            ambient_light,
        );

        let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tile Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: BufferUsages::UNIFORM,
        });

        let tile_texture = self.tile_texture.as_ref().unwrap();
        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Tile Bind Group"),
            layout: &self.tile_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&tile_texture.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&tile_texture.sampler),
                },
            ],
        });

        let pipeline = self.tile_pipeline.as_ref().unwrap();
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Tile Render Pass"),
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
        render_pass.set_vertex_buffer(0, self.tile_vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.set_index_buffer(self.tile_index_buffer.as_ref().unwrap().slice(..), IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.tile_num_indices, 0, 0..1);
    }
}

