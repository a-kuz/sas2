use wgpu::*;
use crate::render::types::WgpuTexture;

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn add_coordinate_grid_to_texture(
    base_pixels: &[u8],
    width: u32,
    height: u32,
) -> Vec<u8> {
    let mut pixels = base_pixels.to_vec();
    
    for y in 0..height {
        for x in 0..width {
            let u = x as f32 / width as f32;
            let v = y as f32 / height as f32;
            
            let wall_size = 500.0;
            let wall_height = 50.0;
            
            let world_x = -wall_size + u * (2.0 * wall_size);
            let world_y = v * wall_height;
            
            let major_grid_step = 50.0;
            
            let x_major = (world_x / major_grid_step).round() * major_grid_step;
            let y_major = (world_y / major_grid_step).round() * major_grid_step;
            
            let x_major_dist = (world_x - x_major).abs();
            let y_major_dist = (world_y - y_major).abs();
            
            let idx = ((y * width + x) * 4) as usize;
            let mut r = pixels[idx] as f32 / 255.0;
            let mut g = pixels[idx + 1] as f32 / 255.0;
            let mut b = pixels[idx + 2] as f32 / 255.0;
            
            let line_thickness = 2.5;
            let is_on_x_line = x_major_dist < line_thickness;
            let is_on_y_line = y_major_dist < line_thickness;
            let is_intersection = is_on_x_line && is_on_y_line;
            
            let x_value = x_major as i32;
            let y_value = y_major as i32;
            let is_hundred_marker = (x_value % 100 == 0 && x_value != 0) || (y_value % 50 == 0 && y_value != 0);
            
            if world_x.abs() < line_thickness {
                r = 1.0;
                g = 0.0;
                b = 0.0;
            } else if world_y.abs() < line_thickness {
                r = 0.0;
                g = 1.0;
                b = 0.0;
            } else if is_intersection && is_hundred_marker {
                r = 1.0;
                g = 1.0;
                b = 0.0;
            } else if is_on_x_line || is_on_y_line {
                if is_hundred_marker {
                    r = 0.9;
                    g = 0.7;
                    b = 0.1;
                } else {
                    r = 0.6;
                    g = 0.4;
                    b = 0.1;
                }
            }
            
            pixels[idx] = (r * 255.0) as u8;
            pixels[idx + 1] = (g * 255.0) as u8;
            pixels[idx + 2] = (b * 255.0) as u8;
        }
    }
    
    pixels
}

pub fn create_ground_texture(device: &Device, queue: &Queue) -> WgpuTexture {
    let texture_paths = vec![
        "../q3-resources/textures/base_floor/clang_floor3b.png",
        "../q3-resources/textures/base_floor/clang_floor3.png",
        "../q3-resources/textures/base_floor/clang_floor2.png",
        "../q3-resources/textures/base_floor/clang_floor1.png",
        "../q3-resources/textures/base_floor/floor1.png",
        "q3-resources/textures/base_floor/clang_floor3b.png",
        "q3-resources/textures/base_floor/clang_floor3.png",
        "q3-resources/textures/base_floor/clang_floor2.png",
        "q3-resources/textures/base_floor/clang_floor1.png",
        "q3-resources/textures/base_floor/floor1.png",
    ];

    for texture_path in texture_paths {
        if std::path::Path::new(&texture_path).exists() {
            if let Ok(data) = std::fs::read(&texture_path) {
                if let Ok(img) = image::load_from_memory(&data) {
                    let img = img.to_rgba8();
                    let size = Extent3d {
                        width: img.width(),
                        height: img.height(),
                        depth_or_array_layers: 1,
                    };
                    let texture = device.create_texture(&TextureDescriptor {
                        label: Some("Ground Texture"),
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
                        &img,
                        ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * img.width()),
                            rows_per_image: Some(img.height()),
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

                    println!("Loaded ground texture from: {}", texture_path);
                    return WgpuTexture {
                        texture,
                        view,
                        sampler,
                    };
                }
            }
        }
    }

    println!("Warning: Could not load ground texture, using fallback");
    let size = 128u32;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    
    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;
            
            let checker = ((fx * 8.0).floor() + (fy * 8.0).floor()) as i32;
            let is_dark = checker % 2 == 0;
            let r = if is_dark { 0.25 } else { 0.18 };
            let g = if is_dark { 0.25 } else { 0.18 };
            let b = if is_dark { 0.28 } else { 0.2 };
            
            pixels.push((r * 255.0) as u8);
            pixels.push((g * 255.0) as u8);
            pixels.push((b * 255.0) as u8);
            pixels.push(255);
        }
    }
    
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("Ground Texture Fallback"),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
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
            bytes_per_row: Some(4 * size),
            rows_per_image: Some(size),
        },
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
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

