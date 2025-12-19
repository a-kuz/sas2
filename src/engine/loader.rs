use wgpu::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, ImageCopyTexture, Origin3d, TextureAspect, ImageDataLayout, TextureViewDescriptor, SamplerDescriptor, FilterMode, AddressMode};
use crate::engine::renderer::{WgpuRenderer, MD3Renderer, WgpuTexture};
use crate::engine::md3::MD3Model;
use std::path::Path;

pub fn load_textures_for_model_static(
    wgpu_renderer: &mut WgpuRenderer,
    md3_renderer: &mut MD3Renderer,
    model: &MD3Model,
    model_name: &str,
    part: &str,
) -> Vec<Option<String>> {
    let mut texture_paths = Vec::new();
    let mut mesh_texture_map = std::collections::HashMap::new();
    
    let skin_candidates = vec![
        format!("q3-resources/models/players/{}/{}_default.skin", model_name, part),
        format!("../q3-resources/models/players/{}/{}_default.skin", model_name, part),
        format!("q3-resources/models/players/{}/{}.skin", model_name, part),
        format!("../q3-resources/models/players/{}/{}.skin", model_name, part),
    ];
    
    for skin_path in skin_candidates {
        if let Ok(content) = std::fs::read_to_string(&skin_path) {
            println!("Loaded skin file: {}", skin_path);
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with("//") {
                    continue;
                }
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() == 2 {
                    let mesh_name = parts[0].trim().to_string();
                    let mut texture_path = parts[1].trim().to_string();
                    if !texture_path.is_empty() {
                        if !texture_path.starts_with("q3-resources/") {
                            texture_path = format!("q3-resources/{}", texture_path);
                        }
                        let texture_path_clone = texture_path.clone();
                        mesh_texture_map.insert(mesh_name.clone(), texture_path);
                        println!("  Mesh '{}' -> texture '{}'", mesh_name, texture_path_clone);
                    }
                }
            }
            break;
        }
    }
    
    for (_mesh_idx, mesh) in model.meshes.iter().enumerate() {
        let mesh_name = std::str::from_utf8(&mesh.header.name)
            .unwrap_or("")
            .trim_end_matches('\0')
            .to_string();
        
        let texture_path = mesh_texture_map.get(&mesh_name)
            .cloned()
            .or_else(|| {
                let candidates = vec![
                    format!("q3-resources/models/players/{}/{}_{}.tga", model_name, part, mesh_name),
                    format!("q3-resources/models/players/{}/{}_{}.png", model_name, part, mesh_name),
                    format!("q3-resources/models/players/{}/{}_{}.jpg", model_name, part, mesh_name),
                    format!("../q3-resources/models/players/{}/{}_{}.tga", model_name, part, mesh_name),
                    format!("../q3-resources/models/players/{}/{}_{}.png", model_name, part, mesh_name),
                    format!("../q3-resources/models/players/{}/{}_{}.jpg", model_name, part, mesh_name),
                    format!("q3-resources/models/players/{}/{}.tga", model_name, mesh_name),
                    format!("q3-resources/models/players/{}/{}.png", model_name, mesh_name),
                    format!("q3-resources/models/players/{}/{}.jpg", model_name, mesh_name),
                    format!("../q3-resources/models/players/{}/{}.tga", model_name, mesh_name),
                    format!("../q3-resources/models/players/{}/{}.png", model_name, mesh_name),
                    format!("../q3-resources/models/players/{}/{}.jpg", model_name, mesh_name),
                ];
                candidates.iter()
                    .find(|p| std::path::Path::new(p).exists())
                    .map(|s| s.to_string())
            });
        
        let mut texture_loaded = false;
        if let Some(ref path) = texture_path {
            let mut alt_paths = vec![];
            
            if path.ends_with(".TGA") {
                let png_path = path.replace(".TGA", ".png");
                alt_paths.push(png_path.clone());
                alt_paths.push(format!("../{}", png_path));
            }
            
            alt_paths.push(path.clone());
            alt_paths.push(format!("../{}", path));
            
            if path.ends_with(".png") {
                alt_paths.push(path.replace(".png", ".jpg"));
                alt_paths.push(path.replace(".png", ".tga"));
                alt_paths.push(format!("../{}", path.replace(".png", ".jpg")));
                alt_paths.push(format!("../{}", path.replace(".png", ".tga")));
            } else if path.ends_with(".jpg") {
                alt_paths.push(path.replace(".jpg", ".png"));
                alt_paths.push(path.replace(".jpg", ".tga"));
                alt_paths.push(format!("../{}", path.replace(".jpg", ".png")));
                alt_paths.push(format!("../{}", path.replace(".jpg", ".tga")));
            } else if path.ends_with(".tga") {
                alt_paths.push(path.replace(".tga", ".png"));
                alt_paths.push(path.replace(".tga", ".jpg"));
                alt_paths.push(format!("../{}", path.replace(".tga", ".png")));
                alt_paths.push(format!("../{}", path.replace(".tga", ".jpg")));
            } else if path.ends_with(".TGA") {
                alt_paths.push(path.replace(".TGA", ".jpg"));
                alt_paths.push(format!("../{}", path.replace(".TGA", ".jpg")));
            }
            
            for alt_path in alt_paths {
                if std::path::Path::new(&alt_path).exists() {
                    if let Ok(data) = std::fs::read(&alt_path) {
                        if let Ok(img) = image::load_from_memory(&data) {
                            let img = img.to_rgba8();
                            let size = Extent3d {
                                width: img.width(),
                                height: img.height(),
                                depth_or_array_layers: 1,
                            };
                            let texture = wgpu_renderer.device.create_texture(&TextureDescriptor {
                                label: Some("MD3 Texture"),
                                size,
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: TextureDimension::D2,
                                format: TextureFormat::Rgba8UnormSrgb,
                                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                                view_formats: &[],
                            });

                            wgpu_renderer.queue.write_texture(
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
                            let sampler = wgpu_renderer.device.create_sampler(&SamplerDescriptor {
                                address_mode_u: AddressMode::Repeat,
                                address_mode_v: AddressMode::Repeat,
                                address_mode_w: AddressMode::Repeat,
                                mag_filter: FilterMode::Linear,
                                min_filter: FilterMode::Linear,
                                mipmap_filter: FilterMode::Linear,
                                ..Default::default()
                            });

                            let wgpu_tex = WgpuTexture {
                                texture,
                                view,
                                sampler,
                            };

                            md3_renderer.load_texture(path, wgpu_tex);
                            println!("Loaded texture: {} for mesh: {} (from file: {})", path, mesh_name, alt_path);
                            texture_loaded = true;
                            break;
                        }
                    }
                }
            }
            if !texture_loaded {
                println!("Warning: texture not found for mesh: {} (path: {:?})", mesh_name, path);
            }
        } else {
            println!("Warning: no texture path for mesh: {}", mesh_name);
        }
        
        texture_paths.push(texture_path);
    }
    
    println!("Total textures loaded: {}/{}", texture_paths.iter().filter(|p| p.is_some()).count(), texture_paths.len());
    texture_paths
}

