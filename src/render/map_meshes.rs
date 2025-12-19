use crate::game::map::Map;
use crate::render::types::VertexData;

pub struct TileMeshes {
    pub vertices: Vec<VertexData>,
    pub indices: Vec<u16>,
}

impl TileMeshes {
    pub fn generate_from_map(map: &Map) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let tile_width = map.tile_width;
        let tile_height = map.tile_height;
        let depth_thickness = 80.0;
        let origin_x = -(map.width as f32 * tile_width) * 0.5;

        for x in 0..map.width {
            for y in 0..map.height {
                let tile = &map.tiles[x][y];
                
                if !tile.solid {
                    continue;
                }

                let world_x = origin_x + x as f32 * tile_width;
                let world_y = (map.height as f32 - 1.0 - y as f32) * tile_height;

                let left_solid = x > 0 && map.tiles[x - 1][y].solid;
                let right_solid = x < map.width - 1 && map.tiles[x + 1][y].solid;
                let top_solid = y > 0 && map.tiles[x][y - 1].solid;
                let bottom_solid = y < map.height - 1 && map.tiles[x][y + 1].solid;

                add_front_quad_xy(
                    &mut vertices,
                    &mut indices,
                    world_x,
                    world_y,
                    tile_width,
                    tile_height,
                    -depth_thickness,
                );

                if !left_solid {
                    add_side_quad_x(
                        &mut vertices,
                        &mut indices,
                        world_x,
                        world_y,
                        tile_height,
                        0.0,
                        -depth_thickness,
                        [-1.0, 0.0, 0.0],
                        false,
                    );
                }

                if !right_solid {
                    add_side_quad_x(
                        &mut vertices,
                        &mut indices,
                        world_x + tile_width,
                        world_y,
                        tile_height,
                        0.0,
                        -depth_thickness,
                        [1.0, 0.0, 0.0],
                        true,
                    );
                }

                if !top_solid {
                    add_side_quad_y(
                        &mut vertices,
                        &mut indices,
                        world_x,
                        world_x + tile_width,
                        world_y + tile_height,
                        0.0,
                        -depth_thickness,
                        [0.0, 1.0, 0.0],
                        false,
                    );
                }

                if !bottom_solid {
                    add_side_quad_y(
                        &mut vertices,
                        &mut indices,
                        world_x,
                        world_x + tile_width,
                        world_y,
                        0.0,
                        -depth_thickness,
                        [0.0, -1.0, 0.0],
                        true,
                    );
                }
            }
        }

        Self { vertices, indices }
    }
}

fn add_front_quad_xy(
    vertices: &mut Vec<VertexData>,
    indices: &mut Vec<u16>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    z: f32,
) {
    let base = vertices.len() as u16;

    vertices.push(VertexData {
        position: [x, y, z],
        uv: [0.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal: [0.0, 0.0, -1.0],
    });
    vertices.push(VertexData {
        position: [x + width, y, z],
        uv: [1.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal: [0.0, 0.0, -1.0],
    });
    vertices.push(VertexData {
        position: [x + width, y + height, z],
        uv: [1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal: [0.0, 0.0, -1.0],
    });
    vertices.push(VertexData {
        position: [x, y + height, z],
        uv: [0.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal: [0.0, 0.0, -1.0],
    });

    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn add_side_quad_x(
    vertices: &mut Vec<VertexData>,
    indices: &mut Vec<u16>,
    x: f32,
    y: f32,
    height: f32,
    z0: f32,
    z1: f32,
    normal: [f32; 3],
    reverse_winding: bool,
) {
    let base = vertices.len() as u16;

    vertices.push(VertexData {
        position: [x, y, z0],
        uv: [0.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal,
    });
    vertices.push(VertexData {
        position: [x, y, z1],
        uv: [1.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal,
    });
    vertices.push(VertexData {
        position: [x, y + height, z1],
        uv: [1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal,
    });
    vertices.push(VertexData {
        position: [x, y + height, z0],
        uv: [0.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal,
    });

    if reverse_winding {
        indices.extend_from_slice(&[base, base + 3, base + 2, base, base + 2, base + 1]);
    } else {
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

fn add_side_quad_y(
    vertices: &mut Vec<VertexData>,
    indices: &mut Vec<u16>,
    x0: f32,
    x1: f32,
    y: f32,
    z0: f32,
    z1: f32,
    normal: [f32; 3],
    reverse_winding: bool,
) {
    let base = vertices.len() as u16;

    vertices.push(VertexData {
        position: [x0, y, z0],
        uv: [0.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal,
    });
    vertices.push(VertexData {
        position: [x1, y, z0],
        uv: [1.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal,
    });
    vertices.push(VertexData {
        position: [x1, y, z1],
        uv: [1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal,
    });
    vertices.push(VertexData {
        position: [x0, y, z1],
        uv: [0.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        normal,
    });

    if reverse_winding {
        indices.extend_from_slice(&[base, base + 3, base + 2, base, base + 2, base + 1]);
    } else {
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}
