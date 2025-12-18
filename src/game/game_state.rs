pub struct GameState {
    pub match_time: f32,
    pub match_duration: f32,
    pub match_started: bool,
    pub match_ended: bool,
    pub frag_limit: i32,
    pub time_limit: f32,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            match_time: 0.0,
            match_duration: 600.0,
            match_started: true,
            match_ended: false,
            frag_limit: 20,
            time_limit: 600.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if !self.match_started || self.match_ended {
            return;
        }

        self.match_time += dt;

        if self.match_time >= self.time_limit {
            self.match_ended = true;
        }
    }

    pub fn check_frag_limit(&mut self, max_frags: i32) {
        if max_frags >= self.frag_limit {
            self.match_ended = true;
        }
    }

    pub fn remaining_time(&self) -> f32 {
        (self.time_limit - self.match_time).max(0.0)
    }
}