pub fn load_weapon_textures_static(
    wgpu_renderer: &mut WgpuRenderer,
    md3_renderer: &mut MD3Renderer,
    model: &MD3Model,
) -> Vec<Option<String>> {
    let mut texture_paths = Vec::new();
    
    let weapon_candidates: Vec<Vec<&str>> = vec![
        vec![
            "q3-resources/models/weapons2/rocketl/rocketl.png",
            "q3-resources/models/weapons2/rocketl/rocketl.jpg",
            "../q3-resources/models/weapons2/rocketl/rocketl.png",
            "../q3-resources/models/weapons2/rocketl/rocketl.jpg",
        ],
        vec![
            "q3-resources/models/weapons2/rocketl/rocketl2.png",
            "q3-resources/models/weapons2/rocketl/rocketl2.jpg",
            "../q3-resources/models/weapons2/rocketl/rocketl2.png",
            "../q3-resources/models/weapons2/rocketl/rocketl2.jpg",
        ],
    ];

    for (_mesh_idx, candidates) in weapon_candidates.iter().take(model.meshes.len()).enumerate() {
        let texture_path = candidates
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .map(|s| s.to_string());

        if let Some(ref path) = texture_path {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(img) = image::load_from_memory(&data) {
                    let img = img.to_rgba8();
                    let size = Extent3d {
                        width: img.width(),
                        height: img.height(),
                        depth_or_array_layers: 1,
                    };
                    let texture = wgpu_renderer.device.create_texture(&TextureDescriptor {
                        label: Some("Weapon Texture"),
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: TextureDimension::D2,
                        format: TextureFormat::Rgba8UnormSrgb,
                        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                        view_formats: &[],
                    });

                    wgpu_renderer.queue.write_texture(
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
                    let sampler = wgpu_renderer.device.create_sampler(&SamplerDescriptor {
                        address_mode_u: AddressMode::Repeat,
                        address_mode_v: AddressMode::Repeat,
                        address_mode_w: AddressMode::Repeat,
                        mag_filter: FilterMode::Linear,
                        min_filter: FilterMode::Linear,
                        mipmap_filter: FilterMode::Linear,
                        ..Default::default()
                    });

                    let wgpu_tex = WgpuTexture {
                        texture,
                        view,
                        sampler,
                    };

                    md3_renderer.load_texture(path, wgpu_tex);
                }
            }
        }

        texture_paths.push(texture_path);
    }
    
    texture_paths
}

