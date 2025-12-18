use std::path::{Path, PathBuf};

pub fn find_resource(relative_path: &str) -> Option<PathBuf> {
    let search_paths = [
        "",
        "../",
        "../../",
    ];

    for base in &search_paths {
        let full_path = Path::new(base).join(relative_path);
        if full_path.exists() {
            return Some(full_path);
        }
    }

    None
}

pub fn find_q3_resource(relative_path: &str) -> Option<PathBuf> {
    let q3_relative = format!("q3-resources/{}", relative_path);
    find_resource(&q3_relative)
}

pub fn find_model(model_name: &str, part: &str) -> Option<PathBuf> {
    let relative_path = format!("models/players/{}/{}.md3", model_name, part);
    find_q3_resource(&relative_path)
}

pub fn find_weapon_model(weapon_name: &str) -> Option<PathBuf> {
    let relative_path = format!("models/weapons2/{}/{}.md3", weapon_name, weapon_name);
    find_q3_resource(&relative_path)
}


