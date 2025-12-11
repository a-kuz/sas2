use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MD3Header {
    pub id: [u8; 4],
    pub version: i32,
    pub filename: [u8; 64],
    pub flags: i32,
    pub num_bone_frames: i32,
    pub num_tags: i32,
    pub num_meshes: i32,
    pub num_max_skins: i32,
    pub header_length: i32,
    pub tag_start: i32,
    pub tag_end: i32,
    pub file_size: i32,
}

#[derive(Debug, Clone)]
pub struct Tag {
    pub name: [u8; 64],
    pub position: [f32; 3],
    pub axis: [[f32; 3]; 3],
}

#[derive(Debug, Clone)]
pub struct Triangle {
    pub vertex: [i32; 3],
}

#[derive(Debug, Clone)]
pub struct TexCoord {
    pub coord: [f32; 2],
}

#[derive(Debug, Clone)]
pub struct Vertex {
    pub vertex: [i16; 3],
    pub normal: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct MeshHeader {
    pub id: [u8; 4],
    pub name: [u8; 64],
    pub flags: i32,
    pub num_mesh_frames: i32,
    pub num_shaders: i32,
    pub num_vertices: i32,
    pub num_triangles: i32,
    pub tri_start: i32,
    pub shaders_start: i32,
    pub tex_vector_start: i32,
    pub vertex_start: i32,
    pub mesh_size: i32,
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub header: MeshHeader,
    pub triangles: Vec<Triangle>,
    pub tex_coords: Vec<TexCoord>,
    pub vertices: Vec<Vec<Vertex>>,
}

#[derive(Debug, Clone)]
pub struct MD3Model {
    pub header: MD3Header,
    pub tags: Vec<Vec<Tag>>,
    pub meshes: Vec<Mesh>,
}

trait CopyFromSlice {
    fn copy_from_slice(&mut self, src: &[u8]);
}

impl CopyFromSlice for [u8] {
    fn copy_from_slice(&mut self, src: &[u8]) {
        let len = self.len().min(src.len());
        self[..len].copy_from_slice(&src[..len]);
    }
}

// Для массивов фиксированного размера
trait CopySlice {
    fn copy_from_slice(&mut self, src: &[u8]);
}

impl<const N: usize> CopySlice for [u8; N] {
    fn copy_from_slice(&mut self, src: &[u8]) {
        let len = N.min(src.len());
        self[..len].copy_from_slice(&src[..len]);
    }
}

impl MD3Model {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;

        let mut header_bytes = [0u8; 108];
        file.read_exact(&mut header_bytes)
            .map_err(|e| format!("Failed to read header: {}", e))?;

        let header: MD3Header = unsafe { std::ptr::read(header_bytes.as_ptr() as *const _) };

        if &header.id != b"IDP3" {
            return Err("Invalid MD3 file format".to_string());
        }

        for _ in 0..header.num_bone_frames {
            let mut frame_bytes = [0u8; 56];
            file.read_exact(&mut frame_bytes)
                .map_err(|e| format!("Failed to read bone frame: {}", e))?;
        }

        let mut tags = vec![Vec::new(); header.num_bone_frames as usize];
        for frame_idx in 0..header.num_bone_frames as usize {
            for _ in 0..header.num_tags {
                let mut tag_bytes = [0u8; 112];
                file.read_exact(&mut tag_bytes)
                    .map_err(|e| format!("Failed to read tag: {}", e))?;

                let mut name = [0u8; 64];
                name.copy_from_slice(&tag_bytes[0..64]);

                let mut position = [0f32; 3];
                for i in 0..3 {
                    let start = 64 + i * 4;
                    position[i] = f32::from_le_bytes([
                        tag_bytes[start],
                        tag_bytes[start + 1],
                        tag_bytes[start + 2],
                        tag_bytes[start + 3],
                    ]);
                }

                let mut axis = [[0f32; 3]; 3];
                for row in 0..3 {
                    for col in 0..3 {
                        let start = 76 + (row * 3 + col) * 4;
                        axis[row][col] = f32::from_le_bytes([
                            tag_bytes[start],
                            tag_bytes[start + 1],
                            tag_bytes[start + 2],
                            tag_bytes[start + 3],
                        ]);
                    }
                }

                tags[frame_idx].push(Tag {
                    name,
                    position,
                    axis,
                });
            }
        }