pub fn load_rocket_textures_static(
    wgpu_renderer: &mut WgpuRenderer,
    md3_renderer: &mut MD3Renderer,
    model: &MD3Model,
) -> Vec<Option<String>> {
    let mut texture_paths = Vec::new();
    
    for mesh in &model.meshes {
        let raw_name = std::str::from_utf8(&mesh.header.name)
            .unwrap_or("")
            .trim_end_matches('\0');
        let shader_name = if raw_name.is_empty() || raw_name == "default" {
            "rocket"
        } else {
            raw_name
        };
        
        let candidates = vec![
            format!("q3-resources/models/ammo/rocket/{}.png", shader_name),
            format!("q3-resources/models/ammo/rocket/{}.jpg", shader_name),
            format!("q3-resources/models/ammo/rocket/{}.tga", shader_name),
            format!("../q3-resources/models/ammo/rocket/{}.png", shader_name),
            format!("../q3-resources/models/ammo/rocket/{}.jpg", shader_name),
            format!("../q3-resources/models/ammo/rocket/{}.tga", shader_name),
        ];
        
        let texture_path = candidates
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .map(|s| s.to_string());

        if let Some(ref path) = texture_path {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(img) = image::load_from_memory(&data) {
                    let img = img.to_rgba8();
                    let size = Extent3d {
                        width: img.width(),
                        height: img.height(),
                        depth_or_array_layers: 1,
                    };
                    let texture = wgpu_renderer.device.create_texture(&TextureDescriptor {
                        label: Some("Rocket Texture"),
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: TextureDimension::D2,
                        format: TextureFormat::Rgba8UnormSrgb,
                        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                        view_formats: &[],
                    });

                    wgpu_renderer.queue.write_texture(
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
                    let sampler = wgpu_renderer.device.create_sampler(&SamplerDescriptor {
                        address_mode_u: AddressMode::Repeat,
                        address_mode_v: AddressMode::Repeat,
                        address_mode_w: AddressMode::Repeat,
                        mag_filter: FilterMode::Linear,
                        min_filter: FilterMode::Linear,
                        mipmap_filter: FilterMode::Linear,
                        ..Default::default()
                    });

                    let wgpu_tex = WgpuTexture {
                        texture,
                        view,
                        sampler,
                    };

                    md3_renderer.load_texture(path, wgpu_tex);
                }
            }
        }

        texture_paths.push(texture_path);
    }
    
    texture_paths
}

