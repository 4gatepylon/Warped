//! `minimap.rs` builds the minimap for the Metal overlay each frame: a Poincaré disk of the ground
//! around the player, turned so they face up and reaching `range_m` (`MinimapSettings`), in a
//! corner of the window, with the other players in reach as marks in their team's colour and the
//! lit parts of laser shots as lines. The shader's `draw_minimap` draws the ground from the map's
//! picture (`map_layout.rs`).
//!
//! Interface: `make_minimap`.
//!
//! Example: `let minimap = make_minimap(&layout, &settings, feet, &players, &beams, (2560, 1600));`
//!
//! Must know:
//! - A point shows where it stands on the ground (its Fermi u, v), seen from the player's ground
//!   point: (x, z) / (1 + w) of that ground point in the player's ground frame, divided by
//!   tanh(range / 2R) so that the reach lands on the rim.
//! - A laser's `BeamSegment` is a start point, a unit tangent and a length, not two points: its
//!   line is sampled along the geodesic (`geodesic_point`).

use glam::{DMat4, DVec4};

use crate::geometry::{AXIS_W, AXIS_X, AXIS_Z, invert_lorentz, make_boost, make_fermi_point};
use crate::map_layout::MapLayout;
use crate::render_metal::{MAX_MINIMAP_LINE_COUNT, MAX_MINIMAP_MARK_COUNT, Minimap};
use crate::settings::{MinimapCorner, MinimapSettings};
use crate::terrain::get_fermi_coordinates;
use crate::voxel_world::{BeamSegment, geodesic_point};

const MARGIN_FRAC: f64 = 0.03;
const PLAYER_MARK_RADIUS_PX: f32 = 8.0;
pub(crate) const TEAM_COLORS: [[f32; 4]; 2] = [[0.35, 0.6, 1.0, 1.0], [1.0, 0.35, 0.3, 1.0]];
const SAMPLES_PER_SEGMENT: usize = 6;

/// `Projection` takes world points onto the minimap's unit disk: the map from the world to the
/// player's ground frame, the Poincaré disk's radius at the rim, and the curvature radius.
struct Projection {
    to_ground: DMat4,
    disk_edge: f64,
    r_m: f64,
}

impl Projection {
    /// `New` makes the projection for a player standing at `feet` (its −z the way they face),
    /// reaching `range_m`, on a map of curvature radius `r_m`, and the ground frame it uses.
    ///
    /// Example: `let (projection, ground_frame) = Projection::New(feet, 60.0, 40.0);`
    #[allow(non_snake_case)]
    fn New(feet: DMat4, range_m: f64, r_m: f64) -> (Projection, DMat4) {
        let ground_frame: DMat4 = make_ground_frame(feet, r_m);
        let projection: Projection = Projection {
            to_ground: invert_lorentz(ground_frame),
            disk_edge: (range_m / (2.0 * r_m)).tanh(),
            r_m,
        };
        (projection, ground_frame)
    }

    /// `project` gives where point `x` shows on the unit disk (x right, y down; the rim at 1).
    ///
    /// Example: `let [x, y] = projection.project(eye);`
    fn project(&self, x: DVec4) -> [f64; 2] {
        let [u_m, v_m, _] = get_fermi_coordinates(x, self.r_m);
        let p: DVec4 = self.to_ground * make_fermi_point(u_m, v_m, 0.0, self.r_m);
        let scale: f64 = 1.0 / ((1.0 + p.w) * self.disk_edge);
        [p.x * scale, p.z * scale]
    }
}

/// `make_ground_frame` gives the frame on the ground under `feet` that faces the way they do: its
/// origin the ground point below them, its y up from the ground, its −z their way.
///
/// Example: `let ground_frame = make_ground_frame(feet, 40.0);`
fn make_ground_frame(feet: DMat4, r_m: f64) -> DMat4 {
    let [u_m, v_m, _] = get_fermi_coordinates(feet.col(AXIS_W), r_m);
    let at_ground: DMat4 = make_boost(AXIS_Z, -u_m / r_m) * make_boost(AXIS_X, v_m / r_m);
    let forward: DVec4 = invert_lorentz(at_ground) * -feet.col(AXIS_Z);
    let length: f64 = forward.x.hypot(forward.z);
    assert!(
        length > 0.0,
        "make_ground_frame: the feet face straight up or down"
    );
    let (fx, fz) = (forward.x / length, forward.z / length);
    at_ground
        * DMat4::from_cols(
            DVec4::new(-fz, 0.0, fx, 0.0),
            DVec4::Y,
            DVec4::new(-fx, 0.0, -fz, 0.0),
            DVec4::W,
        )
}

/// `is_on_disk` says whether a unit-disk point is on the minimap.
///
/// Example: `if is_on_disk([0.2, -0.5]) { ... }`
fn is_on_disk(point: [f64; 2]) -> bool {
    point[0].hypot(point[1]) <= 1.0
}

