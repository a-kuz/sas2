use glam::{Mat3, Mat4, Vec3, Vec4};
use crate::md3::Tag;

#[derive(Clone, Copy)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    pub fn from_view_proj(view_proj: Mat4) -> Self {
        let m = view_proj.to_cols_array_2d();
        let mut planes = [Vec4::ZERO; 6];
        
        planes[0] = Vec4::new(
            m[0][3] + m[0][0],
            m[1][3] + m[1][0],
            m[2][3] + m[2][0],
            m[3][3] + m[3][0],
        ).normalize();
        
        planes[1] = Vec4::new(
            m[0][3] - m[0][0],
            m[1][3] - m[1][0],
            m[2][3] - m[2][0],
            m[3][3] - m[3][0],
        ).normalize();
        
        planes[2] = Vec4::new(
            m[0][3] + m[0][1],
            m[1][3] + m[1][1],
            m[2][3] + m[2][1],
            m[3][3] + m[3][1],
        ).normalize();
        
        planes[3] = Vec4::new(
            m[0][3] - m[0][1],
            m[1][3] - m[1][1],
            m[2][3] - m[2][1],
            m[3][3] - m[3][1],
        ).normalize();
        
        planes[4] = Vec4::new(
            m[0][3] + m[0][2],
            m[1][3] + m[1][2],
            m[2][3] + m[2][2],
            m[3][3] + m[3][2],
        ).normalize();
        
        planes[5] = Vec4::new(
            m[0][3] - m[0][2],
            m[1][3] - m[1][2],
            m[2][3] - m[2][2],
            m[3][3] - m[3][2],
        ).normalize();
        
        Self { planes }
    }
    
    pub fn contains_point(&self, point: Vec3) -> bool {
        let p = Vec4::new(point.x, point.y, point.z, 1.0);
        for plane in &self.planes {
            if plane.dot(p) < 0.0 {
                return false;
            }
        }
        true
    }
    
    pub fn contains_sphere(&self, center: Vec3, radius: f32) -> bool {
        let p = Vec4::new(center.x, center.y, center.z, 1.0);
        for plane in &self.planes {
            let distance = plane.dot(p);
            if distance < -radius {
                return false;
            }
        }
        true
    }
    
    pub fn estimate_visibility_time(&self, start_pos: Vec3, velocity: Vec3, radius: f32) -> f32 {
        if self.contains_sphere(start_pos, radius) {
            let mut min_exit_time = f32::INFINITY;
            
            for plane in &self.planes {
                let normal = Vec3::new(plane.x, plane.y, plane.z);
                let speed_along_normal = normal.dot(velocity);
                
                if speed_along_normal > 0.0 {
                    let p = Vec4::new(start_pos.x, start_pos.y, start_pos.z, 1.0);
                    let dist = plane.dot(p);
                    let time_to_exit = (-radius - dist) / speed_along_normal;
                    if time_to_exit > 0.0 {
                        min_exit_time = min_exit_time.min(time_to_exit);
                    }
                }
            }
            
            if min_exit_time.is_finite() {
                return min_exit_time.max(0.1);
            }
        } else {
            let mut min_enter_time = f32::INFINITY;
            
            for plane in &self.planes {
                let normal = Vec3::new(plane.x, plane.y, plane.z);
                let speed_along_normal = normal.dot(velocity);
                
                if speed_along_normal < 0.0 {
                    let p = Vec4::new(start_pos.x, start_pos.y, start_pos.z, 1.0);
                    let dist = plane.dot(p);
                    if dist < -radius {
                        let time_to_enter = (-radius - dist) / -speed_along_normal;
                        if time_to_enter > 0.0 {
                            min_enter_time = min_enter_time.min(time_to_enter);
                        }
                    }
                }
            }
            
            if min_enter_time.is_finite() {
                let mut min_exit_time = f32::INFINITY;
                let enter_pos = start_pos + velocity * min_enter_time;
                
                for plane in &self.planes {
                    let normal = Vec3::new(plane.x, plane.y, plane.z);
                    let speed_along_normal = normal.dot(velocity);
                    
                    if speed_along_normal > 0.0 {
                        let p = Vec4::new(enter_pos.x, enter_pos.y, enter_pos.z, 1.0);
                        let dist = plane.dot(p);
                        let time_to_exit = (-radius - dist) / speed_along_normal;
                        if time_to_exit > 0.0 {
                            min_exit_time = min_exit_time.min(time_to_exit);
                        }
                    }
                }
                
                if min_exit_time.is_finite() {
                    return (min_enter_time + min_exit_time).max(0.1);
                }
            }
        }
        
        10.0
    }
}

#[derive(Clone, Copy)]
pub struct Orientation {
    pub origin: Vec3,
    pub axis: [Vec3; 3],
}

pub fn axis_from_mat3(m: Mat3) -> [Vec3; 3] {
    let cols = m.to_cols_array();
    [
        Vec3::new(cols[0], cols[1], cols[2]),
        Vec3::new(cols[3], cols[4], cols[5]),
        Vec3::new(cols[6], cols[7], cols[8]),
    ]
}

pub fn orientation_to_mat4(orientation: &Orientation) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(orientation.axis[0].x, orientation.axis[0].y, orientation.axis[0].z, 0.0),
        Vec4::new(orientation.axis[1].x, orientation.axis[1].y, orientation.axis[1].z, 0.0),
        Vec4::new(orientation.axis[2].x, orientation.axis[2].y, orientation.axis[2].z, 0.0),
        Vec4::new(orientation.origin.x, orientation.origin.y, orientation.origin.z, 1.0),
    )
}

pub fn attach_rotated_entity(parent: &Orientation, tag: &Tag) -> Orientation {
    let tag_pos = Vec3::new(tag.position[0], tag.position[1], tag.position[2]);

    let origin =
        parent.origin +
        parent.axis[0] * tag_pos.x +
        parent.axis[1] * tag_pos.y +
        parent.axis[2] * tag_pos.z;

    let mut axis = [Vec3::ZERO; 3];
    for i in 0..3 {
        let mut components = [0.0; 3];
        for j in 0..3 {
            for k in 0..3 {
                components[j] += tag.axis[i][k] * match j {
                    0 => parent.axis[k].x,
                    1 => parent.axis[k].y,
                    2 => parent.axis[k].z,
                    _ => 0.0,
                };
            }
        }
        axis[i] = Vec3::new(components[0], components[1], components[2]);
    }

    Orientation { origin, axis }
}

