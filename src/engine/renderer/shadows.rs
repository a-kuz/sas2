use wgpu::*;
use glam::Vec3;
use bytemuck::{Pod, Zeroable};

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

pub struct Edge {
    pub v0: usize,
    pub v1: usize,
}

pub struct SilhouetteEdge {
    pub v0: Vec3,
    pub v1: Vec3,
}

pub struct ModelSilhouetteCache {
    pub edges: Vec<Edge>,
    pub triangle_neighbors: Vec<[Option<usize>; 3]>,
}