/// `make_minimap` builds the shader's `Minimap` for a player standing at `feet` on the map drawn
/// by `layout`: placed in the window (`window_px`) by `settings`, with the other `players` (each
/// a point and a team) in reach as marks and the lit parts of `beams` (each a team's shot) as
/// lines, at most `MAX_MINIMAP_MARK_COUNT` and `MAX_MINIMAP_LINE_COUNT`.
///
/// Example: `let minimap = make_minimap(&layout, &settings, feet, &players, &beams, (2560, 1600));`
pub fn make_minimap(
    layout: &MapLayout,
    settings: &MinimapSettings,
    feet: DMat4,
    players: &[(DVec4, u32)],
    beams: &[(Vec<BeamSegment>, u32)],
    window_px: (u32, u32),
) -> Minimap {
    let r_m: f64 = layout.get_curvature_radius_m();
    let (projection, ground_frame) = Projection::New(feet, settings.range_m, r_m);
    let (width_px, height_px) = (f64::from(window_px.0), f64::from(window_px.1));
    let radius_px: f64 = settings.radius_frac * height_px;
    let inset_px: f64 = MARGIN_FRAC * height_px + radius_px;
    let (is_left, is_top) = match settings.corner {
        MinimapCorner::TopLeft => (true, true),
        MinimapCorner::TopRight => (false, true),
        MinimapCorner::BottomLeft => (true, false),
        MinimapCorner::BottomRight => (false, false),
    };
    let center_px: [f64; 2] = [
        if is_left {
            inset_px
        } else {
            width_px - inset_px
        },
        if is_top {
            inset_px
        } else {
            height_px - inset_px
        },
    ];
    let (corner_m, m_per_px) = layout.get_placement();
    let mut minimap: Minimap = Minimap::Hidden();
    minimap.ground_frame = ground_frame
        .to_cols_array_2d()
        .map(|column| column.map(|x| x as f32));
    minimap.placement =
        [center_px[0], center_px[1], radius_px, projection.disk_edge].map(|x| x as f32);
    minimap.map = [corner_m[0], corner_m[1], m_per_px, r_m].map(|x| x as f32);
    let marks: Vec<[[f32; 4]; 2]> = players
        .iter()
        .map(|&(x, team)| (projection.project(x), team))
        .filter(|&(point, _)| is_on_disk(point))
        .take(MAX_MINIMAP_MARK_COUNT)
        .map(|([x, y], team)| {
            [
                [x as f32, y as f32, PLAYER_MARK_RADIUS_PX, 0.0],
                TEAM_COLORS[team as usize],
            ]
        })
        .collect();
    let lines: Vec<[[f32; 4]; 2]> = beams
        .iter()
        .flat_map(|(segments, team)| segments.iter().map(move |segment| (segment, *team)))
        .flat_map(|(segment, team)| {
            let points: Vec<[f64; 2]> = (0..=SAMPLES_PER_SEGMENT)
                .map(|k| {
                    let s: f64 = segment.length * k as f64 / SAMPLES_PER_SEGMENT as f64;
                    projection.project(geodesic_point(segment.a, segment.b, s))
                })
                .collect();
            (1..points.len())
                .map(|k| (points[k - 1], points[k], team))
                .collect::<Vec<_>>()
        })
        .filter(|&(a, b, _)| is_on_disk(a) || is_on_disk(b))
        .take(MAX_MINIMAP_LINE_COUNT)
        .map(|(a, b, team)| {
            [
                [a[0], a[1], b[0], b[1]].map(|x| x as f32),
                TEAM_COLORS[team as usize],
            ]
        })
        .collect();
    minimap.marks[..marks.len()].copy_from_slice(&marks);
    minimap.lines[..lines.len()].copy_from_slice(&lines);
    minimap.counts = [marks.len() as i32, lines.len() as i32, 0, 0];
    minimap
}

#[cfg(test)]
mod tests {
    //! Tests of placing points and the minimap itself.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Point seen | ahead (up), to the right (right), at the reach (on the rim), behind a player turned to +v | `points_show_where_the_player_faces` |
    //! | Corner | top right, bottom left | `the_minimap_sits_in_its_corner` |
    //! | Players and shots | in reach (marked), out of reach (left out) | `the_minimap_sits_in_its_corner` |
    //! | Laser drawn | one straight shot ahead, as one segment (one straight line up to its end) | `a_shot_is_one_line` |

    use super::*;
    use crate::geometry::make_rotation;
    use crate::map::{Map, get_map_folder};
    use crate::settings::PerPlayerSettings;

    const R_M: f64 = 40.0;
    const RANGE_M: f64 = 60.0;
    const NEAR_M: f64 = 10.0;
    const TOLERANCE: f64 = 1e-9;
    const TOLERANCE_F32: f64 = 1e-5;
    const WINDOW_PX: (u32, u32) = (2560, 1600);

