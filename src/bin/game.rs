use std::sync::Arc;
use std::time::Instant;
use std::collections::{HashMap, HashSet};

use glam::{Mat3, Mat4, Vec3};
use pollster::FutureExt;
use wgpu::Texture;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use sas2::engine::anim::{AnimConfig, AnimRange};
use sas2::engine::loader::{
    load_textures_for_model_static,
    load_weapon_textures_static,
    load_rocket_textures_static,
    load_md3_textures_guess_static,
};
use sas2::engine::math::{axis_from_mat3, attach_rotated_entity, orientation_to_mat4, Orientation, Frustum};
use sas2::engine::md3::MD3Model;
use sas2::engine::renderer::{MD3Renderer, WgpuRenderer};
use sas2::render::TextRenderer;

use sas2::game::world::World;
use sas2::game::camera::Camera;
use sas2::game::lighting::{LightingParams, Light};
// use sas2::game::player::Player;
use sas2::game::map::ItemType;

struct PlayerModel {
    lower: Option<MD3Model>,
    upper: Option<MD3Model>,
    head: Option<MD3Model>,
    weapon: Option<MD3Model>,
    lower_textures: Vec<Option<String>>,
    upper_textures: Vec<Option<String>>,
    head_textures: Vec<Option<String>>,
    weapon_textures: Vec<Option<String>>,
    anim_config: Option<AnimConfig>,
}

impl PlayerModel {
    fn new() -> Self {
        Self {
            lower: None,
            upper: None,
            head: None,
            weapon: None,
            lower_textures: Vec::new(),
            upper_textures: Vec::new(),
            head_textures: Vec::new(),
            weapon_textures: Vec::new(),
            anim_config: None,
        }
    }
}

struct StaticModel {
    model: MD3Model,
    textures: Vec<Option<String>>,
    scale: f32,
}

struct GameApp {
    window: Option<Arc<Window>>,
    wgpu_renderer: Option<WgpuRenderer>,
    md3_renderer: Option<MD3Renderer>,
    crosshair_renderer: Option<sas2::engine::renderer::crosshair::Crosshair>,
    text_renderer: Option<TextRenderer>,
    player_model: PlayerModel,
    player2_model: PlayerModel,
    rocket_model: Option<MD3Model>,
    rocket_textures: Vec<Option<String>>,
    item_models: HashMap<ItemType, StaticModel>,
    teleporter_marker: Option<StaticModel>,
    jumppad_marker: Option<StaticModel>,
    depth_texture: Option<Texture>,
    depth_view: Option<wgpu::TextureView>,
    start_time: Instant,
    last_frame_time: Instant,
    last_fps_update: Instant,
    frame_count: u32,
    fps: f32,
    last_debug_log: Instant,
    
    world: World,
    local_player_id: u32,
    
    move_left: bool,
    move_right: bool,
    jump_pressed: bool,
    crouch_pressed: bool,
    shoot_pressed: bool,
    is_shooting: bool,
    shoot_anim_start_time: f32,
    
    player2_gesture_start_time: f32,
    player2_is_gesturing: bool,
    player2_next_gesture_time: f32,
    
    camera: Camera,
    camera_move_z_neg: bool,
    camera_move_z_pos: bool,
    camera_pitch_up: bool,
    camera_pitch_down: bool,
    camera_yaw_left: bool,
    camera_yaw_right: bool,

    aim_x: f32,
    aim_y: f32,
    last_mouse_pos: (f32, f32),
    
    current_legs_yaw: f32,
    player2_legs_yaw: f32,
    
    available_models: Vec<&'static str>,
    current_model_index: usize,
    shift_pressed: bool,
}

