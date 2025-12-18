use glam::Vec3;
use crate::game::player::Player;
use crate::game::weapon::Weapon;

pub struct DamageEvent {
    pub victim_id: u32,
    pub attacker_id: u32,
    pub damage: i32,
    pub weapon: Weapon,
    pub knockback: Option<Vec3>,
    pub was_in_air: bool,
}

pub struct CombatResult {
    pub killed: bool,
    pub gibbed: bool,
    pub final_health: i32,
}

pub fn apply_damage(
    player: &mut Player,
    damage: i32,
    attacker_has_quad: bool,
    knockback: Option<Vec3>,
) -> CombatResult {
    let mut final_damage = damage;
    
    if attacker_has_quad {
        final_damage *= 3;
    }

    let killed = player.damage(final_damage);
    
    if let Some(kb) = knockback {
        let knockback_strength = (final_damage as f32 * 0.08571428571428572).min(14.285714285714286);
        player.vx += kb.x * knockback_strength;
        player.vy += kb.y * knockback_strength;
    }

    CombatResult {
        killed,
        gibbed: player.gibbed,
        final_health: player.health,
    }
}

pub fn apply_self_damage(
    player: &mut Player,
    damage: i32,
    knockback: Option<Vec3>,
) -> CombatResult {
    let final_damage = damage / 2;
    
    let killed = player.damage(final_damage);
    
    if let Some(kb) = knockback {
        let knockback_strength = (final_damage as f32 * 0.05714285714285714).min(11.428571428571429);
        player.vx += kb.x * knockback_strength;
        player.vy += kb.y * knockback_strength;
    }

    CombatResult {
        killed,
        gibbed: player.gibbed,
        final_health: player.health,
    }
}

pub fn check_telefrag(
    teleporter_id: u32,
    teleport_dest: Vec3,
    players: &[Player],
) -> Vec<u32> {
    let mut victims = Vec::new();
    
    for player in players {
        if player.id == teleporter_id || player.dead {
            continue;
        }
        
        let player_pos = Vec3::new(player.x, player.y, 0.0);
        let distance = (teleport_dest - player_pos).length();
        
        if distance < 0.9142857142857143 {
            victims.push(player.id);
        }
    }
    
    victims
}

