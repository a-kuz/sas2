use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AwardType {
    Excellent,
    Impressive,
    Humiliation,
    Perfect,
    Accuracy,
}

pub struct AwardTracker {
    last_kill_time: HashMap<u32, f32>,
    kill_count_in_window: HashMap<u32, u32>,
    match_start_time: f32,
    player_died: HashMap<u32, bool>,
}

impl AwardTracker {
    pub fn new() -> Self {
        Self {
            last_kill_time: HashMap::new(),
            kill_count_in_window: HashMap::new(),
            match_start_time: 0.0,
            player_died: HashMap::new(),
        }
    }

    pub fn reset_match(&mut self, current_time: f32) {
        self.match_start_time = current_time;
        self.player_died.clear();
    }

    pub fn register_kill(
        &mut self,
        killer_id: u32,
        _victim_id: u32,
        current_time: f32,
        weapon_name: &str,
        victim_was_in_air: bool,
    ) -> Vec<AwardType> {
        let mut awards = Vec::new();

        let last_time = self.last_kill_time.get(&killer_id).copied().unwrap_or(0.0);
        let time_since_last = current_time - last_time;

        if time_since_last < 2.0 {
            let count = self.kill_count_in_window.get(&killer_id).copied().unwrap_or(0);
            self.kill_count_in_window.insert(killer_id, count + 1);

            if count + 1 >= 2 {
                awards.push(AwardType::Excellent);
            }
        } else {
            self.kill_count_in_window.insert(killer_id, 1);
        }

        self.last_kill_time.insert(killer_id, current_time);

        if weapon_name == "Railgun" && victim_was_in_air {
            awards.push(AwardType::Impressive);
        }

        if weapon_name == "Gauntlet" {
            awards.push(AwardType::Humiliation);
        }

        awards
    }

    pub fn register_death(&mut self, player_id: u32) {
        self.player_died.insert(player_id, true);
    }

    pub fn check_perfect(&self, player_id: u32) -> bool {
        !self.player_died.get(&player_id).copied().unwrap_or(false)
    }

    pub fn check_accuracy(&self, shots_fired: u32, shots_hit: u32) -> bool {
        if shots_fired < 10 {
            return false;
        }

        let accuracy = (shots_hit as f32 / shots_fired as f32) * 100.0;
        accuracy >= 80.0
    }
}

