use glam::{Mat3, Mat4, Vec3, Vec4};
use crate::md3::Tag;

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