impl GameApp {
    fn item_model_path(item_type: ItemType) -> &'static str {
        match item_type {
            ItemType::Health25 => "q3-resources/models/powerups/health/medium_cross.md3",
            ItemType::Health50 => "q3-resources/models/powerups/health/large_cross.md3",
            ItemType::Health100 => "q3-resources/models/powerups/health/mega_cross.md3",
            ItemType::Armor50 => "q3-resources/models/powerups/armor/armor_yel.md3",
            ItemType::Armor100 => "q3-resources/models/powerups/armor/armor_red.md3",
            ItemType::Shotgun => "q3-resources/models/weapons2/shotgun/shotgun.md3",
            ItemType::GrenadeLauncher => "q3-resources/models/weapons2/grenadel/grenadel.md3",
            ItemType::RocketLauncher => "q3-resources/models/weapons2/rocketl/rocketl.md3",
            ItemType::LightningGun => "q3-resources/models/weapons2/lightning/lightning.md3",
            ItemType::Railgun => "q3-resources/models/weapons2/railgun/railgun.md3",
            ItemType::Plasmagun => "q3-resources/models/weapons2/plasma/plasma.md3",
            ItemType::BFG => "q3-resources/models/weapons2/bfg/bfg.md3",
            ItemType::Quad => "q3-resources/models/powerups/instant/quad.md3",
            ItemType::Regen => "q3-resources/models/powerups/instant/regen.md3",
            ItemType::Battle => "q3-resources/models/powerups/instant/enviro.md3",
            ItemType::Flight => "q3-resources/models/powerups/instant/flight.md3",
            ItemType::Haste => "q3-resources/models/powerups/instant/haste.md3",
            ItemType::Invis => "q3-resources/models/powerups/instant/invis.md3",
        }
    }

    fn item_model_scale(item_type: ItemType) -> f32 {
        match item_type {
            ItemType::Shotgun
            | ItemType::GrenadeLauncher
            | ItemType::RocketLauncher
            | ItemType::LightningGun
            | ItemType::Railgun
            | ItemType::Plasmagun
            | ItemType::BFG => 1.35,
            _ => 1.0,
        }
    }

    fn load_static_model(
        wgpu_renderer: &mut WgpuRenderer,
        md3_renderer: &mut MD3Renderer,
        model_path: &str,
        scale: f32,
    ) -> Option<StaticModel> {
        let model = MD3Model::load(model_path).ok();
        if model.is_none() {
            println!("Failed to load static model: {}", model_path);
            return None;
        }
        let model = model.unwrap();
        let textures = load_md3_textures_guess_static(wgpu_renderer, md3_renderer, &model, model_path);
        println!("Loaded static model: {} with {} textures", model_path, textures.len());
        Some(StaticModel { model, textures, scale })
    }

    fn new() -> Self {
        let now = Instant::now();
        let mut world = World::new();
        
        if let Ok(map) = sas2::game::map::Map::load_from_file("0-arena") {
            println!("Loaded map: {}x{} tiles", map.width, map.height);
            world.map = map;
        } else {
            println!("Failed to load map, using default");
        }
        
        let local_player_id = world.add_player();
        
        Self {
            window: None,
            wgpu_renderer: None,
            md3_renderer: None,
            crosshair_renderer: None,
            text_renderer: None,
            player_model: PlayerModel::new(),
            player2_model: PlayerModel::new(),
            rocket_model: None,
            rocket_textures: Vec::new(),
            item_models: HashMap::new(),
            teleporter_marker: None,
            jumppad_marker: None,
            depth_texture: None,
            depth_view: None,
            start_time: now,
            last_frame_time: now,
            last_fps_update: now,
            frame_count: 0,
            fps: 0.0,
            last_debug_log: now,
            
            world,
            local_player_id,
            
            move_left: false,
            move_right: false,
            jump_pressed: false,
            crouch_pressed: false,
            shoot_pressed: false,
            is_shooting: false,
            shoot_anim_start_time: 0.0,
            
            player2_gesture_start_time: 0.0,
            player2_is_gesturing: false,
            player2_next_gesture_time: 5.0,
            
            camera: Camera::new(),
            camera_move_z_neg: false,
            camera_move_z_pos: false,
            camera_pitch_up: false,
            camera_pitch_down: false,
            camera_yaw_left: false,
            camera_yaw_right: false,
            
            aim_x: 1.0,
            aim_y: 0.0,
            last_mouse_pos: (0.0, 0.0),
            
            current_legs_yaw: 0.0,
            player2_legs_yaw: 0.0,
            
            available_models: vec![
                "sarge", "orbb", "grunt", "major", "visor", "bones", "crash", "slash",
                "ranger", "doom", "keel", "hunter", "mynx", "razor", "uriel", "xaero",
                "sorlag", "tankjr", "anarki", "biker", "bitterman", "klesk", "lucy"
            ],
            current_model_index: 0,
            shift_pressed: false,
        }
    }

    fn create_depth(&mut self) {
        if let Some(ref wgpu_renderer) = self.wgpu_renderer {
            let (width, height) = wgpu_renderer.get_surface_size();
            let depth_texture = wgpu_renderer
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("Depth Texture"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth24PlusStencil8,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
            let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.depth_texture = Some(depth_texture);
            self.depth_view = Some(depth_view);
        }
    }

    fn load_model_part(paths: &[&str]) -> Option<MD3Model> {
        paths
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .and_then(|path| {
                println!("Loading model: {}", path);
                MD3Model::load(path).ok()
            })
    }

    fn update_fps_counter(&mut self, now: Instant) {
        self.frame_count += 1;
        let fps_elapsed = now.duration_since(self.last_fps_update).as_secs_f32();
        if fps_elapsed >= 0.5 {
            self.fps = self.frame_count as f32 / fps_elapsed;
            self.frame_count = 0;
            self.last_fps_update = now;
            if let Some(ref window) = self.window {
                let player_x = self.world.players.get(self.local_player_id as usize).map(|p| p.x).unwrap_or(0.0);
                window.set_title(&format!(
                    "SAS2 MVP | FPS: {:.0} | X: {:.1}",
                    self.fps, player_x
                ));
            }
        }
    }

    fn frame_for_anim(anim: &AnimRange, time: f32, model: &MD3Model) -> usize {
        let frames_passed = (time * anim.fps as f32).floor() as usize;
        let max_index = model.header.num_bone_frames as usize;
        if max_index == 0 {
            return 0;
        }
        if anim.looping_frames == 0 {
            let last = anim.num_frames.saturating_sub(1);
            let frame = anim.first_frame + frames_passed.min(last);
            return frame.min(max_index - 1);
        }
        let loop_len = anim.looping_frames.min(anim.num_frames).max(1);
        if frames_passed < anim.num_frames {
            let frame = anim.first_frame + frames_passed;
            return frame.min(max_index - 1);
        }
        let loop_start = anim.first_frame + anim.num_frames.saturating_sub(loop_len);
        let loop_index = (frames_passed - anim.num_frames) % loop_len;
        let frame = loop_start + loop_index;
        frame.min(max_index - 1)
    }

    fn calculate_legs_frame(
        anim_config: &Option<AnimConfig>,
        is_moving: bool,
        is_moving_backward: bool,
        animation_time: f32,
        model: &MD3Model,
        state: sas2::game::player::PlayerState,
        _is_crouching: bool,
    ) -> usize {
        use sas2::game::player::PlayerState;
        
        if let Some(ref config) = anim_config {
            let anim = match state {
                PlayerState::Air => &config.legs_jump,
                PlayerState::Crouching => {
                    if is_moving {
                        &config.legs_walkcr
                    } else {
                        &config.legs_idlecr
                    }
                }
                PlayerState::Ground => {
                    if is_moving_backward {
                        &config.legs_back
                    } else if is_moving {
                        &config.legs_run
                    } else {
                        &config.legs_idle
                    }
                }
            };
            return Self::frame_for_anim(anim, animation_time, model);
        } else {
            0
        }
    }

    fn calculate_torso_frame(
        anim_config: &Option<AnimConfig>,
        elapsed_time: f32,
        model: &MD3Model,
        is_shooting: bool,
        shoot_anim_time: f32,
    ) -> usize {
        if let Some(ref config) = anim_config {
            let anim = if is_shooting {
                &config.torso_attack
            } else {
                &config.torso_stand
            };
            let time = if is_shooting { shoot_anim_time } else { elapsed_time };
            return Self::frame_for_anim(anim, time, model);
        } else {
            0
        }
    }

    fn calculate_torso_frame_with_gesture(
        anim_config: &Option<AnimConfig>,
        elapsed_time: f32,
        model: &MD3Model,
        is_gesturing: bool,
        gesture_anim_time: f32,
    ) -> usize {
        if let Some(ref config) = anim_config {
            let anim = if is_gesturing {
                &config.torso_gesture
            } else {
                &config.torso_stand
            };
            let time = if is_gesturing { gesture_anim_time } else { elapsed_time };
            return Self::frame_for_anim(anim, time, model);
        } else {
            0
        }
    }

    fn find_tag<'a>(tags: &'a [sas2::engine::md3::Tag], name: &str) -> Option<&'a sas2::engine::md3::Tag> {
        tags.iter().find(|t| {
            let tag_name = std::str::from_utf8(&t.name).unwrap_or("");
            tag_name.trim_end_matches('\0') == name
        })
    }

    fn switch_player_model(&mut self) {
        self.current_model_index = (self.current_model_index + 1) % self.available_models.len();
        let model_name = self.available_models[self.current_model_index];
        
        println!("Switching to model: {}", model_name);
        
        if let Some(ref mut md3_renderer) = self.md3_renderer.as_mut() {
            md3_renderer.clear_model_cache();
        }
        
        self.player_model.lower = None;
        self.player_model.upper = None;
        self.player_model.head = None;
        self.player_model.lower_textures.clear();
        self.player_model.upper_textures.clear();
        self.player_model.head_textures.clear();
        
        self.player_model.lower = Self::load_model_part(&[
            &format!("q3-resources/models/players/{}/lower.md3", model_name),
            &format!("../q3-resources/models/players/{}/lower.md3", model_name),
        ]);
        self.player_model.upper = Self::load_model_part(&[
            &format!("q3-resources/models/players/{}/upper.md3", model_name),
            &format!("../q3-resources/models/players/{}/upper.md3", model_name),
        ]);
        self.player_model.head = Self::load_model_part(&[
            &format!("q3-resources/models/players/{}/head.md3", model_name),
            &format!("../q3-resources/models/players/{}/head.md3", model_name),
        ]);
        
        if self.player_model.lower.is_none() {
            println!("WARNING: Failed to load lower model for {}", model_name);
        }
        if self.player_model.upper.is_none() {
            println!("WARNING: Failed to load upper model for {}", model_name);
        }
        if self.player_model.head.is_none() {
            println!("WARNING: Failed to load head model for {}", model_name);
        }
        
        self.player_model.anim_config = AnimConfig::load(model_name).ok();
        
        if let (Some(ref mut wgpu_renderer), Some(ref mut md3_renderer)) = 
            (self.wgpu_renderer.as_mut(), self.md3_renderer.as_mut()) {
            
            if let Some(ref lower) = self.player_model.lower {
                self.player_model.lower_textures =
                    load_textures_for_model_static(wgpu_renderer, md3_renderer, lower, model_name, "lower");
            }
            if let Some(ref upper) = self.player_model.upper {
                self.player_model.upper_textures =
                    load_textures_for_model_static(wgpu_renderer, md3_renderer, upper, model_name, "upper");
            }
            if let Some(ref head) = self.player_model.head {
                self.player_model.head_textures =
                    load_textures_for_model_static(wgpu_renderer, md3_renderer, head, model_name, "head");
            }
        }
        
        if let Some(ref window) = self.window {
            window.set_title(&format!("SAS2 MVP | Model: {}", model_name));
        }
    }

    fn calculate_model_bottom_offset(lower_model: Option<&MD3Model>, frame: usize) -> f32 {
        if let Some(model) = lower_model {
            let min_z = model.get_min_z(frame);
            -min_z
        } else {
            0.0
        }
    }

    fn render_player<'a>(
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        md3_renderer: &mut MD3Renderer,
        surface_format: wgpu::TextureFormat,
        player_model: &'a PlayerModel,
        game_transform: Mat4,
        scale_mat: Mat4,
        lower_orientation: Orientation,
        lower_frame: usize,
        upper_frame: usize,
        view_proj: Mat4,
        camera_pos: Vec3,
        lights: &[(Vec3, Vec3, f32)],
        ambient: f32,
        include_weapon: bool,
        aim_angle: f32,
        flip_x: bool,
        current_legs_yaw: &mut f32,
        dt: f32,
    ) -> (Option<Orientation>, Vec<(&'a MD3Model, usize, &'a [Option<String>], Mat4)>) {
        let mut shadow_models = Vec::new();
        
        let pitch = if flip_x {
            std::f32::consts::PI - aim_angle
        } else {
            aim_angle
        };
        // Normalize pitch to -PI to PI
        let pitch = pitch.atan2(1.0).atan2(1.0) * 0.0 + pitch; // Just a dummy op, but I should normalize correctly.
        // Actually simpler:
        // Since we inverted aim_y in the input system (screen Y down = world Y down),
        // we need to negate aim_angle here to make rotations work correctly
        let pitch = if flip_x {
            let mut p = std::f32::consts::PI - (-aim_angle);
            while p > std::f32::consts::PI { p -= 2.0 * std::f32::consts::PI; }
            while p < -std::f32::consts::PI { p += 2.0 * std::f32::consts::PI; }
            p
        } else {
            -aim_angle  // Negate because we inverted Y in input
        };

        let effective_pitch = if flip_x { -pitch } else { pitch };
        
        let target_legs_yaw = if effective_pitch.abs() > 0.3 {
            let intensity = ((effective_pitch.abs() - 0.3) / 1.2).min(1.0);
            let raw_yaw = effective_pitch.signum() * intensity * 1.2;
            raw_yaw.clamp(-0.5, 0.5)
        } else {
            0.0
        };
        
        let legs_yaw_speed = 6.0;
        let yaw_diff = target_legs_yaw - *current_legs_yaw;
        let max_change = legs_yaw_speed * dt;
        *current_legs_yaw += yaw_diff.clamp(-max_change, max_change);
        
        let legs_yaw = *current_legs_yaw;
        let torso_yaw = legs_yaw * 0.5;
        let torso_roll_extra = -effective_pitch * 0.25;
        let torso_pitch = (pitch * 0.3).clamp(-0.6, 0.6);

        // Inside render_player, we work in MD3 coordinate system (Z-up)
        // The correction matrix is applied in game_transform outside this function
        // So here: Z is up, X is forward, Y is left
        // Yaw (turning) is around Z axis (vertical in MD3)
        let lower_rot = Mat3::from_rotation_z(legs_yaw);
        
        let lower_orientation_rotated = Orientation {
            origin: lower_orientation.origin,
            axis: {
                let base_mat = Mat3::from_cols(lower_orientation.axis[0], lower_orientation.axis[1], lower_orientation.axis[2]);
                let new_mat = base_mat * lower_rot;
                [new_mat.x_axis, new_mat.y_axis, new_mat.z_axis]
            }
        };

        let mut upper_orientation = lower_orientation_rotated;
        let mut head_orientation: Option<Orientation> = None;
        let mut weapon_orientation_result: Option<Orientation> = None;

        if let Some(ref lower) = player_model.lower {
            let md3_model_mat = scale_mat * orientation_to_mat4(&lower_orientation_rotated);
            let model_mat = game_transform * md3_model_mat;
            md3_renderer.render_model(
                encoder,
                view,
                depth_view,
                surface_format,
                lower,
                lower_frame,
                &player_model.lower_textures,
                model_mat,
                view_proj,
                camera_pos,
                lights,
                ambient,
                false,
            );
            shadow_models.push((lower, lower_frame, player_model.lower_textures.as_slice(), model_mat));

            if let Some(tags) = lower.tags.get(lower_frame) {
                if let Some(torso_tag) = Self::find_tag(tags, "tag_torso") {
                    upper_orientation = attach_rotated_entity(&lower_orientation_rotated, torso_tag);
                    
                    // Apply Torso Twist in MD3 coordinates
                    // torso_yaw around Z (vertical in MD3)
                    // torso_pitch around Y (left in MD3) - follows aim up/down
                    // torso_roll around X (forward in MD3)
                    let twist = Mat3::from_rotation_z(torso_yaw);
                    let pitch_rot = Mat3::from_rotation_y(torso_pitch);
                    let roll = Mat3::from_rotation_x(torso_roll_extra);
                    
                    let torso_local_rot = twist * pitch_rot * roll;
                    
                    let base_mat = Mat3::from_cols(upper_orientation.axis[0], upper_orientation.axis[1], upper_orientation.axis[2]);
                    let new_mat = base_mat * torso_local_rot;
                    upper_orientation.axis = [new_mat.x_axis, new_mat.y_axis, new_mat.z_axis];
                }
            }
        }

        if let Some(ref upper) = player_model.upper {
            let md3_model_mat = scale_mat * orientation_to_mat4(&upper_orientation);
            let model_mat = game_transform * md3_model_mat;
            md3_renderer.render_model(
                encoder,
                view,
                depth_view,
                surface_format,
                upper,
                upper_frame,
                &player_model.upper_textures,
                model_mat,
                view_proj,
                camera_pos,
                lights,
                ambient,
                false,
            );
            shadow_models.push((upper, upper_frame, player_model.upper_textures.as_slice(), model_mat));

            if let Some(tags) = upper.tags.get(upper_frame) {
                if let Some(head_tag) = Self::find_tag(tags, "tag_head") {
                    head_orientation = Some(attach_rotated_entity(&upper_orientation, head_tag));
                    
                    // Apply Head Rotation for aiming in MD3 coordinates
                    // In MD3: Z is up, X is forward, Y is left
                    // Pitch (looking up/down) rotates around Y axis
                    
                    let head_pitch = pitch.clamp(-1.2, 1.2);
                    let head_rot = Mat3::from_rotation_y(head_pitch);
                    
                    if let Some(ref mut orient) = head_orientation {
                         let base = Mat3::from_cols(orient.axis[0], orient.axis[1], orient.axis[2]);
                         let new_mat = base * head_rot;
                         orient.axis = [new_mat.x_axis, new_mat.y_axis, new_mat.z_axis];
                    }
                }
                if include_weapon {
                    if let Some(weapon_tag) = Self::find_tag(tags, "tag_weapon") {
                        weapon_orientation_result = Some(attach_rotated_entity(&upper_orientation, weapon_tag));
                        
                        // Apply Weapon Rotation (Pitch) in MD3 coordinates
                        // Rotate around Y axis for pitch
                        // Limit weapon pitch to avoid excessive rotation
                        let weapon_pitch = (pitch * 0.7).clamp(-1.0, 1.0);
                        let weapon_rot = Mat3::from_rotation_y(weapon_pitch);
                        
                        if let Some(ref mut orient) = weapon_orientation_result {
                             let base = Mat3::from_cols(orient.axis[0], orient.axis[1], orient.axis[2]);
                             let new_mat = base * weapon_rot;
                             orient.axis = [new_mat.x_axis, new_mat.y_axis, new_mat.z_axis];
                        }
                    }
                }
            }
        }

        if let (Some(ref head), Some(head_orient)) = (&player_model.head, head_orientation) {
            let md3_model_mat = scale_mat * orientation_to_mat4(&head_orient);
            let model_mat = game_transform * md3_model_mat;
            md3_renderer.render_model(
                encoder,
                view,
                depth_view,
                surface_format,
                head,
                0,
                &player_model.head_textures,
                model_mat,
                view_proj,
                camera_pos,
                lights,
                ambient,
                false,
            );
            shadow_models.push((head, 0, player_model.head_textures.as_slice(), model_mat));
        }

        if include_weapon {
            if let (Some(ref weapon), Some(weapon_orient)) = (&player_model.weapon, weapon_orientation_result) {
                let md3_model_mat = scale_mat * orientation_to_mat4(&weapon_orient);
                let model_mat = game_transform * md3_model_mat;
                md3_renderer.render_model(
                    encoder,
                    view,
                    depth_view,
                    surface_format,
                    weapon,
                    0,
                    &player_model.weapon_textures,
                    model_mat,
                    view_proj,
                    camera_pos,
                    lights,
                    ambient,
                    false,
                );
                shadow_models.push((weapon, 0, player_model.weapon_textures.as_slice(), model_mat));
            }
        }

        (weapon_orientation_result, shadow_models)
    }
}

