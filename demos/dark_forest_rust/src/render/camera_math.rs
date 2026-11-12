//! `camera_math` makes the renderer's view and projection matrices with the arithmetic of glam
//! 0.30's `Mat4::look_to_rh`, `Mat4::orthographic_rh` and `Mat4::perspective_infinite_reverse_rh`,
//! which glam 0.34 replaced by its `camera` module with other arithmetic: kept here so raising
//! glam to the shared crate's 0.34 changed no picture.
//!
//! Interface: `make_view_rh`, `make_orthographic_rh`, `make_perspective_infinite_reverse_rh`.
//!
//! Example: `let view: Mat4 = make_view_rh(eye_m, Vec3::NEG_Z, Vec3::Y);`

use glam::{Mat4, Vec3, Vec4};

/// `make_view_rh` gives a right-handed view matrix for a camera at `eye` facing `dir` with up
/// `up` (both unit vectors): view space has +x right, +y up and +z back.
///
/// Example: `let view: Mat4 = make_view_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);`
pub fn make_view_rh(eye: Vec3, dir: Vec3, up: Vec3) -> Mat4 {
    let f: Vec3 = dir;
    let s: Vec3 = f.cross(up).normalize();
    let u: Vec3 = s.cross(f);
    Mat4::from_cols(
        Vec4::new(s.x, u.x, -f.x, 0.0),
        Vec4::new(s.y, u.y, -f.y, 0.0),
        Vec4::new(s.z, u.z, -f.z, 0.0),
        Vec4::new(-eye.dot(s), -eye.dot(u), eye.dot(f), 1.0),
    )
}

/// `make_orthographic_rh` gives a right-handed orthographic projection of the box from `left` to
/// `right`, `bottom` to `top` and `near` to `far`, with depth from 0 to 1.
///
/// Example: `let projection: Mat4 = make_orthographic_rh(-10.0, 10.0, -10.0, 10.0, 0.0, 100.0);`
pub fn make_orthographic_rh(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> Mat4 {
    let rcp_width: f32 = 1.0 / (right - left);
    let rcp_height: f32 = 1.0 / (top - bottom);
    let r: f32 = 1.0 / (near - far);
    Mat4::from_cols(
        Vec4::new(rcp_width + rcp_width, 0.0, 0.0, 0.0),
        Vec4::new(0.0, rcp_height + rcp_height, 0.0, 0.0),
        Vec4::new(0.0, 0.0, r, 0.0),
        Vec4::new(
            -(left + right) * rcp_width,
            -(top + bottom) * rcp_height,
            r * near,
            1.0,
        ),
    )
}

/// `make_perspective_infinite_reverse_rh` gives a right-handed perspective projection with an
/// infinite far plane and reversed depth (1 at `z_near`, 0 at infinity), for a vertical field of
/// view `fov_y_rad` and width over height `aspect_ratio`.
///
/// Example: `let projection: Mat4 = make_perspective_infinite_reverse_rh(1.2, 16.0 / 9.0, 0.05);`
pub fn make_perspective_infinite_reverse_rh(
    fov_y_rad: f32,
    aspect_ratio: f32,
    z_near: f32,
) -> Mat4 {
    let f: f32 = 1.0 / (0.5 * fov_y_rad).tan();
    Mat4::from_cols(
        Vec4::new(f / aspect_ratio, 0.0, 0.0, 0.0),
        Vec4::new(0.0, f, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, -1.0),
        Vec4::new(0.0, 0.0, z_near, 0.0),
    )
}
