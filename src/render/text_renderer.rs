use wgpu::util::DeviceExt;
use std::sync::Arc;
use std::collections::HashMap;
use fontdue::{Font, FontSettings};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

struct GlyphInfo {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    advance: f32,
    offset_x: f32,
    offset_y: f32,
}

pub struct TextRenderer {
    pipeline: wgpu::RenderPipeline,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    glyph_info: HashMap<char, GlyphInfo>,
    atlas_width: u32,
    atlas_height: u32,
}

impl TextRenderer {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, surface_format: wgpu::TextureFormat) -> Self {
        let font_data = include_bytes!("../../assets/fonts/RobotoMono.ttf");
        let font = Font::from_bytes(font_data as &[u8], FontSettings::default()).unwrap();
        
        let font_size = 48.0;
        let chars: Vec<char> = (32..127).map(|c| c as u8 as char).collect();
        
        let mut glyph_info = HashMap::new();
        let mut atlas_data: Vec<u8> = Vec::new();
        let atlas_width = 512u32;
        let atlas_height = 512u32;
        atlas_data.resize((atlas_width * atlas_height) as usize, 0);
        
        let mut cursor_x = 0u32;
        let mut cursor_y = 0u32;
        let mut row_height = 0u32;
        
        for ch in chars {
            let (metrics, bitmap) = font.rasterize(ch, font_size);
            
            if cursor_x + metrics.width as u32 > atlas_width {
                cursor_x = 0;
                cursor_y += row_height + 2;
                row_height = 0;
            }
            
            if cursor_y + metrics.height as u32 > atlas_height {
                break;
            }
            
            for y in 0..metrics.height {
                for x in 0..metrics.width {
                    let atlas_x = cursor_x + x as u32;
                    let atlas_y = cursor_y + y as u32;
                    let idx = (atlas_y * atlas_width + atlas_x) as usize;
                    atlas_data[idx] = bitmap[y * metrics.width + x];
                }
            }
            
            glyph_info.insert(ch, GlyphInfo {
                x: cursor_x,
                y: cursor_y,
                width: metrics.width as u32,
                height: metrics.height as u32,
                advance: metrics.advance_width,
                offset_x: metrics.xmin as f32,
                offset_y: metrics.ymin as f32,
            });
            
            row_height = row_height.max(metrics.height as u32);
            cursor_x += metrics.width as u32 + 2;
        }
        
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Text Atlas Texture"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(atlas_width),
                rows_per_image: Some(atlas_height),
            },
            wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
        );
        
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/text.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Self {
            pipeline,
            device,
            queue,
            texture,
            texture_view,
            sampler,
            bind_group,
            glyph_info,
            atlas_width,
            atlas_height,
        }
    }

    pub fn render_text(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
        width: u32,
        height: u32,
    ) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        
        let scale = size / 48.0;
        let mut cursor_x = x;
        
        for ch in text.chars() {
            if let Some(glyph) = self.glyph_info.get(&ch) {
                let x0 = cursor_x + glyph.offset_x * scale;
                let y0 = y + glyph.offset_y * scale;
                let x1 = x0 + glyph.width as f32 * scale;
                let y1 = y0 + glyph.height as f32 * scale;
                
                let screen_x0 = (x0 / width as f32) * 2.0 - 1.0;
                let screen_y0 = 1.0 - (y0 / height as f32) * 2.0;
                let screen_x1 = (x1 / width as f32) * 2.0 - 1.0;
                let screen_y1 = 1.0 - (y1 / height as f32) * 2.0;
                
                let u0 = glyph.x as f32 / self.atlas_width as f32;
                let v0 = glyph.y as f32 / self.atlas_height as f32;
                let u1 = (glyph.x + glyph.width) as f32 / self.atlas_width as f32;
                let v1 = (glyph.y + glyph.height) as f32 / self.atlas_height as f32;
                
                let base = vertices.len() as u16;
                vertices.push(Vertex { position: [screen_x0, screen_y0], tex_coords: [u0, v0] });
                vertices.push(Vertex { position: [screen_x1, screen_y0], tex_coords: [u1, v0] });
                vertices.push(Vertex { position: [screen_x1, screen_y1], tex_coords: [u1, v1] });
                vertices.push(Vertex { position: [screen_x0, screen_y1], tex_coords: [u0, v1] });
                
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                
                cursor_x += glyph.advance * scale;
            } else {
                cursor_x += size * 0.5;
            }
        }
        
        if vertices.is_empty() {
            println!("TextRenderer: vertices is empty, returning");
            return;
        }
        
        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        
        let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Text Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
    }
}
