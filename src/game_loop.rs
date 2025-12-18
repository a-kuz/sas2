use std::time::{Duration, Instant};

pub struct GameLoop {
    last_update: Instant,
    accumulator: Duration,
    fixed_timestep: Duration,
}

impl GameLoop {
    pub fn new(fps: u32) -> Self {
        Self {
            last_update: Instant::now(),
            accumulator: Duration::ZERO,
            fixed_timestep: Duration::from_secs_f64(1.0 / fps as f64),
        }
    }

    pub fn tick<F>(&mut self, mut update_fn: F) -> f32
    where
        F: FnMut(f32),
    {
        let now = Instant::now();
        let frame_time = now.duration_since(self.last_update);
        self.last_update = now;

        self.accumulator += frame_time;

        let dt = self.fixed_timestep.as_secs_f32();
        
        while self.accumulator >= self.fixed_timestep {
            update_fn(dt);
            self.accumulator -= self.fixed_timestep;
        }

        dt
    }

    pub fn delta_time(&self) -> f32 {
        self.fixed_timestep.as_secs_f32()
    }
}

impl Default for GameLoop {
    fn default() -> Self {
        Self::new(60)
    }
}


