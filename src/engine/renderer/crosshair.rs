use wgpu::*;

pub struct Crosshair {
    pipeline: RenderPipeline,
    vertex_buffer: Buffer,
}

impl Crosshair {
    pub fn new(device: &Device, format: TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Crosshair Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/crosshair.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Crosshair Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Crosshair Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[VertexBufferLayout {
                    array_stride: 8,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &vertex_attr_array![0 => Float32x2],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
        });

        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Crosshair Vertex Buffer"),
            size: 8 * 4,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
        }
    }

    pub fn render(
        &self,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        queue: &Queue,
        screen_x: f32,
        screen_y: f32,
        width: u32,
        height: u32,
    ) {
        let ndc_x = (screen_x / width as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_y / height as f32) * 2.0;

        let size = 0.02;

        let vertices: [f32; 8] = [
            ndc_x - size, ndc_y,
            ndc_x + size, ndc_y,
            ndc_x, ndc_y - size,
            ndc_x, ndc_y + size,
        ];

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Crosshair Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..4, 0..1);
    }
}
