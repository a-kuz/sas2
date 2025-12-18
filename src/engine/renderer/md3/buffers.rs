use std::collections::HashMap;
use std::sync::Arc;
use wgpu::*;
use wgpu::util::DeviceExt;
use glam::{Mat4, Vec3};
use crate::engine::md3::MD3Model;
use crate::engine::renderer::types::*;

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct BufferCacheKey {
    pub model_id: usize,
    pub mesh_idx: usize,
    pub frame_idx: usize,
}

pub struct CachedBuffers {
    pub vertex_buffer: Arc<Buffer>,
    pub index_buffer: Arc<Buffer>,
    pub num_indices: u32,
}

pub fn get_or_create_buffers(
    buffer_cache: &mut HashMap<BufferCacheKey, CachedBuffers>,
    device: &Device,
    model: &MD3Model,
    mesh_idx: usize,
    frame_idx: usize,
) -> Option<(Arc<Buffer>, Arc<Buffer>, u32)> {
    let model_id = std::ptr::addr_of!(*model) as usize;
    let key = BufferCacheKey {
        model_id,
        mesh_idx,
        frame_idx,
    };
    
    if let Some(cached) = buffer_cache.get(&key) {
        return Some((cached.vertex_buffer.clone(), cached.index_buffer.clone(), cached.num_indices));
    }
    
    let (vertex_buffer, index_buffer, num_indices) = create_buffers_internal(device, model, mesh_idx, frame_idx)?;
    let cached = CachedBuffers {
        vertex_buffer: Arc::new(vertex_buffer),
        index_buffer: Arc::new(index_buffer),
        num_indices,
    };
    let result = (cached.vertex_buffer.clone(), cached.index_buffer.clone(), cached.num_indices);
    buffer_cache.insert(key, cached);
    Some(result)
}

pub fn create_buffers_internal(
    device: &Device,
    model: &MD3Model,
    mesh_idx: usize,
    frame_idx: usize,
) -> Option<(Buffer, Buffer, u32)> {
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
            color: [1.0, 1.0, 1.0, 1.0],
            normal: [nx, ny, nz],
        });
    }

    for triangle in &mesh.triangles {
        indices.push(triangle.vertex[0] as u16);
        indices.push(triangle.vertex[1] as u16);
        indices.push(triangle.vertex[2] as u16);
    }
    
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("MD3 Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: BufferUsages::VERTEX,
    });
    
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("MD3 Index Buffer"),
        contents: bytemuck::cast_slice(&indices),
        usage: BufferUsages::INDEX,
    });
    
    let num_indices = indices.len() as u32;
    
    Some((vertex_buffer, index_buffer, num_indices))
}

pub fn create_uniforms(
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

pub fn update_uniform_buffer(queue: &Queue, uniforms: &MD3Uniforms, buffer: &Buffer) {
    queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[*uniforms]));
}

pub fn find_texture<'a>(
    model_textures: &'a HashMap<String, WgpuTexture>,
    path: &str,
) -> Option<&'a WgpuTexture> {
    let mut alt_paths = vec![
        path.to_string(),
        format!("../{}", path),
        path.replace("../", ""),
    ];
    
    if path.ends_with(".TGA") {
        let png_path = path.replace(".TGA", ".png");
        alt_paths.push(png_path.clone());
        alt_paths.push(format!("../{}", png_path));
        alt_paths.push(path.replace(".TGA", ".jpg"));
        alt_paths.push(format!("../{}", path.replace(".TGA", ".jpg")));
    } else if path.ends_with(".tga") {
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
        if let Some(tex) = model_textures.get(alt_path) {
            return Some(tex);
        }
    }
    
    println!("Warning: texture not found in HashMap for path: {:?}", path);
    println!("Tried paths: {:?}", alt_paths);
    println!("Available texture keys: {:?}", model_textures.keys().collect::<Vec<_>>());
    None
}

pub fn create_mesh_bind_groups(
    device: &Device,
    bind_group_layout: &BindGroupLayout,
    texture: &WgpuTexture,
    uniform_buffer: &Buffer,
    shadow_uniform_buffer: Option<&Buffer>,
    render_shadow: bool,
) -> (BindGroup, Option<BindGroup>) {
    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("MD3 Bind Group"),
        layout: bind_group_layout,
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
        Some(device.create_bind_group(&BindGroupDescriptor {
            label: Some("Shadow Bind Group"),
            layout: bind_group_layout,
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

pub fn prepare_mesh_data(
    buffer_cache: &mut HashMap<BufferCacheKey, CachedBuffers>,
    device: &Device,
    bind_group_layout: &BindGroupLayout,
    model_textures: &HashMap<String, WgpuTexture>,
    model: &MD3Model,
    frame_idx: usize,
    texture_paths: &[Option<String>],
    uniform_buffer: Arc<Buffer>,
    shadow_uniform_buffer: Option<Arc<Buffer>>,
    render_shadow: bool,
) -> Vec<MeshRenderData> {
    let mut buffers_vec = Vec::new();
    
    for (mesh_idx, _mesh) in model.meshes.iter().enumerate() {
        let (vertex_buffer, index_buffer, num_indices) = match get_or_create_buffers(
            buffer_cache,
            device,
            model,
            mesh_idx,
            frame_idx,
        ) {
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
        let texture = texture_path.as_ref().and_then(|path| find_texture(model_textures, path));
        if let Some(texture) = texture {
            let (bind_group, shadow_bind_group) = create_mesh_bind_groups(
                device,
                bind_group_layout,
                texture,
                &uniform_buffer,
                shadow_uniform_buffer.as_ref().map(|b| b.as_ref()),
                render_shadow,
            );

            let is_additive = texture_path.as_ref()
                .map(|path| path.ends_with(".TGA"))
                .unwrap_or(false);

            mesh_data.push(MeshRenderData {
                vertex_buffer,
                index_buffer,
                num_indices,
                bind_group,
                shadow_bind_group,
                uniform_buffer: uniform_buffer.clone(),
                shadow_uniform_buffer: shadow_uniform_buffer.clone(),
                is_additive,
            });
        }
    }

    mesh_data
}