        let mut meshes = Vec::with_capacity(header.num_meshes as usize);
        for _ in 0..header.num_meshes {
            let mesh_start =
                file.stream_position()
                    .map_err(|e| format!("Failed to get position: {}", e))? as i64;

            let mut mesh_header_bytes = [0u8; 108];
            file.read_exact(&mut mesh_header_bytes)
                .map_err(|e| format!("Failed to read mesh header: {}", e))?;

            let mut id = [0u8; 4];
            id.copy_from_slice(&mesh_header_bytes[0..4]);
            let mut name = [0u8; 64];
            name.copy_from_slice(&mesh_header_bytes[4..68]);
            let flags = i32::from_le_bytes(mesh_header_bytes[68..72].try_into().unwrap());
            let num_mesh_frames = i32::from_le_bytes(mesh_header_bytes[72..76].try_into().unwrap());
            let num_shaders = i32::from_le_bytes(mesh_header_bytes[76..80].try_into().unwrap());
            let num_vertices = i32::from_le_bytes(mesh_header_bytes[80..84].try_into().unwrap());
            let num_triangles = i32::from_le_bytes(mesh_header_bytes[84..88].try_into().unwrap());
            let tri_start = i32::from_le_bytes(mesh_header_bytes[88..92].try_into().unwrap());
            let shaders_start = i32::from_le_bytes(mesh_header_bytes[92..96].try_into().unwrap());
            let tex_vector_start = i32::from_le_bytes(mesh_header_bytes[96..100].try_into().unwrap());
            let vertex_start = i32::from_le_bytes(mesh_header_bytes[100..104].try_into().unwrap());
            let mesh_size = i32::from_le_bytes(mesh_header_bytes[104..108].try_into().unwrap());

            let mesh_header = MeshHeader {
                id,
                name,
                flags,
                num_mesh_frames,
                num_shaders,
                num_vertices,
                num_triangles,
                tri_start,
                shaders_start,
                tex_vector_start,
                vertex_start,
                mesh_size,
            };

            file.seek(SeekFrom::Start(
                (mesh_start + mesh_header.tri_start as i64) as u64,
            ))
            .map_err(|e| format!("Failed to seek: {}", e))?;

            let mut triangles = Vec::with_capacity(mesh_header.num_triangles as usize);
            for _ in 0..mesh_header.num_triangles {
                let mut tri_bytes = [0u8; 12];
                file.read_exact(&mut tri_bytes)
                    .map_err(|e| format!("Failed to read triangle: {}", e))?;
                let tri = unsafe { std::ptr::read(tri_bytes.as_ptr() as *const Triangle) };
                triangles.push(tri);
            }

            file.seek(SeekFrom::Start(
                (mesh_start + mesh_header.tex_vector_start as i64) as u64,
            ))
            .map_err(|e| format!("Failed to seek: {}", e))?;

            let mut tex_coords = Vec::with_capacity(mesh_header.num_vertices as usize);
            for _ in 0..mesh_header.num_vertices {
                let mut tc_bytes = [0u8; 8];
                file.read_exact(&mut tc_bytes)
                    .map_err(|e| format!("Failed to read tex coord: {}", e))?;
                let tc = unsafe { std::ptr::read(tc_bytes.as_ptr() as *const TexCoord) };
                tex_coords.push(tc);
            }

            file.seek(SeekFrom::Start(
                (mesh_start + mesh_header.vertex_start as i64) as u64,
            ))
            .map_err(|e| format!("Failed to seek: {}", e))?;

            let mut vertices = Vec::with_capacity(mesh_header.num_mesh_frames as usize);
            for _ in 0..mesh_header.num_mesh_frames {
                let mut frame_verts = Vec::with_capacity(mesh_header.num_vertices as usize);
                for _ in 0..mesh_header.num_vertices {
                    let mut vert_bytes = [0u8; 8];
                    file.read_exact(&mut vert_bytes)
                        .map_err(|e| format!("Failed to read vertex: {}", e))?;
                    let vertex = [
                        i16::from_le_bytes([vert_bytes[0], vert_bytes[1]]),
                        i16::from_le_bytes([vert_bytes[2], vert_bytes[3]]),
                        i16::from_le_bytes([vert_bytes[4], vert_bytes[5]]),
                    ];
                    let normal = u16::from_le_bytes([vert_bytes[6], vert_bytes[7]]);
                    frame_verts.push(Vertex { vertex, normal });
                }
                vertices.push(frame_verts);
            }

            meshes.push(Mesh {
                header: mesh_header,
                triangles,
                tex_coords,
                vertices,
            });

            file.seek(SeekFrom::Start(
                (mesh_start + mesh_header.mesh_size as i64) as u64,
            ))
            .map_err(|e| format!("Failed to seek: {}", e))?;
        }

        Ok(MD3Model {
            header,
            tags,
            meshes,
        })
    }
}