pub fn create_wall_texture(device: &Device, queue: &Queue) -> (WgpuTexture, WgpuTexture) {
    let texture_paths = vec![
        "../q3-resources/textures/base_wall/atech2_c.png",
        "../q3-resources/textures/base_wall/atech3_a.png",
        "../q3-resources/textures/base_wall/basewall04.png",
        "../q3-resources/textures/base_wall/concrete.png",
        "../q3-resources/textures/base_wall/atech1_a.png",
        "q3-resources/textures/base_wall/atech2_c.png",
        "q3-resources/textures/base_wall/atech3_a.png",
        "q3-resources/textures/base_wall/basewall04.png",
        "q3-resources/textures/base_wall/concrete.png",
        "q3-resources/textures/base_wall/atech1_a.png",
    ];

    let mut wall_texture = None;
    for texture_path in texture_paths {
        if std::path::Path::new(&texture_path).exists() {
            if let Ok(data) = std::fs::read(&texture_path) {
                if let Ok(img) = image::load_from_memory(&data) {
                    let img = img.to_rgba8();
                    let size = Extent3d {
                        width: img.width(),
                        height: img.height(),
                        depth_or_array_layers: 1,
                    };
                    
                    println!("Loaded wall texture from: {}", texture_path);
                    
                    let img_data = img.as_raw();
                    
                    let texture = device.create_texture(&TextureDescriptor {
                        label: Some("Wall Texture"),
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
                        img_data,
                        ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * img.width()),
                            rows_per_image: Some(img.height()),
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
                    
                    wall_texture = Some(WgpuTexture {
                        texture,
                        view,
                        sampler,
                    });
                    break;
                }
            }
        }
    }

    let curb_texture_paths = vec![
        "../q3-resources/textures/base_trim/border11.png",
        "../q3-resources/textures/base_trim/spiderbit4.png",
        "../q3-resources/textures/base_trim/dirty_pewter_big.png",
        "../q3-resources/textures/base_trim/rusty_pewter_big.png",
        "../q3-resources/textures/base_trim/metal2_2.png",
        "../q3-resources/textures/base_trim/pewter.png",
        "../q3-resources/textures/base_trim/tin.png",
        "q3-resources/textures/base_trim/border11.png",
        "q3-resources/textures/base_trim/spiderbit4.png",
        "q3-resources/textures/base_trim/dirty_pewter_big.png",
        "q3-resources/textures/base_trim/rusty_pewter_big.png",
        "q3-resources/textures/base_trim/metal2_2.png",
        "q3-resources/textures/base_trim/pewter.png",
        "q3-resources/textures/base_trim/tin.png",
    ];

    let mut curb_texture = None;
    for texture_path in curb_texture_paths {
        if std::path::Path::new(&texture_path).exists() {
            if let Ok(data) = std::fs::read(&texture_path) {
                if let Ok(img) = image::load_from_memory(&data) {
                    let img = img.to_rgba8();
                    let size = Extent3d {
                        width: img.width(),
                        height: img.height(),
                        depth_or_array_layers: 1,
                    };
                    let texture = device.create_texture(&TextureDescriptor {
                        label: Some("Wall Curb Texture"),
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
                        &img,
                        ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * img.width()),
                            rows_per_image: Some(img.height()),
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

                    println!("Loaded wall curb texture from: {}", texture_path);
                    curb_texture = Some(WgpuTexture {
                        texture,
                        view,
                        sampler,
                    });
                    break;
                }
            }
        }
    }

    let curb_texture = curb_texture.unwrap_or_else(|| {
        println!("Warning: Could not load wall curb texture, creating fallback");
        let size = 128u32;
        let mut pixels = Vec::with_capacity((size * size * 4) as usize);
        
        for y in 0..size {
            for x in 0..size {
                let fx = x as f32 / size as f32;
                let fy = y as f32 / size as f32;
                
                let rust_pattern = (fx * 4.0).sin() * (fy * 4.0).cos();
                let base_rust_r = 0.4;
                let base_rust_g = 0.25;
                let base_rust_b = 0.15;
                let rust_highlight_r = 0.6;
                let rust_highlight_g = 0.35;
                let rust_highlight_b = 0.2;
                let mix_factor = rust_pattern * 0.5 + 0.5;
                let rust_r = base_rust_r + (rust_highlight_r - base_rust_r) * mix_factor;
                let rust_g = base_rust_g + (rust_highlight_g - base_rust_g) * mix_factor;
                let rust_b = base_rust_b + (rust_highlight_b - base_rust_b) * mix_factor;
                
                let rivet_dx = fx - 0.5;
                let rivet_dy = fy - 0.5;
                let rivet_dist = (rivet_dx * rivet_dx + rivet_dy * rivet_dy).sqrt();
                let rivet = if rivet_dist < 0.1 {
                    1.0 - (rivet_dist - 0.1) / 0.05
                } else if rivet_dist < 0.15 {
                    1.0 - (rivet_dist - 0.1) / 0.05
                } else {
                    0.0
                };
                let rivet_r = rust_r + (0.7 - rust_r) * rivet;
                let rivet_g = rust_g + (0.6 - rust_g) * rivet;
                let rivet_b = rust_b + (0.4 - rust_b) * rivet;
                
                pixels.push((rivet_r * 255.0) as u8);
                pixels.push((rivet_g * 255.0) as u8);
                pixels.push((rivet_b * 255.0) as u8);
                pixels.push(255);
            }
        }
        
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Wall Curb Texture Fallback"),
            size: Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
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
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
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
    });

    let wall_texture = wall_texture.unwrap_or_else(|| {
        println!("Warning: Could not load wall texture, using fallback with grid");
        let size = 1024u32;
        let mut base_pixels = Vec::with_capacity((size * size * 4) as usize);
        
        for y in 0..size {
            for x in 0..size {
                let fx = x as f32 / size as f32;
                let fy = y as f32 / size as f32;
                
                let noise_x = (fx * 8.0 + (fy * 3.14159).sin() * 0.3).fract();
                let noise_y = (fy * 8.0 + (fx * 2.71828).cos() * 0.3).fract();
                
                let stone_pattern = (noise_x * 3.14159).sin() * (noise_y * 2.71828).cos();
                let base_gray = 0.4 + stone_pattern * 0.15;
                
                let r = base_gray + (fx * 10.0).sin() * 0.05;
                let g = base_gray + (fy * 7.0).cos() * 0.05;
                let b = base_gray + ((fx + fy) * 5.0).sin() * 0.05;
                
                base_pixels.push((r * 255.0) as u8);
                base_pixels.push((g * 255.0) as u8);
                base_pixels.push((b * 255.0) as u8);
                base_pixels.push(255);
            }
        }
        
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Wall Texture Fallback"),
            size: Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
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
            &base_pixels,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
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
    });

    (wall_texture, curb_texture)
}

pub fn create_smoke_texture(device: &Device, queue: &Queue) -> WgpuTexture {
    let candidates = vec![
        "q3-resources/gfx/misc/smokepuff2b.png",
        "q3-resources/gfx/misc/smokepuff3.png",
        "../q3-resources/gfx/misc/smokepuff2b.png",
        "../q3-resources/gfx/misc/smokepuff3.png",
    ];

    for path in candidates {
        if std::path::Path::new(path).exists() {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(img) = image::load_from_memory(&data) {
                    let img = img.to_rgba8();
                    let size = Extent3d {
                        width: img.width(),
                        height: img.height(),
                        depth_or_array_layers: 1,
                    };
                    let texture = device.create_texture(&TextureDescriptor {
                        label: Some("Smoke Texture"),
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: TextureDimension::D2,
                        format: TextureFormat::Rgba8Unorm,
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
                        &img,
                        ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * img.width()),
                            rows_per_image: Some(img.height()),
                        },
                        size,
                    );

                    let view = texture.create_view(&TextureViewDescriptor::default());
                    let sampler = device.create_sampler(&SamplerDescriptor {
                        address_mode_u: AddressMode::ClampToEdge,
                        address_mode_v: AddressMode::ClampToEdge,
                        address_mode_w: AddressMode::ClampToEdge,
                        mag_filter: FilterMode::Linear,
                        min_filter: FilterMode::Linear,
                        mipmap_filter: FilterMode::Linear,
                        ..Default::default()
                    });

                    return WgpuTexture {
                        texture,
                        view,
                        sampler,
                    };
                }
            }
        }
    }

    let size = 64u32;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    let center = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let fx = x as f32;
            let fy = y as f32;
            let dx = fx - center;
            let dy = fy - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let max_dist = center * 0.9;
            let normalized_dist = (dist / max_dist).min(1.0);
            let alpha = smoothstep(1.0, 0.3, normalized_dist);
            let base_color = 0.8;
            pixels.push((base_color * 255.0) as u8);
            pixels.push((base_color * 255.0) as u8);
            pixels.push((base_color * 255.0) as u8);
            pixels.push((alpha.min(1.0) * 255.0) as u8);
        }
    }
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("Smoke Texture Fallback"),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
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
            bytes_per_row: Some(4 * size),
            rows_per_image: Some(size),
        },
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&TextureViewDescriptor::default());
    let sampler = device.create_sampler(&SamplerDescriptor {
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
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

pub fn create_flame_texture(device: &Device, queue: &Queue) -> WgpuTexture {
    let candidates = vec![
        "q3-resources/models/ammo/rocket/rockflar.png",
        "q3-resources/models/ammo/rocket/rockfls1.png",
        "q3-resources/models/ammo/rocket/rockfls2.png",
        "../q3-resources/models/ammo/rocket/rockflar.png",
        "../q3-resources/models/ammo/rocket/rockfls1.png",
        "../q3-resources/models/ammo/rocket/rockfls2.png",
    ];

    for path in candidates {
        if std::path::Path::new(path).exists() {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(img) = image::load_from_memory(&data) {
                    let img = img.to_rgba8();
                    let size = Extent3d {
                        width: img.width(),
                        height: img.height(),
                        depth_or_array_layers: 1,
                    };
                    let texture = device.create_texture(&TextureDescriptor {
                        label: Some("Flame Texture"),
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
                        &img,
                        ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * img.width()),
                            rows_per_image: Some(img.height()),
                        },
                        size,
                    );

                    let view = texture.create_view(&TextureViewDescriptor::default());
                    let sampler = device.create_sampler(&SamplerDescriptor {
                        address_mode_u: AddressMode::ClampToEdge,
                        address_mode_v: AddressMode::ClampToEdge,
                        address_mode_w: AddressMode::ClampToEdge,
                        mag_filter: FilterMode::Linear,
                        min_filter: FilterMode::Linear,
                        mipmap_filter: FilterMode::Linear,
                        ..Default::default()
                    });

                    return WgpuTexture {
                        texture,
                        view,
                        sampler,
                    };
                }
            }
        }
    }

    let size = 64u32;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    let center = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let fx = x as f32;
            let fy = y as f32;
            let dx = fx - center;
            let dy = fy - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let max_dist = center * 0.9;
            let normalized_dist = (dist / max_dist).min(1.0);
            let alpha = smoothstep(1.0, 0.0, normalized_dist);
            pixels.push(255);
            pixels.push(200);
            pixels.push(100);
            pixels.push((alpha.min(1.0) * 255.0) as u8);
        }
    }
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("Flame Texture Fallback"),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
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
            bytes_per_row: Some(4 * size),
            rows_per_image: Some(size),
        },
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&TextureViewDescriptor::default());
    let sampler = device.create_sampler(&SamplerDescriptor {
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
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

