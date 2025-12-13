use std::sync::Arc;
use wgpu::*;
use wgpu::util::DeviceExt;
use glam::{Mat4, Vec3};
use bytemuck::{Pod, Zeroable};
use crate::engine::renderer::types::VertexData;
use crate::engine::shaders::{DEBUG_LIGHT_SPHERE_SHADER, DEBUG_LIGHT_RAY_SHADER};
use super::pipelines::*;

pub struct DebugRenderer {
    device: Arc<Device>,
    queue: Arc<Queue>,
    debug_light_sphere_pipeline: Option<RenderPipeline>,
    debug_light_ray_pipeline: Option<RenderPipeline>,
    debug_light_sphere_uniform_buffer: Option<Buffer>,
    debug_light_sphere_bind_group: Option<BindGroup>,
    debug_light_ray_uniform_buffer: Option<Buffer>,
    debug_light_ray_bind_group: Option<BindGroup>,
    debug_sphere_vertex_buffer: Option<Buffer>,
    debug_sphere_index_buffer: Option<Buffer>,
    debug_sphere_instance_buffer: Option<Buffer>,
    debug_ray_vertex_buffer: Option<Buffer>,
}

impl DebugRenderer {
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        _debug_light_sphere_bind_group_layout: &BindGroupLayout,
        _debug_light_ray_bind_group_layout: &BindGroupLayout,
    ) -> Self {
        Self {
            device,
            queue,
            debug_light_sphere_pipeline: None,
            debug_light_ray_pipeline: None,
            debug_light_sphere_uniform_buffer: None,
            debug_light_sphere_bind_group: None,
            debug_light_ray_uniform_buffer: None,
            debug_light_ray_bind_group: None,
            debug_sphere_vertex_buffer: None,
            debug_sphere_index_buffer: None,
            debug_sphere_instance_buffer: None,
            debug_ray_vertex_buffer: None,
        }
    }

    fn init_debug_light_sphere(&mut self, surface_format: TextureFormat, debug_light_sphere_bind_group_layout: &BindGroupLayout) {
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
            layout: debug_light_sphere_bind_group_layout,
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
            bind_group_layouts: &[debug_light_sphere_bind_group_layout],
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
                targets: &[Some(create_color_target_state(surface_format))],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: create_primitive_state(Some(Face::Back)),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: true,
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: create_multisample_state(),
            multiview: None,
        });

        self.debug_light_sphere_pipeline = Some(pipeline);
    }

    fn init_debug_light_ray(&mut self, surface_format: TextureFormat, debug_light_ray_bind_group_layout: &BindGroupLayout) {
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
            layout: debug_light_ray_bind_group_layout,
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
            bind_group_layouts: &[debug_light_ray_bind_group_layout],
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
                depth_write_enabled: true,
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: create_multisample_state(),
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
        debug_light_sphere_bind_group_layout: &BindGroupLayout,
    ) {
        if lights.is_empty() {
            return;
        }

        self.init_debug_light_sphere(surface_format, debug_light_sphere_bind_group_layout);

        #[repr(C)]
        #[derive(Copy, Clone, Pod, Zeroable)]
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
        #[derive(Copy, Clone, Pod, Zeroable)]
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
        debug_light_ray_bind_group_layout: &BindGroupLayout,
    ) {
        if lights.is_empty() {
            return;
        }

        self.init_debug_light_ray(surface_format, debug_light_ray_bind_group_layout);

        #[repr(C)]
        #[derive(Copy, Clone, Pod, Zeroable)]
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
        #[derive(Copy, Clone, Pod, Zeroable)]
        struct RayVertex {
            position: [f32; 3],
            color: [f32; 4],
        }

        let mut vertices = Vec::new();
        
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
}

