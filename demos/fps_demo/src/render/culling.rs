//! `culling` holds the backend-independent camera and shadow math: frustum planes, sphere tests,
//! and the fitting of stable cascaded shadow maps.
//!
//! Example:
//!     let frustum: Frustum = Frustum::from_view_proj(view_proj);
//!     let is_visible: bool = frustum.contains_sphere(center_m, radius_m);

use glam::{Mat4, Vec3, Vec4};

/// `CASCADE_COUNT` is the number of shadow cascades; `common.metal` must agree.
pub const CASCADE_COUNT: usize = 4;
/// `CASCADE_SPLIT_LAMBDA_FRAC` blends logarithmic (1) and uniform (0) cascade splits.
const CASCADE_SPLIT_LAMBDA_FRAC: f32 = 0.82;
/// `CASCADE_NEAR_M` is where the first cascade starts.
const CASCADE_NEAR_M: f32 = 0.1;
/// `CASCADE_CASTER_MARGIN_M` extends each cascade toward the sun, to catch tall casters.
const CASCADE_CASTER_MARGIN_M: f32 = 120.0;
/// `DEGENERATE_PLANE_LENGTH` is the plane-normal length below which a plane is ignored.
const DEGENERATE_PLANE_LENGTH: f32 = 1e-6;

/// `Frustum` is a set of inward-facing planes (xyz normal, w distance).
///
/// Example:
///     let frustum: Frustum = Frustum::from_view_proj(view_proj);
pub struct Frustum {
    pub planes: Vec<Vec4>,
}

impl Frustum {
    /// `from_view_proj` extracts the planes of a projection with depth in [0, 1].
    ///
    /// Degenerate planes, such as the far plane of an infinite projection, are dropped.
    ///
    /// Example:
    ///     let frustum: Frustum = Frustum::from_view_proj(proj * view);
    pub fn from_view_proj(view_proj: Mat4) -> Frustum {
        let row_0: Vec4 = view_proj.row(0);
        let row_1: Vec4 = view_proj.row(1);
        let row_2: Vec4 = view_proj.row(2);
        let row_3: Vec4 = view_proj.row(3);
        let candidates: [Vec4; 6] = [
            row_3 + row_0,
            row_3 - row_0,
            row_3 + row_1,
            row_3 - row_1,
            row_2,
            row_3 - row_2,
        ];
        let planes: Vec<Vec4> = candidates
            .iter()
            .filter(|plane: &&Vec4| plane.truncate().length() > DEGENERATE_PLANE_LENGTH)
            .map(|plane: &Vec4| *plane / plane.truncate().length())
            .collect();
        Frustum { planes }
    }

    /// `contains_sphere` says whether any part of a sphere is inside the frustum.
    ///
    /// Example:
    ///     let is_visible: bool = frustum.contains_sphere(Vec3::ZERO, 1.0);
    pub fn contains_sphere(&self, center_m: Vec3, radius_m: f32) -> bool {
        self.planes
            .iter()
            .all(|plane: &Vec4| plane.truncate().dot(center_m) + plane.w >= -radius_m)
    }
}

/// `Cascades` is the fitted set of shadow cascades for one frame.
///
/// Example:
///     let cascades: Cascades = fit_cascades(&camera_info, sun_direction, 120.0, 2048);
pub struct Cascades {
    pub view_proj: [Mat4; CASCADE_COUNT],
    pub split_far_m: [f32; CASCADE_COUNT],
    pub texel_size_m: [f32; CASCADE_COUNT],
}

/// `CascadeCamera` is the part of the camera that cascade fitting needs.
///
/// Example:
///     let camera: CascadeCamera = CascadeCamera { inverse_view, fov_y_rad, aspect };
pub struct CascadeCamera {
    pub inverse_view: Mat4,
    pub fov_y_rad: f32,
    pub aspect: f32,
}

/// `cascade_split_far_m` returns where each cascade ends, mixing log and uniform splits.
///
/// Example:
///     let splits: [f32; 4] = cascade_split_far_m(120.0);
pub fn cascade_split_far_m(shadow_distance_m: f32) -> [f32; CASCADE_COUNT] {
    let mut splits: [f32; CASCADE_COUNT] = [0.0; CASCADE_COUNT];
    for (index, split) in splits.iter_mut().enumerate() {
        let frac: f32 = (index + 1) as f32 / CASCADE_COUNT as f32;
        let log_split_m: f32 = CASCADE_NEAR_M * (shadow_distance_m / CASCADE_NEAR_M).powf(frac);
        let uniform_split_m: f32 = CASCADE_NEAR_M + (shadow_distance_m - CASCADE_NEAR_M) * frac;
        *split = CASCADE_SPLIT_LAMBDA_FRAC * log_split_m
            + (1.0 - CASCADE_SPLIT_LAMBDA_FRAC) * uniform_split_m;
    }
    splits
}