    #[test]
    fn points_show_where_the_player_faces() {
        let (projection, _) = Projection::New(DMat4::IDENTITY, RANGE_M, R_M);
        let [x, y] = projection.project(make_fermi_point(NEAR_M, 0.0, 0.0, R_M));
        assert!(x.abs() < TOLERANCE && y < 0.0, "ahead is up: {x}, {y}");
        let [x, y] = projection.project(make_fermi_point(0.0, NEAR_M, 0.0, R_M));
        assert!(x > 0.0 && y.abs() < TOLERANCE, "right is right: {x}, {y}");
        let rim: [f64; 2] = projection.project(make_fermi_point(-RANGE_M, 0.0, 0.0, R_M));
        assert!(
            (rim[0].hypot(rim[1]) - 1.0).abs() < TOLERANCE,
            "the reach is the rim: {rim:?}"
        );
        let facing_v: DMat4 = make_rotation(AXIS_X, AXIS_Z, std::f64::consts::FRAC_PI_2);
        let (projection, _) = Projection::New(facing_v, RANGE_M, R_M);
        let [x, y] = projection.project(make_fermi_point(0.0, NEAR_M, 0.0, R_M));
        assert!(
            x.abs() < TOLERANCE && y < 0.0,
            "+v is up when facing it: {x}, {y}"
        );
    }

    /// `a_shot_is_one_line` checks that a shot fired straight ahead shows as one straight line,
    /// from the player up to where the shot ends (W-91: it showed as two).
    #[test]
    fn a_shot_is_one_line() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("Lunar Surface loads");
        let layout: MapLayout = MapLayout::FromMap(&map, (992, 556)).expect("a layout");
        let mut settings: MinimapSettings = PerPlayerSettings::Builtin().minimap;
        settings.range_m = RANGE_M;
        let r_m: f64 = layout.get_curvature_radius_m();
        let ahead: DVec4 = -DMat4::IDENTITY.col(AXIS_Z);
        let shot: Vec<BeamSegment> = vec![BeamSegment {
            a: DVec4::W,
            b: ahead,
            length: NEAR_M / r_m,
        }];
        let minimap: Minimap = make_minimap(
            &layout,
            &settings,
            DMat4::IDENTITY,
            &[],
            &[(shot, 0)],
            WINDOW_PX,
        );
        let line_count: usize = minimap.counts[1] as usize;
        assert_eq!(line_count, SAMPLES_PER_SEGMENT, "one line, in its pieces");
        let (projection, _) = Projection::New(DMat4::IDENTITY, RANGE_M, r_m);
        let [_, end_y] = projection.project(make_fermi_point(NEAR_M, 0.0, 0.0, r_m));
        let mut last_y: f64 = 0.0;
        for (k, [ends, _]) in minimap.lines[..line_count].iter().enumerate() {
            let [x0, y0, x1, y1] = ends.map(f64::from);
            assert!(
                x0.abs() < TOLERANCE_F32 && x1.abs() < TOLERANCE_F32,
                "piece {k} leaves the line ahead: {ends:?}"
            );
            assert!(
                (y0 - last_y).abs() < TOLERANCE_F32 && y1 < y0,
                "piece {k} doesn't go on up from the last: {ends:?}"
            );
            last_y = y1;
        }
        assert!(
            (last_y - end_y).abs() < TOLERANCE_F32,
            "the line ends at {last_y}, not at the shot's end {end_y}"
        );
    }

    #[test]
    fn the_minimap_sits_in_its_corner() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("Lunar Surface loads");
        let layout: MapLayout = MapLayout::FromMap(&map, (992, 556)).expect("a layout");
        let mut settings: MinimapSettings = PerPlayerSettings::Builtin().minimap;
        settings.range_m = RANGE_M;
        let players: Vec<(DVec4, u32)> = vec![
            (make_fermi_point(NEAR_M, 0.0, 0.0, R_M), 1),
            (make_fermi_point(2.0 * RANGE_M, 0.0, 0.0, R_M), 0),
        ];
        let (width, height) = (f64::from(WINDOW_PX.0), f64::from(WINDOW_PX.1));
        for (corner, is_left, is_top) in [
            (MinimapCorner::TopRight, false, true),
            (MinimapCorner::BottomLeft, true, false),
        ] {
            settings.corner = corner;
            let minimap: Minimap = make_minimap(
                &layout,
                &settings,
                DMat4::IDENTITY,
                &players,
                &[],
                WINDOW_PX,
            );
            let [x, y, radius, _] = minimap.placement.map(f64::from);
            assert_eq!(x < width / 2.0, is_left);
            assert_eq!(y < height / 2.0, is_top);
            assert!(
                x - radius >= 0.0
                    && x + radius <= width
                    && y - radius >= 0.0
                    && y + radius <= height
            );
            assert_eq!(minimap.counts[0], 1, "only the near player is marked");
            assert_eq!(minimap.marks[0][1], TEAM_COLORS[1]);
        }
    }
}
