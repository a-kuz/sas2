use winit::keyboard::KeyCode;
use crate::game::weapon::Weapon;

#[derive(Default)]
pub struct InputState {
    pub move_left: bool,
    pub move_right: bool,
    pub move_up: bool,
    pub move_down: bool,
    pub jump: bool,
    pub crouch: bool,
    pub fire: bool,
    pub gesture: bool,
    pub switch_model: bool,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub weapon_switch: Option<Weapon>,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_key_press(&mut self, keycode: KeyCode) {
        match keycode {
            KeyCode::KeyA => self.move_left = true,
            KeyCode::KeyD => self.move_right = true,
            KeyCode::KeyW => self.jump = true,
            KeyCode::KeyS => self.crouch = true,
            KeyCode::Space => self.fire = true,
            KeyCode::KeyG => self.gesture = true,
            KeyCode::KeyM => self.switch_model = true,
            KeyCode::Digit1 => self.weapon_switch = Some(Weapon::Gauntlet),
            KeyCode::Digit2 => self.weapon_switch = Some(Weapon::MachineGun),
            KeyCode::Digit3 => self.weapon_switch = Some(Weapon::Shotgun),
            KeyCode::Digit4 => self.weapon_switch = Some(Weapon::GrenadeLauncher),
            KeyCode::Digit5 => self.weapon_switch = Some(Weapon::RocketLauncher),
            KeyCode::Digit6 => self.weapon_switch = Some(Weapon::Lightning),
            KeyCode::Digit7 => self.weapon_switch = Some(Weapon::Railgun),
            KeyCode::Digit8 => self.weapon_switch = Some(Weapon::Plasmagun),
            KeyCode::Digit9 => self.weapon_switch = Some(Weapon::BFG),
            _ => {}
        }
    }

    pub fn handle_key_release(&mut self, keycode: KeyCode) {
        match keycode {
            KeyCode::KeyA => self.move_left = false,
            KeyCode::KeyD => self.move_right = false,
            KeyCode::KeyW => self.jump = false,
            KeyCode::KeyS => self.crouch = false,
            KeyCode::Space => self.fire = false,
            KeyCode::KeyG => self.gesture = false,
            KeyCode::KeyM => self.switch_model = false,
            _ => {}
        }
    }

    pub fn handle_mouse_button_press(&mut self) {
        self.fire = true;
    }

    pub fn handle_mouse_button_release(&mut self) {
        self.fire = false;
    }

    pub fn update_mouse_position(&mut self, x: f32, y: f32) {
        self.mouse_x = x;
        self.mouse_y = y;
    }

    pub fn reset_one_shot_inputs(&mut self) {
        self.switch_model = false;
        self.weapon_switch = None;
    }

    pub fn take_weapon_switch(&mut self) -> Option<Weapon> {
        self.weapon_switch.take()
    }
}
