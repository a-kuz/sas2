use crate::game::map::Map;
use wgpu::{Device, Queue, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView, Extent3d, ImageCopyTexture, ImageDataLayout, Origin3d};

pub struct TileOcclusionData {
    pub texture: Texture,
    pub view: TextureView,
    pub width: u32,
    pub height: u32,
}

impl TileOcclusionData {
    pub fn from_map(device: &Device, queue: &Queue, map: &Map) -> Self {
        let width = map.width as u32;
        let height = map.height as u32;
        
        let mut data = Vec::with_capacity((width * height) as usize);
        
        for y in 0..height {
            for x in 0..width {
                let solid = map.tiles[x as usize][y as usize].solid;
                data.push(if solid { 255u8 } else { 0u8 });
            }
        }
        
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Tile Occlusion Texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        
        queue.write_texture(
            ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        Self {
            texture,
            view,
            width,
            height,
        }
    }
}

pub fn is_visible_through_tiles(
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    map: &Map,
) -> bool {
    let dx = to_x - from_x;
    let dy = to_y - from_y;
    let distance = (dx * dx + dy * dy).sqrt();
    
    if distance < 0.001 {
        return true;
    }
    
    let tile_size = map.tile_width.min(map.tile_height);
    let steps = ((distance / tile_size) * 2.0).ceil() as i32;
    let steps = steps.max(2).min(200);
    
    let step_x = dx / steps as f32;
    let step_y = dy / steps as f32;
    
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = from_x + dx * t;
        let y = from_y + dy * t;
        
        if map.is_solid_world(x, y) {
            return false;
        }
    }
    
    true
}

pub fn dda_line_of_sight(
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    map: &Map,
) -> bool {
    let dx = to_x - from_x;
    let dy = to_y - from_y;
    
    let tile_from_x = map.world_to_tile_x(from_x);
    let tile_from_y = map.world_to_tile_y(from_y);
    let tile_to_x = map.world_to_tile_x(to_x);
    let tile_to_y = map.world_to_tile_y(to_y);
    
    let mut x = tile_from_x;
    let mut y = tile_from_y;
    
    let step_x = if dx > 0.0 { 1 } else { -1 };
    let step_y = if dy > 0.0 { 1 } else { -1 };
    
    let t_delta_x = if dx.abs() > 0.0001 {
        map.tile_width / dx.abs()
    } else {
        f32::MAX
    };
    
    let t_delta_y = if dy.abs() > 0.0001 {
        map.tile_height / dy.abs()
    } else {
        f32::MAX
    };
    
    let origin_x = map.origin_x();
    let tile_edge_x = if step_x > 0 {
        origin_x + (x + 1) as f32 * map.tile_width
    } else {
        origin_x + x as f32 * map.tile_width
    };
    
    let tile_edge_y = if step_y > 0 {
        ((map.height as i32 - y) as f32) * map.tile_height
    } else {
        ((map.height as i32 - y - 1) as f32) * map.tile_height
    };
    
    let mut t_max_x = if dx.abs() > 0.0001 {
        (tile_edge_x - from_x) / dx
    } else {
        f32::MAX
    };
    
    let mut t_max_y = if dy.abs() > 0.0001 {
        (tile_edge_y - from_y) / dy
    } else {
        f32::MAX
    };
    
    let max_steps = 200;
    for _ in 0..max_steps {
        if x == tile_to_x && y == tile_to_y {
            break;
        }
        
        if map.is_solid(x, y) {
            if x == tile_from_x && y == tile_from_y {
            } else {
                return false;
            }
        }
        
        if t_max_x < t_max_y {
            t_max_x += t_delta_x;
            x += step_x;
        } else {
            t_max_y += t_delta_y;
            y += step_y;
        }
        
        if x < 0 || y < 0 || x >= map.width as i32 || y >= map.height as i32 {
            break;
        }
    }
    
    if map.is_solid(tile_to_x, tile_to_y) {
        return false;
    }
    
    true
}
