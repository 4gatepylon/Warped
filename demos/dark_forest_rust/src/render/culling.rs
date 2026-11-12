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
/// `EARLY_MOVE_FRAC` of a cascade's spare margin is how far the camera may walk before the
/// cascade moves early, on a frame when no other cascade moves, so that two cascades rarely redraw
/// their static casters in the same frame.
const EARLY_MOVE_FRAC: f32 = 0.6;
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

/// `Cascades` is the fitted set of shadow cascades for one frame, with each cascade's centre in
/// the light's space and its half size, which the next frame's fit keeps while they still cover
/// its slice.
///
/// Example:
///     let cascades: Cascades = fit_cascades(&camera_info, sun_direction, 120.0, 2048, None);
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cascades {
    pub view_proj: [Mat4; CASCADE_COUNT],
    pub split_far_m: [f32; CASCADE_COUNT],
    pub texel_size_m: [f32; CASCADE_COUNT],
    pub center_light_m: [Vec3; CASCADE_COUNT],
    pub radius_m: [f32; CASCADE_COUNT],
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

/// `fit_cascades` fits one orthographic sun projection for each slice of the view frustum.
///
/// Each cascade is a square, in the sun's view, centred on the camera, as wide as the sphere around
/// its slice (so its texels are that sphere's size whichever way the camera turns), and its centre
/// snaps to whole shadow texels, so shadow edges don't shimmer when it moves. A cascade of
/// `previous` with the same size is kept while it still covers everything within its slice's far
/// distance of the camera, so turning never moves it and walking moves it only now and then: the
/// renderer can keep its static casters' depth between frames. A cascade past `EARLY_MOVE_FRAC`
/// of its margin moves early on a frame when no other cascade has to, one at a time.
///
/// Example:
///     let cascades: Cascades = fit_cascades(&camera, Vec3::Y, 120.0, 2048, Some(&last_cascades));
pub fn fit_cascades(
    camera: &CascadeCamera,
    sun_direction: Vec3,
    shadow_distance_m: f32,
    resolution_px: u32,
    previous: Option<&Cascades>,
) -> Cascades {
    let split_far_m: [f32; CASCADE_COUNT] = cascade_split_far_m(shadow_distance_m);
    let tan_half_y: f32 = (camera.fov_y_rad * 0.5).tan();
    let tan_half_x: f32 = tan_half_y * camera.aspect;
    let up: Vec3 = if sun_direction.y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let light_view: Mat4 = crate::render::camera_math::make_view_rh(Vec3::ZERO, -sun_direction, up);
    let camera_light: Vec3 = light_view.transform_point3(camera.inverse_view.w_axis.truncate());
    let mut cascades: Cascades = Cascades {
        view_proj: [Mat4::IDENTITY; CASCADE_COUNT],
        split_far_m,
        texel_size_m: [0.0; CASCADE_COUNT],
        center_light_m: [Vec3::ZERO; CASCADE_COUNT],
        radius_m: [0.0; CASCADE_COUNT],
    };
    let mut near_m: f32 = CASCADE_NEAR_M;
    let mut kept_centers: [Option<Vec3>; CASCADE_COUNT] = [None; CASCADE_COUNT];
    let mut walked_fracs: [f32; CASCADE_COUNT] = [f32::MAX; CASCADE_COUNT];
    for index in 0..CASCADE_COUNT {
        let far_m: f32 = split_far_m[index];
        let center_depth_m: f32 = (near_m + far_m) * 0.5;
        let far_corner: Vec3 = Vec3::new(tan_half_x * far_m, tan_half_y * far_m, -far_m);
        let near_corner: Vec3 = Vec3::new(tan_half_x * near_m, tan_half_y * near_m, -near_m);
        let center_view: Vec3 = Vec3::new(0.0, 0.0, -center_depth_m);
        let radius_m: f32 = far_corner
            .distance(center_view)
            .max(near_corner.distance(center_view))
            .max(far_m)
            .ceil();
        let texel_m: f32 = 2.0 * radius_m / resolution_px as f32;
        let slack_m: f32 = (radius_m - far_m - texel_m).max(0.0);
        if let Some(previous) = previous.filter(|previous| previous.radius_m[index] == radius_m) {
            let center: Vec3 = previous.center_light_m[index];
            kept_centers[index] = Some(center);
            walked_fracs[index] =
                (center - camera_light).abs().max_element() / slack_m.max(f32::MIN_POSITIVE);
        }
        cascades.texel_size_m[index] = texel_m;
        cascades.radius_m[index] = radius_m;
        near_m = far_m;
    }
    let is_any_forced: bool = walked_fracs.iter().any(|&frac| frac > 1.0);
    let early: Option<usize> = (!is_any_forced)
        .then(|| {
            (0..CASCADE_COUNT)
                .filter(|&index| walked_fracs[index] > EARLY_MOVE_FRAC)
                .max_by(|&a, &b| walked_fracs[a].total_cmp(&walked_fracs[b]))
        })
        .flatten();
    for index in 0..CASCADE_COUNT {
        let (radius_m, texel_m) = (cascades.radius_m[index], cascades.texel_size_m[index]);
        let is_moving: bool = walked_fracs[index] > 1.0 || early == Some(index);
        let center_light: Vec3 = match kept_centers[index] {
            Some(center) if !is_moving => center,
            _ => Vec3::new(
                (camera_light.x / texel_m).floor() * texel_m,
                (camera_light.y / texel_m).floor() * texel_m,
                camera_light.z,
            ),
        };
        let projection: Mat4 = crate::render::camera_math::make_orthographic_rh(
            center_light.x - radius_m,
            center_light.x + radius_m,
            center_light.y - radius_m,
            center_light.y + radius_m,
            -center_light.z - radius_m - CASCADE_CASTER_MARGIN_M,
            -center_light.z + radius_m,
        );
        cascades.view_proj[index] = projection * light_view;
        cascades.center_light_m[index] = center_light;
    }
    cascades
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
/// | camera move vs last fit | a turn, a step within the slack, a step past the first cascade's slack | `test_cascades_stay_until_the_camera_leaves_them` |
/// | point vs cascade | at its slice's far distance in any direction, after a turn and a step | `test_cascades_cover_their_far_distance_all_round` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `test_frustum_contains_sphere` checks spheres in front of, behind, and beside the camera.
    #[test]
    fn test_frustum_contains_sphere() {
        let proj: Mat4 =
            crate::render::camera_math::make_perspective_infinite_reverse_rh(1.2, 1.5, 0.05);
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
        let cascades: Cascades = fit_cascades(&camera, sun, 120.0, 2048, None);
        let point: Vec3 = Vec3::new(0.0, 0.0, -cascades.split_far_m[0] * 0.5);
        let ndc: Vec3 = cascades.view_proj[0].project_point3(point);
        assert!(ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0);
        assert!(ndc.z > 0.0 && ndc.z < 1.0);
    }

    /// `camera_at` is a 75° camera at `position_m` turned `yaw_rad` about the vertical.
    fn camera_at(position_m: Vec3, yaw_rad: f32) -> CascadeCamera {
        CascadeCamera {
            inverse_view: Mat4::from_translation(position_m) * Mat4::from_rotation_y(yaw_rad),
            fov_y_rad: 75.0_f32.to_radians(),
            aspect: 2.4,
        }
    }

    /// `SUN` is a sun high in the south-west.
    const SUN: Vec3 = Vec3::new(-0.4, 0.8, 0.3);

    /// `test_cascades_stay_until_the_camera_leaves_them` checks that a turn and a short step keep
    /// every cascade, and that a step past the first cascade's slack (on every axis of the sun's
    /// view, since the step is √3 times it) moves it but not the last.
    #[test]
    fn test_cascades_stay_until_the_camera_leaves_them() {
        let sun: Vec3 = SUN.normalize();
        let first: Cascades = fit_cascades(&camera_at(Vec3::ZERO, 0.0), sun, 110.0, 2048, None);
        let turned: Cascades =
            fit_cascades(&camera_at(Vec3::ZERO, 2.0), sun, 110.0, 2048, Some(&first));
        assert_eq!(turned.view_proj, first.view_proj);
        let stepped: Cascades = fit_cascades(
            &camera_at(Vec3::new(0.3, 0.0, 0.2), 2.0),
            sun,
            110.0,
            2048,
            Some(&turned),
        );
        assert_eq!(stepped.view_proj, first.view_proj);
        let slack_m: f32 = first.radius_m[0] - first.split_far_m[0];
        let walked: Cascades = fit_cascades(
            &camera_at(Vec3::new((slack_m + 1.0) * 3.0_f32.sqrt(), 0.0, 0.0), 2.0),
            sun,
            110.0,
            2048,
            Some(&stepped),
        );
        assert_ne!(walked.view_proj[0], first.view_proj[0]);
        assert_eq!(
            walked.view_proj[CASCADE_COUNT - 1],
            first.view_proj[CASCADE_COUNT - 1]
        );
    }

    /// `test_cascades_cover_their_far_distance_all_round` checks that, after a turn and a step
    /// that keep the cascades, points at each slice's far distance in 26 directions around the
    /// camera land inside that cascade's map and depth range.
    #[test]
    fn test_cascades_cover_their_far_distance_all_round() {
        let sun: Vec3 = SUN.normalize();
        let first: Cascades = fit_cascades(&camera_at(Vec3::ZERO, 0.0), sun, 110.0, 2048, None);
        let position_m: Vec3 = Vec3::new(0.4, 0.1, -0.3);
        let moved: Cascades =
            fit_cascades(&camera_at(position_m, 1.0), sun, 110.0, 2048, Some(&first));
        for index in 0..CASCADE_COUNT {
            for (x, y, z) in (-1..=1)
                .flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| (x, y, z))))
                .filter(|&direction| direction != (0, 0, 0))
            {
                let direction: Vec3 = Vec3::new(x as f32, y as f32, z as f32).normalize();
                let point: Vec3 = position_m + direction * moved.split_far_m[index];
                let ndc: Vec3 = moved.view_proj[index].project_point3(point);
                assert!(
                    ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0,
                    "{index} {direction} {ndc}"
                );
                assert!((0.0..=1.0).contains(&ndc.z), "{index} {direction} {ndc}");
            }
        }
    }
}
