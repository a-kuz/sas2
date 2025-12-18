use std::sync::Arc;
use wgpu::*;
use bytemuck::{Pod, Zeroable};

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

pub const MAX_LIGHTS: usize = 8;

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

pub struct MeshRenderData {
    pub vertex_buffer: Arc<Buffer>,
    pub index_buffer: Arc<Buffer>,
    pub num_indices: u32,
    pub bind_group: BindGroup,
    pub shadow_bind_group: Option<BindGroup>,
    pub uniform_buffer: Arc<Buffer>,
    pub shadow_uniform_buffer: Option<Arc<Buffer>>,
    pub is_additive: bool,
}
