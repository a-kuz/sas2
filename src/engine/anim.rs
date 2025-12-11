#[derive(Clone, Debug)]
pub struct AnimRange {
    pub first_frame: usize,
    pub num_frames: usize,
    pub looping_frames: usize,
    pub fps: usize,
}

#[derive(Clone, Debug)]
pub struct AnimEntry {
    pub name: String,
    pub range: AnimRange,
}

#[derive(Clone, Debug)]
pub struct AnimConfig {
    pub entries: Vec<AnimEntry>,
    pub both_death1: AnimRange,
    pub both_dead1: AnimRange,
    pub both_death2: AnimRange,
    pub both_dead2: AnimRange,
    pub both_death3: AnimRange,
    pub both_dead3: AnimRange,
    pub both_dead3_2: AnimRange,
    pub torso_gesture: AnimRange,
    pub torso_attack: AnimRange,
    pub torso_attack2: AnimRange,
    pub torso_drop: AnimRange,
    pub torso_raise: AnimRange,
    pub torso_stand: AnimRange,
    pub torso_stand2: AnimRange,
    pub legs_walkcr: AnimRange,
    pub legs_walk: AnimRange,
    pub legs_run: AnimRange,
    pub legs_back: AnimRange,
    pub legs_swim: AnimRange,
    pub legs_jump: AnimRange,
    pub legs_land: AnimRange,
    pub legs_jumpb: AnimRange,
    pub legs_landb: AnimRange,
    pub legs_idle: AnimRange,
    pub legs_idlecr: AnimRange,
    pub legs_turn: AnimRange,
}

impl AnimConfig {
    pub fn load(model_name: &str) -> Result<Self, String> {
        let path = format!("q3-resources/models/players/{}/animation.cfg", model_name);
        let alt_path = format!("../q3-resources/models/players/{}/animation.cfg", model_name);
        
        let content = std::fs::read_to_string(&path)
            .or_else(|_| std::fs::read_to_string(&alt_path))
            .map_err(|e| format!("Failed to read animation.cfg: {}", e))?;
        
        Self::parse_content(&content)
    }

    pub fn parse_content(content: &str) -> Result<Self, String> {
        let mut entries: Vec<AnimEntry> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") || line.starts_with("sex") {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }
            let parsed = (
                parts[0].parse::<usize>(),
                parts[1].parse::<usize>(),
                parts[2].parse::<usize>(),
                parts[3].parse::<usize>(),
            );
            if let (Ok(first), Ok(num), Ok(loop_frames), Ok(fps)) = parsed {
                let name = line
                    .split("//")
                    .nth(1)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| format!("ANIM_{}", entries.len()));
                let range = AnimRange {
                    first_frame: first,
                    num_frames: num,
                    looping_frames: loop_frames,
                    fps,
                };
                entries.push(AnimEntry { name, range });
            }
        }

        let mut ranges: Vec<AnimRange> = entries.iter().map(|e| e.range.clone()).collect();

        let skip = if ranges.len() > 13 {
            if ranges[13].first_frame > ranges[6].first_frame {
                ranges[13].first_frame - ranges[6].first_frame
            } else {
                0
            }
        } else {
            0
        };

        for i in 13..ranges.len() {
            ranges[i].first_frame = ranges[i].first_frame.saturating_sub(skip);
        }

        for (entry, range) in entries.iter_mut().zip(ranges.iter()) {
            entry.range = range.clone();
        }

        let get = |i: usize| -> AnimRange {
            ranges
                .get(i)
                .cloned()
                .unwrap_or(AnimRange { first_frame: 0, num_frames: 1, looping_frames: 0, fps: 10 })
        };

        Ok(AnimConfig {
            entries,
            both_death1: get(0),
            both_dead1: get(1),
            both_death2: get(2),
            both_dead2: get(3),
            both_death3: get(4),
            both_dead3: get(5),
            both_dead3_2: get(5), // Reuse or placeholder
            torso_gesture: get(6),
            torso_attack: get(7),
            torso_attack2: get(8),
            torso_drop: get(9),
            torso_raise: get(10),
            torso_stand: get(11),
            torso_stand2: get(12),
            legs_walkcr: get(13),
            legs_walk: get(14),
            legs_run: get(15),
            legs_back: get(16),
            legs_swim: get(17),
            legs_jump: get(18),
            legs_land: get(19),
            legs_jumpb: get(20),
            legs_landb: get(21),
            legs_idle: get(22),
            legs_idlecr: get(23),
            legs_turn: get(24),
        })
    }

    pub fn by_name(&self, name: &str) -> Option<&AnimRange> {
        self.entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .map(|e| &e.range)
    }
}