impl ApplicationHandler for GameApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title("SAS2 MVP - WGPU")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        let mut wgpu_renderer = WgpuRenderer::new(window.clone()).block_on().unwrap();
        let mut md3_renderer =
            MD3Renderer::new(wgpu_renderer.device.clone(), wgpu_renderer.queue.clone());
        
        md3_renderer.load_map_tiles(&self.world.map);
        
        let crosshair_renderer = sas2::engine::renderer::crosshair::Crosshair::new(
            &wgpu_renderer.device,
            wgpu_renderer.surface_config.format,
        );
        let text_renderer = TextRenderer::new(
            wgpu_renderer.device.clone(),
            wgpu_renderer.queue.clone(),
            wgpu_renderer.surface_config.format,
        );

        self.player_model.lower = Self::load_model_part(&[
            "q3-resources/models/players/sarge/lower.md3",
            "../q3-resources/models/players/sarge/lower.md3",
        ]);
        if let Some(ref lower) = self.player_model.lower {
            let (min_x, max_x, min_y, max_y, min_z, max_z) = lower.get_bounds(0);
            let height = max_z - min_z;
            let width = max_y - min_y;
            println!("Lower model bounds - Height (Z): {:.2}, Width (Y): {:.2}, Depth (X): {:.2}", height, width, max_x - min_x);
        }
        
        self.player_model.upper = Self::load_model_part(&[
            "q3-resources/models/players/sarge/upper.md3",
            "../q3-resources/models/players/sarge/upper.md3",
        ]);
        if let Some(ref upper) = self.player_model.upper {
            let (min_x, max_x, min_y, max_y, min_z, max_z) = upper.get_bounds(0);
            let height = max_z - min_z;
            let width = max_y - min_y;
            println!("Upper model bounds - Height (Z): {:.2}, Width (Y): {:.2}, Depth (X): {:.2}", height, width, max_x - min_x);
        }
        
        self.player_model.head = Self::load_model_part(&[
            "q3-resources/models/players/sarge/head.md3",
            "../q3-resources/models/players/sarge/head.md3",
        ]);
        if let Some(ref head) = self.player_model.head {
            let (min_x, max_x, min_y, max_y, min_z, max_z) = head.get_bounds(0);
            let height = max_z - min_z;
            let width = max_y - min_y;
            println!("Head model bounds - Height (Z): {:.2}, Width (Y): {:.2}, Depth (X): {:.2}", height, width, max_x - min_x);
        }
        
        if let (Some(ref lower), Some(ref upper), Some(ref head)) = (&self.player_model.lower, &self.player_model.upper, &self.player_model.head) {
            let (lower_min_x, lower_max_x, lower_min_y, lower_max_y, lower_min_z, lower_max_z) = lower.get_bounds(0);
            let (upper_min_x, upper_max_x, upper_min_y, upper_max_y, upper_min_z, upper_max_z) = upper.get_bounds(0);
            let (head_min_x, head_max_x, head_min_y, head_max_y, head_min_z, head_max_z) = head.get_bounds(0);
            
            let total_min_z = lower_min_z.min(upper_min_z).min(head_min_z);
            let total_max_z = lower_max_z.max(upper_max_z).max(head_max_z);
            let total_min_y = lower_min_y.min(upper_min_y).min(head_min_y);
            let total_max_y = lower_max_y.max(upper_max_y).max(head_max_y);
            
            let total_height = total_max_z - total_min_z;
            let total_width = total_max_y - total_min_y;
            println!("Total player model - Height: {:.2}, Width: {:.2}", total_height, total_width);
        }
        self.player_model.weapon = Self::load_model_part(&[
            "q3-resources/models/weapons2/rocketl/rocketl.md3",
            "../q3-resources/models/weapons2/rocketl/rocketl.md3",
        ]);

        self.player2_model.lower = Self::load_model_part(&[
            "q3-resources/models/players/orbb/lower.md3",
            "../q3-resources/models/players/orbb/lower.md3",
        ]);
        self.player2_model.upper = Self::load_model_part(&[
            "q3-resources/models/players/orbb/upper.md3",
            "../q3-resources/models/players/orbb/upper.md3",
        ]);
        self.player2_model.head = Self::load_model_part(&[
            "q3-resources/models/players/orbb/head.md3",
            "../q3-resources/models/players/orbb/head.md3",
        ]);

        self.rocket_model = Self::load_model_part(&[
            "q3-resources/models/ammo/rocket/rocket.md3",
            "../q3-resources/models/ammo/rocket/rocket.md3",
        ]);

        self.player_model.anim_config = AnimConfig::load("sarge").ok();
        self.player2_model.anim_config = AnimConfig::load("orbb").ok();

        let surface_format = wgpu_renderer.surface_config.format;
        md3_renderer.create_pipeline(surface_format);

        if let Some(ref lower) = self.player_model.lower {
            self.player_model.lower_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, lower, "sarge", "lower");
        }
        if let Some(ref upper) = self.player_model.upper {
            self.player_model.upper_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, upper, "sarge", "upper");
        }
        if let Some(ref head) = self.player_model.head {
            self.player_model.head_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, head, "sarge", "head");
        }
        if let Some(ref weapon) = self.player_model.weapon {
            self.player_model.weapon_textures =
                load_weapon_textures_static(&mut wgpu_renderer, &mut md3_renderer, weapon);
        }

        if let Some(ref lower) = self.player2_model.lower {
            self.player2_model.lower_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, lower, "orbb", "lower");
        }
        if let Some(ref upper) = self.player2_model.upper {
            self.player2_model.upper_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, upper, "orbb", "upper");
        }
        if let Some(ref head) = self.player2_model.head {
            self.player2_model.head_textures =
                load_textures_for_model_static(&mut wgpu_renderer, &mut md3_renderer, head, "orbb", "head");
        }

        if let Some(ref rocket) = self.rocket_model {
            self.rocket_textures =
                load_rocket_textures_static(&mut wgpu_renderer, &mut md3_renderer, rocket);
        }

        let mut unique_item_types = HashSet::new();
        for item in &self.world.map.items {
            unique_item_types.insert(item.item_type);
        }
        for item_type in unique_item_types {
            let model_path = Self::item_model_path(item_type);
            let scale = Self::item_model_scale(item_type);
            if let Some(model) = Self::load_static_model(&mut wgpu_renderer, &mut md3_renderer, model_path, scale) {
                self.item_models.insert(item_type, model);
            }
        }

        self.teleporter_marker = Self::load_static_model(
            &mut wgpu_renderer,
            &mut md3_renderer,
            "q3-resources/models/powerups/holdable/teleporter.md3",
            2.0,
        );

        self.jumppad_marker = Self::load_static_model(
            &mut wgpu_renderer,
            &mut md3_renderer,
            "q3-resources/models/mapobjects/podium/podium4.md3",
            0.6,
        );

        self.window = Some(window.clone());
        self.wgpu_renderer = Some(wgpu_renderer);
        self.md3_renderer = Some(md3_renderer);
        self.crosshair_renderer = Some(crosshair_renderer);
        self.text_renderer = Some(text_renderer);
        self.create_depth();
        self.last_frame_time = Instant::now();

        window.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(ref mut wgpu_renderer) = self.wgpu_renderer {
                    wgpu_renderer.resize(size);
                    self.create_depth();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == winit::event::ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    match code {
                        KeyCode::KeyA => self.move_left = pressed,
                        KeyCode::KeyD => self.move_right = pressed,
                        KeyCode::KeyW => self.jump_pressed = pressed,
                        KeyCode::KeyS => self.crouch_pressed = pressed,
                        KeyCode::KeyR => self.camera_move_z_neg = pressed,
                        KeyCode::KeyF => self.camera_move_z_pos = pressed,
                        KeyCode::ArrowUp => self.camera_pitch_up = pressed,
                        KeyCode::ArrowDown => self.camera_pitch_down = pressed,
                        KeyCode::ArrowLeft => self.camera_yaw_left = pressed,
                        KeyCode::ArrowRight => self.camera_yaw_right = pressed,
                        KeyCode::Space => {
                            self.shoot_pressed = pressed;
                        }
                        KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                            self.shift_pressed = pressed;
                        }
                        KeyCode::F5 if pressed && self.shift_pressed => {
                            self.switch_player_model();
                        }
                        KeyCode::Escape if pressed => event_loop.exit(),
                        _ => {}
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // SAS-style aiming: mouse movement rotates aim direction
                let current_pos = (position.x as f32, position.y as f32);
                let mouse_delta = (
                    current_pos.0 - self.last_mouse_pos.0,
                    current_pos.1 - self.last_mouse_pos.1,
                );
                self.last_mouse_pos = current_pos;
                
                // Sensitivity settings
                let sensitivity = 20.0;
                let joystick_sensitivity = 0.01;
                let m_yaw = 0.022;
                let m_pitch = 0.022;
                
                // Accumulate mouse movement into aim vector
                // Invert Y because screen Y goes down but world Y goes up
                self.aim_x += mouse_delta.0 * joystick_sensitivity * sensitivity * m_yaw;
                self.aim_y -= mouse_delta.1 * joystick_sensitivity * sensitivity * m_pitch; // Note the minus!
                
                // Normalize to keep on unit circle
                let len = (self.aim_x * self.aim_x + self.aim_y * self.aim_y).sqrt();
                if len > 0.0 {
                    self.aim_x /= len;
                    self.aim_y /= len;
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(self.last_frame_time).as_secs_f32();
                self.last_frame_time = now;

                self.update_fps_counter(now);

                if let Some(player) = self.world.players.get(self.local_player_id as usize) {
                    self.camera.follow(player.x, player.y);
                }
                self.camera.update(dt, &self.world.map);

                let camera_speed = 20.0;
                if self.camera_move_z_neg {
                    self.camera.z -= camera_speed * dt;
                }
                if self.camera_move_z_pos {
                    self.camera.z += camera_speed * dt;
                }

                let angle_speed = 1.5;
                if self.camera_pitch_up {
                    self.camera.pitch += angle_speed * dt;
                }
                if self.camera_pitch_down {
                    self.camera.pitch -= angle_speed * dt;
                }
                if self.camera_yaw_left {
                    self.camera.yaw -= angle_speed * dt;
                }
                if self.camera_yaw_right {
                    self.camera.yaw += angle_speed * dt;
                }
                self.camera.pitch = self.camera.pitch.clamp(-1.5, 1.5);
                self.camera.yaw = self.camera.yaw.clamp(-1.5, 1.5);

                // Update World
                let (width, height) = if let Some(ref wgpu_renderer) = self.wgpu_renderer {
                    wgpu_renderer.get_viewport_size()
                } else {
                    return;
                };
                let aspect = width as f32 / height as f32;
                let (view_proj, _camera_pos) = self.camera.get_view_proj(aspect);
                let frustum = Frustum::from_view_proj(view_proj);

                if let Some(player) = self.world.players.get_mut(self.local_player_id as usize) {
                    let aim_angle = self.aim_y.atan2(self.aim_x);
                    
                    player.update(dt, self.move_left, self.move_right, self.jump_pressed, self.crouch_pressed, &mut self.world.map, aim_angle);
                }
                
                self.world.update(dt, &frustum);

                let now_debug = Instant::now();
                if now_debug.duration_since(self.last_debug_log).as_secs_f32() >= 1.0 {
                    if let Some(player) = self.world.players.get(self.local_player_id as usize) {
                        println!("=== DEBUG: Player pos=({:.2}, {:.2}), Teleporters count={}", 
                            player.x, player.y, self.world.map.teleporters.len());
                        for (i, tp) in self.world.map.teleporters.iter().enumerate() {
                            let center_x = tp.x + tp.width * 0.5;
                            let center_y = tp.y + tp.height * 0.5;
                            let dist = ((player.x - center_x).powi(2) + (player.y - center_y).powi(2)).sqrt();
                            println!("  Teleporter[{}]: bottom_left=({:.2}, {:.2}) center=({:.2}, {:.2}) size=({:.2}, {:.2}) dest=({:.2}, {:.2}), dist_to_player={:.2}, marker={}", 
                                i, tp.x, tp.y, center_x, center_y, tp.width, tp.height, tp.dest_x, tp.dest_y, dist,
                                if self.teleporter_marker.is_some() { "loaded" } else { "NOT LOADED" });
                        }
                    }
                    self.last_debug_log = now_debug;
                }

                // Rendering
                let player = match self.world.players.get(self.local_player_id as usize) {
                    Some(p) => p,
                    None => return,
                };
                let player_x = player.x;
                let player_y = player.y;
                let player_aim_angle = player.aim_angle;
                // Calculate facing from aim_angle
                let normalized_angle = if player.aim_angle > std::f32::consts::PI {
                    player.aim_angle - 2.0 * std::f32::consts::PI
                } else {
                    player.aim_angle
                };
                let player_facing_right = normalized_angle.abs() < std::f32::consts::FRAC_PI_2;

                let player_is_moving = player.is_moving;
                let player_is_moving_backward = player.is_moving_backward;
                let player_animation_time = player.animation_time;
                let player_state = player.state;
                let player_is_crouching = player.is_crouching;

                let elapsed_time = self.start_time.elapsed().as_secs_f32();
                let lower_frame = self.player_model.lower.as_ref()
                    .map(|lower| Self::calculate_legs_frame(
                        &self.player_model.anim_config,
                        player_is_moving,
                        player_is_moving_backward,
                        player_animation_time,
                        lower,
                        player_state,
                        player_is_crouching
                    ))
                    .unwrap_or(0);

                let shoot_anim_time = elapsed_time - self.shoot_anim_start_time;
                if self.is_shooting {
                    if let Some(ref config) = self.player_model.anim_config {
                        let anim_duration = config.torso_attack.num_frames as f32 / config.torso_attack.fps as f32;
                        if shoot_anim_time >= anim_duration {
                            self.is_shooting = false;
                        }
                    }
                }

                let upper_frame = self.player_model.upper.as_ref()
                    .map(|upper| Self::calculate_torso_frame(
                        &self.player_model.anim_config,
                        elapsed_time,
                        upper,
                        self.is_shooting,
                        shoot_anim_time
                    ))
                    .unwrap_or(0);

                if elapsed_time >= self.player2_next_gesture_time && !self.player2_is_gesturing {
                    self.player2_is_gesturing = true;
                    self.player2_gesture_start_time = elapsed_time;
                    self.player2_next_gesture_time = elapsed_time + 5.0 + (elapsed_time.sin() * 3.0).abs();
                }

                if self.player2_is_gesturing {
                    if let Some(ref config) = self.player2_model.anim_config {
                        let gesture_time = elapsed_time - self.player2_gesture_start_time;
                        let gesture_duration = config.torso_gesture.num_frames as f32 / config.torso_gesture.fps as f32;
                        if gesture_time >= gesture_duration {
                            self.player2_is_gesturing = false;
                        }
                    }
                }

                let player2_lower_frame = self.player2_model.lower.as_ref()
                    .map(|lower| Self::calculate_legs_frame(
                        &self.player2_model.anim_config,
                        false,
                        false,
                        elapsed_time,
                        lower,
                        sas2::game::player::PlayerState::Ground,
                        false
                    ))
                    .unwrap_or(0);

                let player2_gesture_time = elapsed_time - self.player2_gesture_start_time;
                let player2_upper_frame = self.player2_model.upper.as_ref()
                    .map(|upper| Self::calculate_torso_frame_with_gesture(
                        &self.player2_model.anim_config,
                        elapsed_time,
                        upper,
                        self.player2_is_gesturing,
                        player2_gesture_time
                    ))
                    .unwrap_or(0);

                let player_model = &self.player_model;
                let player2_model = &self.player2_model;
                let rocket_model = self.rocket_model.as_ref();

                let (wgpu_renderer, md3_renderer) =
                    match (self.wgpu_renderer.as_mut(), self.md3_renderer.as_mut()) {
                        (Some(w), Some(m)) => (w, m),
                        _ => return,
                    };

                let frame = match wgpu_renderer.begin_frame() {
                    Some(f) => f,
                    None => {
                        if let Some(ref window) = self.window {
                            window.request_redraw();
                        }
                        return;
                    }
                };
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    wgpu_renderer
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Game Encoder"),
                        });

                let depth_view = self.depth_view.as_ref().unwrap();
                {
                    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Clear Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.05,
                                    g: 0.05,
                                    b: 0.08,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        ..Default::default()
                    });
                }

                let frame_start = Instant::now();
                
                let (width, height) = wgpu_renderer.get_viewport_size();
                let aspect = width as f32 / height as f32;

                let (view_proj, camera_pos) = self.camera.get_view_proj(aspect);
                let frustum = Frustum::from_view_proj(view_proj);

                // Lighting
                let lighting = if !self.world.map.lights.is_empty() {
                    LightingParams::from_map_lights(&self.world.map.lights)
                } else {
                    LightingParams::new()
                };
                let time = self.start_time.elapsed().as_secs_f32();
                
                let mut dynamic_lights = Vec::new();
                
                for rocket in &self.world.rockets {
                    if !rocket.is_visible(&frustum) {
                        continue;
                    }
                    
                    let flame_color = Vec3::new(3.5, 2.0, 0.8);
                    dynamic_lights.push(Light::with_randomized_flicker(
                        rocket.position,
                        flame_color,
                        250.0,
                        41.0,
                        4.3,
                    ));
                    
                    let flame_offset = if rocket.velocity.x > 0.0 { -20.0 } else { 20.0 };
                    let flame_pos = rocket.position + Vec3::new(flame_offset, 0.0, 0.0);
                    let flash_color = Vec3::new(4.0, 2.5, 1.0);
                    dynamic_lights.push(Light::with_randomized_flicker(
                        flame_pos,
                        flash_color,
                        150.0,
                        20.0,
                        0.4,
                    ));
                }
                
                let static_lights: Vec<(Vec3, Vec3, f32)> = lighting.lights.iter()
                    .map(|l| (l.position, l.get_color_at_time(time), l.radius))
                    .collect();
                
                let dynamic_lights_data: Vec<(Vec3, Vec3, f32)> = dynamic_lights.iter()
                    .map(|l| (l.position, l.get_color_at_time(time), l.radius))
                    .collect();
                
                let mut all_lights = static_lights.clone();
                all_lights.extend(dynamic_lights_data.iter().copied());

                let surface_format = wgpu_renderer.surface_config.format;

                md3_renderer.render_tiles(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    &all_lights,
                    lighting.ambient,
                    surface_format,
                );

                let md3_correction_items = Mat3::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                let item_spin = Mat3::from_rotation_y(time * 1.2);
                let item_rotation = Mat4::from_mat3(item_spin * md3_correction_items);

                for item in &self.world.map.items {
                    if !item.active {
                        continue;
                    }
                    let Some(model) = self.item_models.get(&item.item_type) else {
                        continue;
                    };

                    let bob = (time * 2.0).sin() * 6.0;
                    let translation = Mat4::from_translation(Vec3::new(item.x, item.y + bob, 50.0));
                    let scale_mat = Mat4::from_scale(Vec3::splat(model.scale));
                    let model_mat = translation * item_rotation * scale_mat;

                    md3_renderer.render_model(
                        &mut encoder,
                        &view,
                        depth_view,
                        surface_format,
                        &model.model,
                        0,
                        &model.textures,
                        model_mat,
                        view_proj,
                        camera_pos,
                        &all_lights,
                        lighting.ambient,
                        false,
                    );
                }

                if let Some(marker) = self.teleporter_marker.as_ref() {
                    let spin = Mat4::from_mat3(Mat3::from_rotation_y(time * 0.8) * md3_correction_items);
                    for tp in &self.world.map.teleporters {
                        let translation = Mat4::from_translation(Vec3::new(tp.x, tp.y, 50.0));
                        let scale_mat = Mat4::from_scale(Vec3::splat(marker.scale));
                        let model_mat = translation * spin * scale_mat;

                        md3_renderer.render_model(
                            &mut encoder,
                            &view,
                            depth_view,
                            surface_format,
                            &marker.model,
                            0,
                            &marker.textures,
                            model_mat,
                            view_proj,
                            camera_pos,
                            &all_lights,
                            lighting.ambient,
                            false,
                        );
                    }
                }

                if let Some(marker) = self.jumppad_marker.as_ref() {
                    let jumppad_rotation = Mat3::from_rotation_x(std::f32::consts::FRAC_PI_2) * md3_correction_items;
                    let spin = Mat4::from_mat3(Mat3::from_rotation_y(time * 0.8) * jumppad_rotation);
                    for jp in &self.world.map.jumppads {
                        let x = jp.x + jp.width * 0.5;
                        let y = jp.y;
                        let translation = Mat4::from_translation(Vec3::new(x, y, 50.0));
                        let scale_mat = Mat4::from_scale(Vec3::splat(marker.scale));
                        let model_mat = translation * spin * scale_mat;

                        md3_renderer.render_model(
                            &mut encoder,
                            &view,
                            depth_view,
                            surface_format,
                            &marker.model,
                            0,
                            &marker.textures,
                            model_mat,
                            view_proj,
                            camera_pos,
                            &all_lights,
                            lighting.ambient,
                            false,
                        );
                    }
                }

                let scale = 1.0;
                let scale_mat = Mat4::from_scale(Vec3::splat(scale));

                // Render Player
                
                let lower_orientation = Orientation {
                    origin: Vec3::ZERO,
                    axis: axis_from_mat3(Mat3::IDENTITY),
                };
                
                // Determine flip_x based on aiming
                // If aiming left (PI), flip_x = true.
                let flip_x = !player_facing_right;
                
                let player_model_yaw = player.model_yaw;
                
                // MD3 models use Z-up coordinate system (X=forward, Y=left, Z=up)
                // Our world uses Y-up coordinate system (X=right, Y=up, Z=forward)
                // We need to rotate the model -90° around X axis to convert Z-up to Y-up
                let md3_correction = Mat3::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                
                // Then rotate around Y axis (which is now vertical after correction) for facing direction
                let facing_rotation = Mat3::from_rotation_y(player_model_yaw);
                
                let combined_rotation = facing_rotation * md3_correction;
                
                let ground_y = self.world.map.ground_y;
                let model_bottom_offset = Self::calculate_model_bottom_offset(self.player_model.lower.as_ref(), lower_frame);
                let render_y = ground_y + model_bottom_offset + player_y;
                let game_translation = Mat4::from_translation(Vec3::new(player_x, render_y, 50.0));
                let game_rotation = Mat4::from_mat3(combined_rotation);
                let game_transform = game_translation * game_rotation;

                let (_weapon_orientation, mut shadow_models) = Self::render_player(
                    &mut encoder,
                    &view,
                    depth_view,
                    md3_renderer,
                    surface_format,
                    player_model,
                    game_transform,
                    Mat4::from_scale(Vec3::splat(1.0)),
                    lower_orientation,
                    lower_frame,
                    upper_frame,
                    view_proj,
                    camera_pos,
                    &all_lights,
                    lighting.ambient,
                    true,
                    player_aim_angle,
                    flip_x,
                    &mut self.current_legs_yaw,
                    dt,
                );


                // Render Player 2 (Static dummy for now, but should ideally come from World)
                // For MVP refactor, keeping it as static dummy
                let ground_y = self.world.map.ground_y;
                let player2_lower_frame = 0;
                let model_bottom_offset = Self::calculate_model_bottom_offset(self.player2_model.lower.as_ref(), player2_lower_frame);
                let player2_y = ground_y + model_bottom_offset;
                let player2_game_translation = Mat4::from_translation(Vec3::new(250.0, player2_y, 50.0));
                let md3_correction = Mat3::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                let facing_rotation = Mat3::from_rotation_y(std::f32::consts::PI);
                let player2_combined_rotation = facing_rotation * md3_correction;
                let player2_game_rotation = Mat4::from_mat3(player2_combined_rotation);
                let player2_game_transform = player2_game_translation * player2_game_rotation;

                let (_player2_weapon_orientation, player2_shadow_models) = Self::render_player(
                    &mut encoder,
                    &view,
                    depth_view,
                    md3_renderer,
                    surface_format,
                    player2_model,
                    player2_game_transform,
                    Mat4::from_scale(Vec3::splat(1.0)),
                    lower_orientation,
                    player2_lower_frame,
                    player2_upper_frame,
                    view_proj,
                    camera_pos,
                    &all_lights,
                    lighting.ambient,
                    false,
                    0.0,
                    true,
                    &mut self.player2_legs_yaw,
                    dt,
                );
                shadow_models.extend(player2_shadow_models);

                let should_shoot = self.shoot_pressed && !self.is_shooting;

                // Render Rockets
                if let Some(rocket_model) = rocket_model {
                    for rocket in &self.world.rockets {
                        if !rocket.active || !rocket.is_visible(&frustum) {
                            continue;
                        }
                        
                        let rocket_scale = 1.0;
                        let md3_correction = Mat3::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                        let facing_rotation = Mat3::from_rotation_y(
                            if rocket.velocity.x > 0.0 { 0.0 } else { std::f32::consts::PI }
                        );
                        let rocket_rotation = facing_rotation * md3_correction;
                        
                        let translation = Mat4::from_translation(rocket.position);
                        let rotation = Mat4::from_mat3(rocket_rotation);
                        let scale_mat = Mat4::from_scale(Vec3::splat(rocket_scale));
                        let model_mat = translation * rotation * scale_mat;
                        
                        md3_renderer.render_model(
                            &mut encoder,
                            &view,
                            depth_view,
                            surface_format,
                            rocket_model,
                            0,
                            &self.rocket_textures,
                            model_mat,
                            view_proj,
                            camera_pos,
                            &all_lights,
                            lighting.ambient,
                            false,
                        );
                    }
                }

                let smoke_particles: Vec<(Vec3, f32, f32)> = self.world.smoke_particles.iter()
                    .map(|p| (p.position, p.size, p.get_alpha()))
                    .collect();
                
                md3_renderer.render_particles(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    &smoke_particles,
                );

                let flame_particles: Vec<(Vec3, f32, u32)> = self.world.flame_particles.iter()
                    .map(|p| (p.position, p.size, p.texture_index))
                    .collect();
                
                md3_renderer.render_flames(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    camera_pos,
                    &flame_particles,
                );

                let shadow_volume_models: Vec<(&MD3Model, usize, Mat4)> = shadow_models.iter()
                    .map(|(model, frame, _textures, matrix)| (*model, *frame, *matrix))
                    .collect();

                md3_renderer.render_planar_shadows(
                    &mut encoder,
                    &view,
                    depth_view,
                    view_proj,
                    &shadow_volume_models,
                    &all_lights,
                );

                // md3_renderer.render_debug_lights(
                //     &mut encoder,
                //     &view,
                //     depth_view,
                //     view_proj,
                //     camera_pos,
                //     &all_lights,
                //     surface_format,
                // );

                // md3_renderer.render_debug_light_rays(
                //     &mut encoder,
                //     &view,
                //     depth_view,
                //     view_proj,
                //     &all_lights,
                //     surface_format,
                // );

                let render_time = frame_start.elapsed();
                
                wgpu_renderer.queue.submit(Some(encoder.finish()));
                
                if let Some(crosshair_renderer) = &self.crosshair_renderer {
                    const CROSSHAIR_DISTANCE: f32 = 4.0;
                    
                    let lower_frame = 0;
                    let model_bottom_offset = Self::calculate_model_bottom_offset(self.player_model.lower.as_ref(), lower_frame);
                    let player_center_y = ground_y + model_bottom_offset + player_y + 0.5;
                    let player_center = Vec3::new(player_x, player_center_y, 50.0);
                    
                    let crosshair_world_x = player_center.x + self.aim_x * CROSSHAIR_DISTANCE;
                    let crosshair_world_y = player_center.y + self.aim_y * CROSSHAIR_DISTANCE;
                    
                    let crosshair_world_pos = Vec3::new(crosshair_world_x, crosshair_world_y, 0.0);
                    let clip_pos = view_proj * glam::Vec4::new(crosshair_world_pos.x, crosshair_world_pos.y, crosshair_world_pos.z, 1.0);
                    let ndc = Vec3::new(clip_pos.x, clip_pos.y, clip_pos.z) / clip_pos.w;
                    let screen_x = (ndc.x * 0.5 + 0.5) * width as f32;
                    let screen_y = (1.0 - (ndc.y * 0.5 + 0.5)) * height as f32;
                    
                    let mut encoder = wgpu_renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Crosshair Encoder"),
                    });
                    
                    crosshair_renderer.render(
                        &mut encoder,
                        &view,
                        &wgpu_renderer.queue,
                        screen_x,
                        screen_y,
                        width,
                        height,
                    );
                    
                    wgpu_renderer.queue.submit(Some(encoder.finish()));
                }

                if let Some(ref text_renderer) = self.text_renderer {
                    let mut text_encoder = wgpu_renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Text Encoder"),
                    });

                    let ground_y = self.world.map.ground_y;
                    let lower_frame = 0;
                    let model_bottom_offset = Self::calculate_model_bottom_offset(self.player_model.lower.as_ref(), lower_frame);
                    let player_center_y = ground_y + model_bottom_offset + player_y + 0.5;
                    let text_world_pos = Vec3::new(player_x, player_center_y + 2.0, 50.0);
                    let clip_pos = view_proj * glam::Vec4::new(text_world_pos.x, text_world_pos.y, text_world_pos.z, 1.0);
                    if clip_pos.w > 0.0 {
                        let ndc = Vec3::new(clip_pos.x, clip_pos.y, clip_pos.z) / clip_pos.w;
                        if ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0 {
                            let screen_x = (ndc.x * 0.5 + 0.5) * width as f32;
                            let screen_y = (1.0 - (ndc.y * 0.5 + 0.5)) * height as f32;
                            
                            text_renderer.render_text(
                                &mut text_encoder,
                                &view,
                                "PLAYER",
                                screen_x,
                                screen_y,
                                32.0,
                                [1.0, 1.0, 0.0, 1.0],
                                width,
                                height,
                            );
                        }
                    }

                    wgpu_renderer.queue.submit(Some(text_encoder.finish()));
                }
                
                wgpu_renderer.end_frame(frame);
                
                if should_shoot {
                    if self.world.try_fire(self.local_player_id, player_aim_angle, &frustum) {
                        self.is_shooting = true;
                        self.shoot_anim_start_time = elapsed_time;
                    }
                }
                
                let total_time = frame_start.elapsed();
                if self.frame_count % 60 == 0 {
                    println!("Frame timing: render={:.2}ms, total={:.2}ms, submit={:.2}ms", 
                        render_time.as_secs_f64() * 1000.0,
                        total_time.as_secs_f64() * 1000.0,
                        (total_time - render_time).as_secs_f64() * 1000.0);
                }

                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = GameApp::new();
    event_loop.run_app(&mut app).unwrap();
}
