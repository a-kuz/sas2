#[derive(Clone, Debug)]
pub struct AnimRange {
    pub first_frame: usize,
    pub num_frames: usize,
    pub looping_frames: usize,
    pub fps: usize,
}

#[derive(Clone, Debug)]
pub struct AnimConfig {
    pub both_death1: AnimRange,
    pub both_dead1: AnimRange,
    pub both_death2: AnimRange,
    pub both_dead2: AnimRange,
    pub both_death3: AnimRange,
    pub both_dead3: AnimRange,
    pub both_dead3_2: AnimRange, // added to match original
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
        let mut anims: Vec<AnimRange> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") || line.starts_with("sex") {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                if let (Ok(first), Ok(num), Ok(loop_frames), Ok(fps)) = (
                    parts[0].parse::<usize>(),
                    parts[1].parse::<usize>(),
                    parts[2].parse::<usize>(),
                    parts[3].parse::<usize>(),
                ) {
                    let anim_range = AnimRange {
                        first_frame: first,
                        num_frames: num,
                        looping_frames: loop_frames,
                        fps,
                    };
                    anims.push(anim_range);
                }
            }
        }

        if anims.len() < 25 {
             // Basic fallback if file is incomplete or follows different structure
             // But usually q3 animation.cfg has ~26 lines
        }
        
        // Q3 convention:
        // 0-5: deaths
        // 6: gesture
        // 7-8: attacks
        // 9: drop
        // 10: raise
        // 11-12: stand
        // 13: walkcr
        // 14: walk
        // 15: run
        // 16: back
        // 17: swim
        // 18: jump
        // 19: land
        // 20: jumpb
        // 21: landb
        // 22: idle
        // 23: idlecr
        // 24: turn

        // Calculate skip offset for legs
        let skip = if anims.len() > 13 {
             if anims[13].first_frame > anims[6].first_frame {
                 anims[13].first_frame - anims[6].first_frame
             } else {
                 0
             }
        } else {
            0
        };

        // Apply offset to legs animations
        for i in 13..anims.len() {
            anims[i].first_frame = anims[i].first_frame.saturating_sub(skip);
        }

        let get = |i: usize| -> AnimRange {
            anims.get(i).cloned().unwrap_or(AnimRange { first_frame: 0, num_frames: 1, looping_frames: 0, fps: 10 })
        };

        Ok(AnimConfig {
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
}