pub fn load_md3_textures_guess_static(
    wgpu_renderer: &mut WgpuRenderer,
    md3_renderer: &mut MD3Renderer,
    model: &MD3Model,
    model_path: &str,
) -> Vec<Option<String>> {
    let path = Path::new(model_path);
    let base_dir_raw = path.parent().and_then(|p| p.to_str()).unwrap_or("");
    let base_dir = base_dir_raw
        .trim_start_matches("../")
        .trim_start_matches("q3-resources/");
    let base_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    let mut texture_paths = Vec::new();

    for (mesh_idx, mesh) in model.meshes.iter().enumerate() {
        let raw_name = std::str::from_utf8(&mesh.header.name).unwrap_or("").trim_end_matches('\0');
        let mesh_name = if raw_name.is_empty() || raw_name == "default" {
            base_name
        } else {
            raw_name
        };

        let mut candidate_names = Vec::new();
        if !mesh_name.is_empty() {
            candidate_names.push(mesh_name.to_string());
        }
        if !base_name.is_empty() && base_name != mesh_name {
            candidate_names.push(base_name.to_string());
        }

        if !base_name.is_empty() {
            candidate_names.push(format!("{}{}", base_name, mesh_idx + 1));
            candidate_names.push(format!("{}{}", base_name, mesh_idx + 2));
            candidate_names.push(format!("{}3", base_name));
            candidate_names.push(format!("{}4", base_name));
        }

        candidate_names.dedup();

        let mut found: Option<String> = None;
        for name in candidate_names {
            let candidates = [
                format!("q3-resources/{}/{}.png", base_dir, name),
                format!("q3-resources/{}/{}.jpg", base_dir, name),
                format!("q3-resources/{}/{}.tga", base_dir, name),
                format!("q3-resources/{}/{}.TGA", base_dir, name),
                format!("../q3-resources/{}/{}.png", base_dir, name),
                format!("../q3-resources/{}/{}.jpg", base_dir, name),
                format!("../q3-resources/{}/{}.tga", base_dir, name),
                format!("../q3-resources/{}/{}.TGA", base_dir, name),
            ];

            for candidate in candidates {
                if !Path::new(&candidate).exists() {
                    continue;
                }
                if let Ok(data) = std::fs::read(&candidate) {
                    if let Ok(img) = image::load_from_memory(&data) {
                        let img = img.to_rgba8();
                        let size = Extent3d {
                            width: img.width(),
                            height: img.height(),
                            depth_or_array_layers: 1,
                        };
                        let texture = wgpu_renderer.device.create_texture(&TextureDescriptor {
                            label: Some("MD3 Guess Texture"),
                            size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: TextureDimension::D2,
                            format: TextureFormat::Rgba8UnormSrgb,
                            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                            view_formats: &[],
                        });

                        wgpu_renderer.queue.write_texture(
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
                        let sampler = wgpu_renderer.device.create_sampler(&SamplerDescriptor {
                            address_mode_u: AddressMode::Repeat,
                            address_mode_v: AddressMode::Repeat,
                            address_mode_w: AddressMode::Repeat,
                            mag_filter: FilterMode::Linear,
                            min_filter: FilterMode::Linear,
                            mipmap_filter: FilterMode::Linear,
                            ..Default::default()
                        });

                        let wgpu_tex = WgpuTexture { texture, view, sampler };

                        let key = candidate.trim_start_matches("../").to_string();
                        md3_renderer.load_texture(&key, wgpu_tex);
                        found = Some(key);
                        break;
                    }
                }
            }

            if found.is_some() {
                break;
            }
        }

        texture_paths.push(found);
    }

    texture_paths
}