
pub struct Map {
    pub width: f32,
    pub height: f32,
    pub ground_y: f32,
}

impl Map {
    pub fn new() -> Self {
        Self {
            width: 1000.0,
            height: 1000.0,
            ground_y: 0.0,
        }
    }
}
