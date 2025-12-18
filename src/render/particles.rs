use std::sync::Arc;
use wgpu::*;
use wgpu::util::DeviceExt;
use glam::{Mat4, Vec3};
use bytemuck::{Pod, Zeroable};
use crate::render::types::{VertexData, WgpuTexture};
use crate::engine::shaders::{PARTICLE_SHADER, FLAME_SHADER};
use super::pipelines::*;

pub struct ParticleRenderer {
    queue: Arc<Queue>,
    particle_pipeline: Option<RenderPipeline>,
    flame_pipeline: Option<RenderPipeline>,
    particle_quad_vertex_buffer: Option<Buffer>,
    particle_quad_index_buffer: Option<Buffer>,
    particle_instance_buffer: Option<Buffer>,
    flame_instance_buffer: Option<Buffer>,
    particle_uniform_buffer: Option<Buffer>,
    flame_uniform_buffer: Option<Buffer>,
    particle_bind_group: Option<BindGroup>,
    flame_bind_group: Option<BindGroup>,
}

impl ParticleRenderer {
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        particle_bind_group_layout: &BindGroupLayout,
        smoke_texture: &WgpuTexture,
        flame_texture: &WgpuTexture,
        surface_format: TextureFormat,
    ) -> Self {
        let particle_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Particle Shader"),
            source: ShaderSource::Wgsl(PARTICLE_SHADER.into()),
        });

        let particle_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[particle_bind_group_layout],
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

        let particle_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
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
            primitive: create_primitive_state(None),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: create_multisample_state(),
            multiview: None,
        });

        let flame_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Flame Shader"),
            source: ShaderSource::Wgsl(FLAME_SHADER.into()),
        });

        let flame_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Flame Pipeline Layout"),
            bind_group_layouts: &[particle_bind_group_layout],
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

        let flame_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
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
            primitive: create_primitive_state(None),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: create_multisample_state(),
            multiview: None,
        });

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

        let particle_quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(&quad_vertices),
            usage: BufferUsages::VERTEX,
        });

        let particle_quad_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Quad Index Buffer"),
            contents: bytemuck::cast_slice(&quad_indices),
            usage: BufferUsages::INDEX,
        });

        let max_particles = 1000;
        #[repr(C)]
        #[derive(Copy, Clone, Pod, Zeroable)]
        struct ParticleInstance {
            position_size: [f32; 4],
            alpha: f32,
            _padding: [f32; 3],
        }

        let particle_instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Particle Instance Buffer"),
            size: (std::mem::size_of::<ParticleInstance>() * max_particles) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        #[repr(C)]
        #[derive(Copy, Clone, Pod, Zeroable)]
        struct FlameInstanceData {
            position_size: [f32; 4],
        }

        let flame_instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Flame Instance Buffer"),
            size: (std::mem::size_of::<FlameInstanceData>() * max_particles) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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

        let particle_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Particle Uniform Buffer"),
            size: max_size,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let flame_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Flame Uniform Buffer"),
            size: max_size,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let particle_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Particle Bind Group"),
            layout: particle_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: particle_uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&smoke_texture.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&smoke_texture.sampler),
                },
            ],
        });

        let flame_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Flame Bind Group"),
            layout: particle_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: flame_uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&flame_texture.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&flame_texture.sampler),
                },
            ],
        });

        Self {
            queue,
            particle_pipeline: Some(particle_pipeline),
            flame_pipeline: Some(flame_pipeline),
            particle_quad_vertex_buffer: Some(particle_quad_vertex_buffer),
            particle_quad_index_buffer: Some(particle_quad_index_buffer),
            particle_instance_buffer: Some(particle_instance_buffer),
            flame_instance_buffer: Some(flame_instance_buffer),
            particle_uniform_buffer: Some(particle_uniform_buffer),
            flame_uniform_buffer: Some(flame_uniform_buffer),
            particle_bind_group: Some(particle_bind_group),
            flame_bind_group: Some(flame_bind_group),
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

        #[repr(C)]
        #[derive(Copy, Clone, Pod, Zeroable)]
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
        #[derive(Copy, Clone, Pod, Zeroable)]
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

        #[repr(C)]
        #[derive(Copy, Clone, Pod, Zeroable)]
        struct FlameUniforms {
            view_proj: [[f32; 4]; 4],
            camera_pos: [f32; 4],
        }

        #[repr(C)]
        #[derive(Copy, Clone, Pod, Zeroable)]
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
}

