use crate::game::map::Map;
use crate::render::tile_occlusion::dda_line_of_sight;
use wgpu::{Device, Queue, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView, Extent3d, ImageCopyTexture, ImageDataLayout, Origin3d, Sampler, SamplerDescriptor, AddressMode, FilterMode};
use glam::Vec3;

pub struct Lightmap {
    pub texture: Texture,
    pub view: TextureView,
    pub sampler: Sampler,
    pub width: u32,
    pub height: u32,
}

impl Lightmap {
    pub fn bake_from_map(device: &Device, queue: &Queue, map: &Map, texels_per_tile: u32) -> Self {
        let width = (map.width as u32) * texels_per_tile;
        let height = (map.height as u32) * texels_per_tile;
        
        let mut data = vec![0u8; (width * height) as usize];
        
        let origin_x = map.origin_x();
        
        for ty in 0..height {
            for tx in 0..width {
                let world_x = origin_x + (tx as f32 / texels_per_tile as f32) * map.tile_width;
                let world_y = ((height - 1 - ty) as f32 / texels_per_tile as f32) * map.tile_height;
                
                let tile_x = map.world_to_tile_x(world_x);
                let tile_y = map.world_to_tile_y(world_y);
                
                if map.is_solid(tile_x, tile_y) {
                    data[(ty * width + tx) as usize] = 0;
                    continue;
                }
                
                let mut accumulated_light = 0.0f32;
                
                for light in &map.lights {
                    let light_pos = Vec3::new(light.x, light.y, 0.0);
                    let sample_pos = Vec3::new(world_x, world_y, 0.0);
                    
                    let dx = light_pos.x - sample_pos.x;
                    let dy = light_pos.y - sample_pos.y;
                    let dist_sq = dx * dx + dy * dy;
                    let radius_sq = light.radius * light.radius;
                    
                    if dist_sq > radius_sq {
                        continue;
                    }
                    
                    if !dda_line_of_sight(light_pos.x, light_pos.y, sample_pos.x, sample_pos.y, map) {
                        continue;
                    }
                    
                    let dist_norm_sq = dist_sq / radius_sq;
                    let falloff = 1.0 - dist_norm_sq;
                    let attenuation = falloff * falloff * falloff;
                    
                    let light_color = (light.r as f32 / 255.0 + light.g as f32 / 255.0 + light.b as f32 / 255.0) / 3.0;
                    let light_intensity = light_color * light.intensity;
                    accumulated_light += light_intensity * attenuation;
                }
                
                accumulated_light = accumulated_light.min(1.0);
                let byte_value = (accumulated_light * 255.0) as u8;
                data[(ty * width + tx) as usize] = byte_value;
            }
        }
        
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Lightmap Texture"),
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
        
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Lightmap Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });
        
        println!("Baked lightmap: {}x{} ({} lights)", width, height, map.lights.len());
        
        Self {
            texture,
            view,
            sampler,
            width,
            height,
        }
    }
}
