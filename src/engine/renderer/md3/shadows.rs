use std::collections::HashMap;
use std::sync::Arc;
use wgpu::*;
use wgpu::util::DeviceExt;
use glam::{Mat4, Vec3};
use bytemuck::{Pod, Zeroable};
use crate::engine::md3::MD3Model;
use crate::engine::renderer::shadows::*;

pub struct ShadowRenderer {
    device: Arc<Device>,
    shadow_volume_front_pipeline: Option<RenderPipeline>,
    shadow_volume_back_pipeline: Option<RenderPipeline>,
    shadow_volume_bind_group_layout: BindGroupLayout,
    shadow_apply_pipeline: Option<RenderPipeline>,
    shadow_apply_vertex_buffer: Option<Buffer>,
    shadow_planar_pipeline: Option<RenderPipeline>,
    silhouette_cache: HashMap<(usize, usize), ModelSilhouetteCache>,
}

impl ShadowRenderer {
    pub fn new(device: Arc<Device>, shadow_volume_bind_group_layout: BindGroupLayout) -> Self {
        Self {
            device,
            shadow_volume_front_pipeline: None,
            shadow_volume_back_pipeline: None,
            shadow_volume_bind_group_layout,
            shadow_apply_pipeline: None,
            shadow_apply_vertex_buffer: None,
            shadow_planar_pipeline: None,
            silhouette_cache: HashMap::new(),
        }
    }

    pub fn clear_cache(&mut self) {
        self.silhouette_cache.clear();
    }

    pub fn set_volume_pipelines(&mut self, front: RenderPipeline, back: RenderPipeline) {
        self.shadow_volume_front_pipeline = Some(front);
        self.shadow_volume_back_pipeline = Some(back);
    }

    pub fn set_apply_pipeline(&mut self, pipeline: RenderPipeline, vertex_buffer: Buffer) {
        self.shadow_apply_pipeline = Some(pipeline);
        self.shadow_apply_vertex_buffer = Some(vertex_buffer);
    }

    pub fn set_planar_pipeline(&mut self, pipeline: RenderPipeline) {
        self.shadow_planar_pipeline = Some(pipeline);
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
        eps: f32,
    ) -> Vec<[f32; 3]> {
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

            let ground_proj = Self::project_triangles_to_plane(&triangles, *light_pos, Vec3::new(0.0, 1.0, 0.0), 0.0, 0.002);
            let wall_proj = Self::project_triangles_to_plane(&triangles, *light_pos, Vec3::new(0.0, 0.0, 1.0), 3.0, 0.01);

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

