use wgpu::*;
use crate::render::types::WgpuTexture;

pub fn create_tile_texture(device: &Device, queue: &Queue) -> WgpuTexture {
    let width = 64;
    let height = 64;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            
            let base_r = 75;
            let base_g = 75;
            let base_b = 80;
            
            let noise = ((x * 7 + y * 13) % 23) as i32 - 11;
            
            let r = (base_r as i32 + noise).clamp(0, 255) as u8;
            let g = (base_g as i32 + noise).clamp(0, 255) as u8;
            let b = (base_b as i32 + noise).clamp(0, 255) as u8;
            
            pixels[idx] = r;
            pixels[idx + 1] = g;
            pixels[idx + 2] = b;
            pixels[idx + 3] = 255;
        }
    }
    
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("Tile Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    
    queue.write_texture(
        ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        &pixels,
        ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );
    
    let view = texture.create_view(&TextureViewDescriptor::default());
    let sampler = device.create_sampler(&SamplerDescriptor {
        address_mode_u: AddressMode::Repeat,
        address_mode_v: AddressMode::Repeat,
        address_mode_w: AddressMode::Repeat,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: FilterMode::Linear,
        ..Default::default()
    });
    
    WgpuTexture {
        texture,
        view,
        sampler,
    }
}