/// `fit_cascades` fits one orthographic sun projection around each slice of the view frustum.
///
/// Each cascade is a sphere around its slice, so its size doesn't change when the camera turns,
/// and its origin snaps to whole shadow texels, so shadow edges don't shimmer when it moves.
///
/// Example:
///     let cascades: Cascades = fit_cascades(&camera, Vec3::Y, 120.0, 2048);
pub fn fit_cascades(
    camera: &CascadeCamera,
    sun_direction: Vec3,
    shadow_distance_m: f32,
    resolution_px: u32,
) -> Cascades {
    let split_far_m: [f32; CASCADE_COUNT] = cascade_split_far_m(shadow_distance_m);
    let tan_half_y: f32 = (camera.fov_y_rad * 0.5).tan();
    let tan_half_x: f32 = tan_half_y * camera.aspect;
    let up: Vec3 = if sun_direction.y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let light_view: Mat4 = Mat4::look_to_rh(Vec3::ZERO, -sun_direction, up);
    let mut view_proj: [Mat4; CASCADE_COUNT] = [Mat4::IDENTITY; CASCADE_COUNT];
    let mut texel_size_m: [f32; CASCADE_COUNT] = [0.0; CASCADE_COUNT];
    let mut near_m: f32 = CASCADE_NEAR_M;
    for index in 0..CASCADE_COUNT {
        let far_m: f32 = split_far_m[index];
        let center_depth_m: f32 = (near_m + far_m) * 0.5;
        let far_corner: Vec3 = Vec3::new(tan_half_x * far_m, tan_half_y * far_m, -far_m);
        let near_corner: Vec3 = Vec3::new(tan_half_x * near_m, tan_half_y * near_m, -near_m);
        let center_view: Vec3 = Vec3::new(0.0, 0.0, -center_depth_m);
        let radius_m: f32 = far_corner
            .distance(center_view)
            .max(near_corner.distance(center_view))
            .ceil();
        let center_world: Vec3 = camera.inverse_view.transform_point3(center_view);
        let texel_m: f32 = 2.0 * radius_m / resolution_px as f32;
        let center_light: Vec3 = light_view.transform_point3(center_world);
        let snapped_x: f32 = (center_light.x / texel_m).floor() * texel_m;
        let snapped_y: f32 = (center_light.y / texel_m).floor() * texel_m;
        let projection: Mat4 = Mat4::orthographic_rh(
            snapped_x - radius_m,
            snapped_x + radius_m,
            snapped_y - radius_m,
            snapped_y + radius_m,
            -center_light.z - radius_m - CASCADE_CASTER_MARGIN_M,
            -center_light.z + radius_m,
        );
        view_proj[index] = projection * light_view;
        texel_size_m[index] = texel_m;
        near_m = far_m;
    }
    Cascades {
        view_proj,
        split_far_m,
        texel_size_m,
    }
}

/// `tests` checks frustum culling and the shadow cascades.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | sphere position | inside the frustum | `test_frustum_contains_sphere` |
/// | sphere position | behind the camera | `test_frustum_contains_sphere` |
/// | sphere position | beside the frustum | `test_frustum_contains_sphere` |
/// | cascade | split distances, first to last | `test_cascade_splits_increase` |
/// | cascade | a point inside the first cascade's slice | `test_cascades_cover_their_slice` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `test_frustum_contains_sphere` checks spheres in front of, behind, and beside the camera.
    #[test]
    fn test_frustum_contains_sphere() {
        let proj: Mat4 = Mat4::perspective_infinite_reverse_rh(1.2, 1.5, 0.05);
        let frustum: Frustum = Frustum::from_view_proj(proj);
        assert!(frustum.contains_sphere(Vec3::new(0.0, 0.0, -10.0), 1.0));
        assert!(!frustum.contains_sphere(Vec3::new(0.0, 0.0, 10.0), 1.0));
        assert!(!frustum.contains_sphere(Vec3::new(100.0, 0.0, -10.0), 1.0));
    }

    /// `test_cascade_splits_increase` checks that the splits grow up to the shadow distance.
    #[test]
    fn test_cascade_splits_increase() {
        let splits: [f32; CASCADE_COUNT] = cascade_split_far_m(120.0);
        assert!(splits.windows(2).all(|pair: &[f32]| pair[0] < pair[1]));
        assert!((splits[CASCADE_COUNT - 1] - 120.0).abs() < 1e-3);
    }

    /// `test_cascades_cover_their_slice` checks that a point at a cascade's center lands inside
    /// its shadow map.
    #[test]
    fn test_cascades_cover_their_slice() {
        let camera: CascadeCamera = CascadeCamera {
            inverse_view: Mat4::IDENTITY,
            fov_y_rad: 1.2,
            aspect: 1.6,
        };
        let sun: Vec3 = Vec3::new(0.3, 0.8, 0.2).normalize();
        let cascades: Cascades = fit_cascades(&camera, sun, 120.0, 2048);
        let point: Vec3 = Vec3::new(0.0, 0.0, -cascades.split_far_m[0] * 0.5);
        let ndc: Vec3 = cascades.view_proj[0].project_point3(point);
        assert!(ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0);
        assert!(ndc.z > 0.0 && ndc.z < 1.0);
    }
}
