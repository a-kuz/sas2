use sas2::engine::md3::MD3Model;

fn main() {
    let paths = [
        "q3-resources/models/players/sarge/lower.md3",
        "../q3-resources/models/players/sarge/lower.md3",
    ];
    
    for path in &paths {
        if let Ok(model) = MD3Model::load(path) {
            println!("Loaded: {}", path);
            println!("Num frames: {}", model.header.num_bone_frames);
            
            for (mesh_idx, mesh) in model.meshes.iter().enumerate() {
                let name = std::str::from_utf8(&mesh.header.name).unwrap_or("?");
                println!("\nMesh {}: {}", mesh_idx, name.trim_end_matches('\0'));
                
                if let Some(frame) = mesh.vertices.first() {
                    let mut min_y = i16::MAX;
                    let mut max_y = i16::MIN;
                    
                    for v in frame {
                        min_y = min_y.min(v.vertex[2]); 
                        max_y = max_y.max(v.vertex[2]);
                    }
                    
                    println!("  Raw Y range: {} to {}", min_y, max_y);
                    println!("  Normalized Y range: {:.2} to {:.2}", 
                        min_y as f32 / 64.0, max_y as f32 / 64.0);
                    println!("  Height: {:.2} units", (max_y - min_y) as f32 / 64.0);
                }
            }
            break;
        }
    }
}
