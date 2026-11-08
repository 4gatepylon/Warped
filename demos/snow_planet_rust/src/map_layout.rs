//! `map_layout.rs` draws a whole map seen from above, for the menu's map page: a picture in the
//! map's Fermi coordinates (`common/maps/README.md`), u to the right and v down, with the top of
//! every structure and box in its material's colour (brighter the higher it stands), the ground's
//! relief, everything shaded as if lit from the top left, the consoles, the black hole and wormhole
//! mouths, the spawns, the team zones' edges, the playable boundary, and, on each frame, the player
//! with a circle of what is within reach of them (the rest darker and blurred) and the other
//! players as dots in their team's colour. When the map has a picture from above
//! (`map_photo.rs`), that picture replaces the drawing under the marks. It also lends the picture without labels, and where it
//! lies, to the minimap.
//!
//! Interface: `MapLayout` (`FromMap`, `draw_players`, `get_size_px`, `get_plain_pixels`,
//! `get_placement`, `get_curvature_radius_m`).
//!
//! Example: `let pixels = layout.draw_players(feet_frame, 0, 60.0, &others);`
//!
//! Must know:
//! - Fermi coordinates aren't distances: along u, a meter at v from the middle line takes only
//!   1 / cosh(v / R) of a meter on the picture, so things far above and below the middle line look
//!   squeezed from side to side. That is the hyperbolic plane showing.
//! - The picture reads a structure's shapes, and a box, straight down through the height of its
//!   frame's origin, so a frame must keep "up" pointing away from the ground, as every shipped
//!   map's frames do.

use std::error::Error;

use glam::{DMat4, DVec3, DVec4};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::surface::Surface;

use crate::geometry::{AXIS_W, AXIS_Z, invert_lorentz, make_fermi_point};
use crate::map::{Map, to_frame};
use crate::map_photo::get_map_photo_path;
use crate::net::TEAM_NAMES;
use crate::shapes::{Shape, find_hit_mu};
use crate::terrain::{Terrain, get_fermi_coordinates};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BYTES_PER_PIXEL: usize = 4;
const OPAQUE: u8 = 255;
const MARGIN_M: f64 = 4.0;
const GROUND_SHADE_FRAC: f32 = 0.45;
const OUTSIDE_SHADE_FRAC: f32 = 0.4;
const LOW_SHADE_FRAC: f32 = 0.6;
const FULL_SHADE_HEIGHT_M: f64 = 12.0;
const LIGHT_TOWARD: [f64; 3] = [-1.0, -1.0, 1.0];
const MIN_RELIEF_SHADE_FRAC: f64 = 0.35;
const MAX_RELIEF_SHADE_FRAC: f64 = 1.6;
const COLUMN_TOP_MARGIN_M: f64 = 0.5;
const GAMMA: f32 = 2.2;
const ZONE_DASH_PX: usize = 6;
const ZONE_COLOR: [u8; 3] = [230, 230, 230];
const BOUNDARY_COLOR: [u8; 3] = [255, 255, 255];
const BLACK_HOLE_COLOR: [u8; 3] = [0, 0, 0];
const BLACK_HOLE_RING_COLOR: [u8; 3] = [255, 170, 60];
const MOUTH_COLOR: [u8; 3] = [90, 230, 255];
const RING_WIDTH_PX: f64 = 1.5;
const CONSOLE_MARKER_RADIUS_PX: f64 = 2.5;
const SPAWN_MARKER_RADIUS_PX: f64 = 3.5;
const TEAM_COLORS: [[u8; 3]; 2] = [[90, 150, 255], [255, 90, 80]];
const ANY_TEAM_COLOR: [u8; 3] = [230, 230, 230];
const LABEL_COLOR: [u8; 3] = [255, 255, 255];
const LABEL_GAP_PX: f64 = 3.0;
const LABEL_SHADOW_PX: f32 = 1.0;
const LABEL_OUTLINE_OFFSETS_PX: [[f32; 2]; 5] = [
    [-1.0, 0.0],
    [1.0, 0.0],
    [0.0, -1.0],
    [0.0, 1.0],
    [LABEL_SHADOW_PX, LABEL_SHADOW_PX],
];
const DEBUG_GLYPH_PX: f32 = 8.0;
const PLAYER_RADIUS_PX: f64 = 5.0;
const PLAYER_OUTLINE_PX: f64 = 1.5;
const PLAYER_HEADING_PX: f64 = 16.0;
const PLAYER_HEADING_HALF_WIDTH_PX: f64 = 1.2;
const HEADING_PROBE_M: f64 = 1.0;
const PLAYER_OUTLINE_COLOR: [u8; 3] = [255, 255, 255];
const OTHER_PLAYER_RADIUS_PX: f64 = 4.0;
const OTHER_PLAYER_OUTLINE_PX: f64 = 1.5;
const OTHER_PLAYER_OUTLINE_COLOR: [u8; 3] = [20, 20, 20];
const WARP_KIND_BLACK_HOLE: &str = "black_hole";
const MOUTH_NAME_PREFIX: &str = "wormhole mouth ";
const MATERIAL_KIND_LIGHT: &str = "light";
const SPAWN_TEAM_ANY: &str = "any";
const OUT_OF_RANGE_BLUR_PX: usize = 3;
const OUT_OF_RANGE_SHADE_FRAC: f32 = 0.55;
const RANGE_RIM_COLOR: [u8; 3] = [255, 255, 255];

/// `Top` is what a column of a structure or box shows from above: its top's Fermi height h in
/// meters, and its material.
type Top = (f64, u8);

/// `Footprint` is one structure or box seen from above: the map from the world to its frame, the
/// height of its frame's origin, the circle on the picture's plane (u, v) that holds it, and its
/// form.
struct Footprint {
    to_local: DMat4,
    center_h_m: f64,
    center_m: [f64; 2],
    radius_m: f64,
    form: Form,
}

/// `Form` is a footprint's form: a structure's smooth shapes (in its local chart, meters), or a box
/// with its half sizes and material.
enum Form {
    Shapes(Vec<Shape>),
    Box { half_size_m: [f64; 3], material: u8 },
}

impl Footprint {
    /// `get_top` gives what this footprint shows at ground point (u, v), or `None` where it has
    /// nothing, on a map of curvature radius `r_m`.
    ///
    /// Example: `let top = footprint.get_top(12.0, -3.0, 40.0);`
    fn get_top(&self, u_m: f64, v_m: f64, r_m: f64) -> Option<Top> {
        let p: DVec4 = self.to_local * make_fermi_point(u_m, v_m, self.center_h_m, r_m);
        match &self.form {
            Form::Shapes(shapes) => {
                let (q_x_m, q_z_m) = (r_m * p.x / p.w, r_m * p.z / p.w);
                let candidates: Vec<Shape> = shapes
                    .iter()
                    .filter(|shape| {
                        let reach_m: f64 = shape.half.max_element();
                        (q_x_m - shape.centre.x).abs() <= reach_m
                            && (q_z_m - shape.centre.z).abs() <= reach_m
                    })
                    .copied()
                    .collect();
                let start_m: f64 = candidates
                    .iter()
                    .map(|shape| shape.centre.y + shape.half.max_element())
                    .fold(f64::MIN, f64::max)
                    + COLUMN_TOP_MARGIN_M;
                let end_m: f64 = candidates
                    .iter()
                    .map(|shape| shape.centre.y - shape.half.max_element())
                    .fold(f64::MAX, f64::min);
                let (mu, i) = find_hit_mu(
                    &candidates,
                    DVec3::new(q_x_m, start_m, q_z_m),
                    DVec3::new(0.0, end_m - start_m, 0.0),
                    0.0,
                    1.0,
                    None,
                )?;
                Some((
                    self.center_h_m + start_m + mu * (end_m - start_m),
                    candidates[i].material,
                ))
            }
            Form::Box {
                half_size_m,
                material,
            } => {
                let is_inside: bool = p.x.abs() <= (half_size_m[0] / r_m).sinh()
                    && p.z.abs() <= (half_size_m[2] / r_m).sinh();
                is_inside.then_some((self.center_h_m + half_size_m[1], *material))
            }
        }
    }
}

/// `MapLayout` is a map's picture seen from above (RGBA, top row first), the same picture without
/// labels (for the minimap) and darkened and blurred (out of the player's range), and what
/// `draw_players` needs to place players on it: the curvature radius, the Fermi coordinates of the
/// picture's top left corner and the meters per pixel.
pub struct MapLayout {
    pixels: Vec<u8>,
    plain_pixels: Vec<u8>,
    dim_pixels: Vec<u8>,
    size_px: (u32, u32),
    r_m: f64,
    corner_m: [f64; 2],
    m_per_px: f64,
}

impl MapLayout {
    /// `FromMap` draws `map` from above in a picture at most `max_size_px` big: its playable
    /// boundary with a margin, or, on a map without one, everything on it with a margin.
    ///
    /// Example: `let layout = MapLayout::FromMap(&map, (992, 536))?;`
    #[allow(non_snake_case)]
    pub fn FromMap(map: &Map, max_size_px: (u32, u32)) -> Result<MapLayout> {
        let r_m: f64 = map.get_curvature_radius_m();
        let footprints: Vec<Footprint> = make_footprints(map, r_m)?;
        let boundary_m: Option<[f64; 2]> = map
            .regions
            .as_ref()
            .and_then(|regions| regions.boundary.as_ref())
            .map(|boundary| [boundary.u_m, boundary.v_m]);
        let [min_m, max_m] = get_bounds_m(map, &footprints, boundary_m, r_m);
        let m_per_px: f64 = ((max_m[0] - min_m[0]) / f64::from(max_size_px.0))
            .max((max_m[1] - min_m[1]) / f64::from(max_size_px.1));
        let size_px: (u32, u32) = (
            ((max_m[0] - min_m[0]) / m_per_px)
                .ceil()
                .min(f64::from(max_size_px.0)) as u32,
            ((max_m[1] - min_m[1]) / m_per_px)
                .ceil()
                .min(f64::from(max_size_px.1)) as u32,
        );
        let mut layout: MapLayout = MapLayout {
            pixels: vec![0; size_px.0 as usize * size_px.1 as usize * BYTES_PER_PIXEL],
            plain_pixels: Vec::new(),
            dim_pixels: Vec::new(),
            size_px,
            r_m,
            corner_m: min_m,
            m_per_px,
        };
        layout.paint_tops(map, &footprints, boundary_m)?;
        let has_photo: bool = layout.paint_photo(&map.name)?;
        if let Some(regions) = &map.regions {
            for zone in regions.zones.iter().flatten() {
                for u_m in [zone.u_min_m, zone.u_max_m].into_iter().flatten() {
                    layout.paint_zone_edge(u_m, boundary_m);
                }
            }
        }
        let mut labels: Vec<(String, [f64; 2])> = Vec::new();
        for (point_m, team) in find_console_points(map, r_m) {
            let center_px: [f64; 2] = layout.get_px(point_m);
            layout.paint_disk(
                center_px,
                CONSOLE_MARKER_RADIUS_PX,
                TEAM_COLORS[team as usize],
            );
        }
        for warp in &map.warps {
            let [u_m, v_m, _] = get_fermi_coordinates(to_frame(&warp.frame).col(AXIS_W), r_m);
            let center_px: [f64; 2] = layout.get_px([u_m, v_m]);
            let radius_px: f64 = warp.schwarzschild_radius_m / m_per_px;
            if warp.kind == WARP_KIND_BLACK_HOLE {
                if !has_photo {
                    layout.paint_disk(center_px, radius_px, BLACK_HOLE_COLOR);
                    layout.paint_ring(center_px, radius_px, BLACK_HOLE_RING_COLOR);
                }
                labels.push(make_label_right("black hole", center_px, radius_px));
            } else {
                if !has_photo {
                    layout.paint_ring(center_px, radius_px, MOUTH_COLOR);
                }
                let name: &str = warp
                    .name
                    .strip_prefix(MOUTH_NAME_PREFIX)
                    .unwrap_or(&warp.name);
                labels.push(make_label_right(name, center_px, radius_px));
            }
        }
        for spawn in &map.spawns {
            let [u_m, v_m, _] = get_fermi_coordinates(to_frame(&spawn.frame).col(AXIS_W), r_m);
            let center_px: [f64; 2] = layout.get_px([u_m, v_m]);
            let (color, label): ([u8; 3], String) = match spawn.team.as_str() {
                SPAWN_TEAM_ANY => (ANY_TEAM_COLOR, "spawn".to_string()),
                team => {
                    let index: usize = TEAM_NAMES
                        .iter()
                        .position(|name| *name == team)
                        .ok_or_else(|| format!("map {}: no team {team:?}", map.name))?;
                    (TEAM_COLORS[index], format!("{team} spawn"))
                }
            };
            layout.paint_disk(center_px, SPAWN_MARKER_RADIUS_PX, color);
            labels.push((
                label.clone(),
                [
                    center_px[0] - f64::from(DEBUG_GLYPH_PX) * label.len() as f64 / 2.0,
                    center_px[1] + SPAWN_MARKER_RADIUS_PX + LABEL_GAP_PX,
                ],
            ));
        }
        layout.plain_pixels = layout.pixels.clone();
        layout.draw_labels(&labels)?;
        layout.dim_pixels = make_dim_pixels(&layout.pixels, size_px);
        Ok(layout)
    }

    /// `paint_photo` replaces the drawing with the map's picture from above (`map_photo.rs`),
    /// scaled to this picture's size, and says whether the map has one.
    ///
    /// Example: `let has_photo = layout.paint_photo("snow_planet")?;`
    fn paint_photo(&mut self, map_name: &str) -> Result<bool> {
        let path: std::path::PathBuf = get_map_photo_path(map_name);
        if !path.exists() {
            return Ok(false);
        }
        let photo: image::RgbaImage = image::imageops::resize(
            &image::open(&path)?.to_rgba8(),
            self.size_px.0,
            self.size_px.1,
            image::imageops::FilterType::Triangle,
        );
        self.pixels.copy_from_slice(photo.as_raw());
        Ok(true)
    }

    /// `get_size_px` gives the picture's width and height in pixels.
    ///
    /// Example: `let (width_px, height_px) = layout.get_size_px();`
    pub fn get_size_px(&self) -> (u32, u32) {
        self.size_px
    }

    /// `get_plain_pixels` gives the picture without its labels (RGBA, top row first), for the
    /// minimap, which turns it.
    ///
    /// Example: `let pixels = layout.get_plain_pixels();`
    pub fn get_plain_pixels(&self) -> &[u8] {
        &self.plain_pixels
    }

    /// `get_placement` gives where the picture lies: the Fermi (u, v) of its top left corner and
    /// its meters per pixel.
    ///
    /// Example: `let (corner_m, m_per_px) = layout.get_placement();`
    pub fn get_placement(&self) -> ([f64; 2], f64) {
        (self.corner_m, self.m_per_px)
    }

    /// `get_curvature_radius_m` gives the map's curvature radius.
    ///
    /// Example: `let r_m = layout.get_curvature_radius_m();`
    pub fn get_curvature_radius_m(&self) -> f64 {
        self.r_m
    }

    /// `draw_players` gives the picture with this player standing at `feet` (a frame: its
    /// position, and its −z the way they face) as a disk in their team's colour (an index into
    /// `TEAM_NAMES`), ringed in white, with a white line the way they face, and everything farther
    /// than `range_m` from them (along the ground) darker and blurred, behind a white rim. Each of
    /// `others` (a point in the world and a team) shows as a smaller disk in its team's colour,
    /// ringed in dark grey, over the ground point under it, near or far, under this player's disk.
    ///
    /// Example: `let pixels = layout.draw_players(feet, 0, 60.0, &others);`
    pub fn draw_players(
        &self,
        feet: DMat4,
        team: u32,
        range_m: f64,
        others: &[(DVec4, u32)],
    ) -> Vec<u8> {
        let x: DVec4 = feet.col(AXIS_W);
        let forward: DVec4 = -feet.col(AXIS_Z);
        let probe: f64 = HEADING_PROBE_M / self.r_m;
        let ahead: DVec4 = x * probe.cosh() + forward * probe.sinh();
        let [u_m, v_m, _] = get_fermi_coordinates(x, self.r_m);
        let [ahead_u_m, ahead_v_m, _] = get_fermi_coordinates(ahead, self.r_m);
        let heading: DVec3 = DVec3::new(ahead_u_m - u_m, ahead_v_m - v_m, 0.0).normalize();
        let center_px: [f64; 2] = self.get_px([u_m, v_m]);
        let mut picture: MapLayout = MapLayout {
            pixels: self.get_range_pixels([u_m, v_m], range_m),
            plain_pixels: Vec::new(),
            dim_pixels: Vec::new(),
            size_px: self.size_px,
            r_m: self.r_m,
            corner_m: self.corner_m,
            m_per_px: self.m_per_px,
        };
        for &(other, other_team) in others {
            let other_px: [f64; 2] = picture.get_ground_px(other);
            picture.paint_disk(
                other_px,
                OTHER_PLAYER_RADIUS_PX + OTHER_PLAYER_OUTLINE_PX,
                OTHER_PLAYER_OUTLINE_COLOR,
            );
            picture.paint_disk(
                other_px,
                OTHER_PLAYER_RADIUS_PX,
                TEAM_COLORS[other_team as usize],
            );
        }
        for step in 0..=PLAYER_HEADING_PX as usize {
            let along_px: f64 = step as f64;
            let point_px: [f64; 2] = [
                center_px[0] + heading.x * along_px,
                center_px[1] + heading.y * along_px,
            ];
            picture.paint_disk(point_px, PLAYER_HEADING_HALF_WIDTH_PX, PLAYER_OUTLINE_COLOR);
        }
        picture.paint_disk(
            center_px,
            PLAYER_RADIUS_PX + PLAYER_OUTLINE_PX,
            PLAYER_OUTLINE_COLOR,
        );
        picture.paint_disk(center_px, PLAYER_RADIUS_PX, TEAM_COLORS[team as usize]);
        picture.pixels
    }

    /// `get_range_pixels` gives the picture with the pixels whose ground point is farther than
    /// `range_m` from ground point `center_m` (u, v) taken from the darker, blurred copy, and a
    /// white rim on the near side of that edge. The distance d is hyperbolic: for ground point X at
    /// (u, v) and the centre P, cosh d = −⟨X, P⟩ = cosh v (sinh u P_z + cosh u P_w) − sinh v P_x
    /// (in curvature radii), so with a term per column and per row each pixel costs a few
    /// products.
    ///
    /// Example: `let pixels = layout.get_range_pixels([-60.0, 0.0], 60.0);`
    fn get_range_pixels(&self, center_m: [f64; 2], range_m: f64) -> Vec<u8> {
        let (width, height) = (self.size_px.0 as usize, self.size_px.1 as usize);
        let p: DVec4 = make_fermi_point(center_m[0], center_m[1], 0.0, self.r_m);
        let max_cosh: f64 = (range_m / self.r_m).cosh();
        let column_terms: Vec<f64> = (0..width)
            .map(|x| {
                let u: f64 = (self.corner_m[0] + (x as f64 + 0.5) * self.m_per_px) / self.r_m;
                u.sinh() * p.z + u.cosh() * p.w
            })
            .collect();
        let rows: Vec<(f64, f64)> = (0..height)
            .map(|y| {
                let v: f64 = (self.corner_m[1] + (y as f64 + 0.5) * self.m_per_px) / self.r_m;
                (v.sinh() * p.x, v.cosh())
            })
            .collect();
        let is_near = |x: usize, y: usize| -> bool {
            let (sinh_term, cosh_v) = rows[y];
            cosh_v * column_terms[x] - sinh_term <= max_cosh
        };
        let mut pixels: Vec<u8> = self.pixels.clone();
        for y in 0..height {
            for x in 0..width {
                let i: usize = (x + width * y) * BYTES_PER_PIXEL;
                if !is_near(x, y) {
                    pixels[i..i + BYTES_PER_PIXEL]
                        .copy_from_slice(&self.dim_pixels[i..i + BYTES_PER_PIXEL]);
                } else if [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ]
                .into_iter()
                .any(|(nx, ny)| nx < width && ny < height && !is_near(nx, ny))
                {
                    pixels[i..i + BYTES_PER_PIXEL].copy_from_slice(&[
                        RANGE_RIM_COLOR[0],
                        RANGE_RIM_COLOR[1],
                        RANGE_RIM_COLOR[2],
                        OPAQUE,
                    ]);
                }
            }
        }
        pixels
    }

    /// `get_ground_px` gives where the ground point under world point `x` (its Fermi u, v) falls
    /// on the picture, in pixels from its top left corner.
    ///
    /// Example: `let [x_px, y_px] = layout.get_ground_px(player.frame.col(AXIS_W));`
    fn get_ground_px(&self, x: DVec4) -> [f64; 2] {
        let [u_m, v_m, _] = get_fermi_coordinates(x, self.r_m);
        self.get_px([u_m, v_m])
    }

    /// `get_px` gives where ground point (u, v), in meters, falls on the picture, in pixels from
    /// its top left corner.
    ///
    /// Example: `let [x_px, y_px] = layout.get_px([12.0, -3.0]);`
    fn get_px(&self, point_m: [f64; 2]) -> [f64; 2] {
        [
            (point_m[0] - self.corner_m[0]) / self.m_per_px,
            (point_m[1] - self.corner_m[1]) / self.m_per_px,
        ]
    }

    /// `paint_tops` paints every pixel: the highest top of the footprints there above the ground in
    /// its material's colour (lights at full brightness, the rest brighter the higher above the
    /// ground), else the ground; then lit from the top left by the slope of those heights, darker
    /// outside the boundary, which gets a white outline.
    ///
    /// Example: `layout.paint_tops(&map, &footprints, Some([128.0, 72.0]))?;`
    fn paint_tops(
        &mut self,
        map: &Map,
        footprints: &[Footprint],
        boundary_m: Option<[f64; 2]>,
    ) -> Result<()> {
        let (width, height) = (self.size_px.0 as usize, self.size_px.1 as usize);
        let (corner_m, m_per_px, r_m) = (self.corner_m, self.m_per_px, self.r_m);
        let get_point_m = |x: usize, y: usize| -> [f64; 2] {
            [
                corner_m[0] + (x as f64 + 0.5) * m_per_px,
                corner_m[1] + (y as f64 + 0.5) * m_per_px,
            ]
        };
        let is_inside = |x: usize, y: usize| -> bool {
            let [u_m, v_m] = get_point_m(x, y);
            match boundary_m {
                Some([u_max_m, v_max_m]) => {
                    (u_m / u_max_m).powi(2) + (v_m / v_max_m).powi(2) <= 1.0
                }
                None => true,
            }
        };
        let ground: [f32; 3] = map.get_material(map.ground.material)?.color;
        let terrain: Option<Terrain> = map.ground.terrain.as_ref().map(Terrain::FromMap);
        let mut heights_m: Vec<f64> = vec![0.0; width * height];
        let mut colors: Vec<[f32; 3]> = vec![[0.0; 3]; width * height];
        for y in 0..height {
            for x in 0..width {
                let [u_m, v_m] = get_point_m(x, y);
                let ground_m: f64 = match &terrain {
                    Some(terrain) => terrain.get_height_m(u_m, v_m),
                    None => 0.0,
                };
                let top: Option<Top> = footprints
                    .iter()
                    .filter(|f| (u_m - f.center_m[0]).hypot(v_m - f.center_m[1]) <= f.radius_m)
                    .filter_map(|f| f.get_top(u_m, v_m, r_m))
                    .filter(|(top_m, _)| *top_m > ground_m)
                    .max_by(|a, b| a.0.total_cmp(&b.0));
                heights_m[x + width * y] = ground_m;
                colors[x + width * y] = match top {
                    None => ground.map(|c| c * GROUND_SHADE_FRAC),
                    Some((top_m, id)) => {
                        heights_m[x + width * y] = top_m;
                        let material = map.get_material(id)?;
                        let shade_frac: f32 = if material.kind == MATERIAL_KIND_LIGHT {
                            1.0
                        } else {
                            LOW_SHADE_FRAC
                                + (1.0 - LOW_SHADE_FRAC)
                                    * ((top_m - ground_m) / FULL_SHADE_HEIGHT_M).min(1.0) as f32
                        };
                        material.color.map(|c| c * shade_frac)
                    }
                };
            }
        }
        for y in 0..height {
            for x in 0..width {
                let i: usize = x + width * y;
                let get_slope = |before: usize, after: usize| -> f64 {
                    (heights_m[after] - heights_m[before]) / (2.0 * m_per_px)
                };
                let slope_x: f64 = get_slope(
                    x.saturating_sub(1) + width * y,
                    (x + 1).min(width - 1) + width * y,
                );
                let slope_y: f64 = get_slope(
                    x + width * y.saturating_sub(1),
                    x + width * (y + 1).min(height - 1),
                );
                let normal: DVec3 = DVec3::new(-slope_x, -slope_y, 1.0).normalize();
                let toward_light: DVec3 = DVec3::from_array(LIGHT_TOWARD).normalize();
                let mut frac: f32 = (normal.dot(toward_light) / toward_light.z)
                    .clamp(MIN_RELIEF_SHADE_FRAC, MAX_RELIEF_SHADE_FRAC)
                    as f32;
                if !is_inside(x, y) {
                    frac *= OUTSIDE_SHADE_FRAC;
                }
                let is_edge: bool = is_inside(x, y)
                    && [
                        (x.wrapping_sub(1), y),
                        (x + 1, y),
                        (x, y.wrapping_sub(1)),
                        (x, y + 1),
                    ]
                    .into_iter()
                    .any(|(nx, ny)| nx < width && ny < height && !is_inside(nx, ny));
                let rgb: [u8; 3] = if is_edge {
                    BOUNDARY_COLOR
                } else {
                    colors[i].map(|c| encode_srgb(c * frac))
                };
                self.pixels[i * BYTES_PER_PIXEL..][..BYTES_PER_PIXEL]
                    .copy_from_slice(&[rgb[0], rgb[1], rgb[2], OPAQUE]);
            }
        }
        Ok(())
    }

    /// `paint_zone_edge` paints a dashed line down the picture at `u_m`, the edge of a team's zone,
    /// inside the boundary (if any).
    ///
    /// Example: `layout.paint_zone_edge(45.0, Some([128.0, 72.0]));`
    fn paint_zone_edge(&mut self, u_m: f64, boundary_m: Option<[f64; 2]>) {
        let x: f64 = self.get_px([u_m, 0.0])[0];
        for y in (0..self.size_px.1 as usize).filter(|y| (y / ZONE_DASH_PX).is_multiple_of(2)) {
            let v_m: f64 = self.corner_m[1] + (y as f64 + 0.5) * self.m_per_px;
            let is_inside: bool = match boundary_m {
                Some([u_max_m, v_max_m]) => (u_m / u_max_m).powi(2) + (v_m / v_max_m).powi(2) < 1.0,
                None => true,
            };
            if is_inside {
                self.paint_pixel(x as i64, y as i64, ZONE_COLOR);
            }
        }
    }

    /// `paint_disk` paints the pixels within `radius_px` of `center_px` in `color`.
    ///
    /// Example: `layout.paint_disk([120.0, 80.0], 3.0, [255, 255, 255]);`
    fn paint_disk(&mut self, center_px: [f64; 2], radius_px: f64, color: [u8; 3]) {
        self.paint_where(center_px, radius_px, color, |distance_px| {
            distance_px <= radius_px
        });
    }

    /// `paint_ring` paints a ring `RING_WIDTH_PX` wide around `center_px` at `radius_px`.
    ///
    /// Example: `layout.paint_ring([120.0, 80.0], 6.0, [90, 230, 255]);`
    fn paint_ring(&mut self, center_px: [f64; 2], radius_px: f64, color: [u8; 3]) {
        self.paint_where(center_px, radius_px + RING_WIDTH_PX, color, |distance_px| {
            (distance_px - radius_px).abs() <= RING_WIDTH_PX / 2.0
        });
    }

    /// `paint_where` paints in `color` the pixels within `reach_px` of `center_px` whose centre's
    /// distance from it passes `should_paint`.
    ///
    /// Example: `layout.paint_where([120.0, 80.0], 4.0, color, |d| d <= 4.0);`
    fn paint_where(
        &mut self,
        center_px: [f64; 2],
        reach_px: f64,
        color: [u8; 3],
        should_paint: impl Fn(f64) -> bool,
    ) {
        let (min_x, max_x) = (
            (center_px[0] - reach_px).floor() as i64,
            (center_px[0] + reach_px).ceil() as i64,
        );
        let (min_y, max_y) = (
            (center_px[1] - reach_px).floor() as i64,
            (center_px[1] + reach_px).ceil() as i64,
        );
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let distance_px: f64 =
                    (x as f64 + 0.5 - center_px[0]).hypot(y as f64 + 0.5 - center_px[1]);
                if should_paint(distance_px) {
                    self.paint_pixel(x, y, color);
                }
            }
        }
    }

    /// `paint_pixel` paints pixel (x, y) in `color`, or nothing if it is off the picture.
    ///
    /// Example: `layout.paint_pixel(12, 40, [255, 255, 255]);`
    fn paint_pixel(&mut self, x: i64, y: i64, color: [u8; 3]) {
        let (width, height) = (i64::from(self.size_px.0), i64::from(self.size_px.1));
        if (0..width).contains(&x) && (0..height).contains(&y) {
            let i: usize = (x + width * y) as usize * BYTES_PER_PIXEL;
            self.pixels[i..i + BYTES_PER_PIXEL]
                .copy_from_slice(&[color[0], color[1], color[2], OPAQUE]);
        }
    }

    /// `draw_labels` writes each label with SDL's built-in text, white with a black shadow, its
    /// top left corner at its point.
    ///
    /// Example: `layout.draw_labels(&[("A".to_string(), [120.0, 80.0])])?;`
    fn draw_labels(&mut self, labels: &[(String, [f64; 2])]) -> Result<()> {
        let (width, height) = self.size_px;
        let pitch_bytes: u32 = width * BYTES_PER_PIXEL as u32;
        let surface: Surface = Surface::from_data(
            &mut self.pixels,
            width,
            height,
            pitch_bytes,
            PixelFormat::RGBA32,
        )?;
        let mut canvas = surface.into_canvas()?;
        for (text, point_px) in labels {
            let (x, y) = (point_px[0] as f32, point_px[1] as f32);
            canvas.set_draw_color(Color::RGB(0, 0, 0));
            for [dx, dy] in LABEL_OUTLINE_OFFSETS_PX {
                canvas.draw_debug_text(text.as_str(), (x + dx, y + dy))?;
            }
            canvas.set_draw_color(Color::RGB(LABEL_COLOR[0], LABEL_COLOR[1], LABEL_COLOR[2]));
            canvas.draw_debug_text(text.as_str(), (x, y))?;
        }
        canvas.present();
        Ok(())
    }
}

/// `make_dim_pixels` gives a picture blurred by a box `2 OUT_OF_RANGE_BLUR_PX + 1` pixels wide,
/// first across and then down, and darkened to `OUT_OF_RANGE_SHADE_FRAC`.
///
/// Example: `let dim = make_dim_pixels(&pixels, (966, 556));`
fn make_dim_pixels(pixels: &[u8], size_px: (u32, u32)) -> Vec<u8> {
    let (width, height) = (size_px.0 as usize, size_px.1 as usize);
    let blur = |source: &[u8], step: [usize; 2]| -> Vec<u8> {
        let mut blurred: Vec<u8> = source.to_vec();
        for y in 0..height {
            for x in 0..width {
                let mut sum: [u32; BYTES_PER_PIXEL] = [0; BYTES_PER_PIXEL];
                let mut count: u32 = 0;
                for k in 0..=2 * OUT_OF_RANGE_BLUR_PX {
                    let (nx, ny) = (
                        (x + k * step[0]).wrapping_sub(OUT_OF_RANGE_BLUR_PX * step[0]),
                        (y + k * step[1]).wrapping_sub(OUT_OF_RANGE_BLUR_PX * step[1]),
                    );
                    if nx < width && ny < height {
                        let j: usize = (nx + width * ny) * BYTES_PER_PIXEL;
                        for c in 0..BYTES_PER_PIXEL {
                            sum[c] += u32::from(source[j + c]);
                        }
                        count += 1;
                    }
                }
                let i: usize = (x + width * y) * BYTES_PER_PIXEL;
                for c in 0..BYTES_PER_PIXEL {
                    blurred[i + c] = (sum[c] / count) as u8;
                }
            }
        }
        blurred
    };
    let mut dim: Vec<u8> = blur(&blur(pixels, [1, 0]), [0, 1]);
    for pixel in dim.as_chunks_mut::<BYTES_PER_PIXEL>().0 {
        for channel in &mut pixel[..BYTES_PER_PIXEL - 1] {
            *channel = (f32::from(*channel) * OUT_OF_RANGE_SHADE_FRAC) as u8;
        }
    }
    dim
}

/// `make_label_right` places a label just right of a ring of `radius_px` around `center_px`,
/// centred on it from top to bottom.
///
/// Example: `labels.push(make_label_right("A", [120.0, 80.0], 6.0));`
fn make_label_right(text: &str, center_px: [f64; 2], radius_px: f64) -> (String, [f64; 2]) {
    (
        text.to_string(),
        [
            center_px[0] + radius_px + LABEL_GAP_PX,
            center_px[1] - f64::from(DEBUG_GLYPH_PX) / 2.0,
        ],
    )
}

/// `encode_srgb` turns a linear colour channel (0 to 1, clamped) into a display byte.
///
/// Example: `let byte = encode_srgb(0.5);`
fn encode_srgb(linear: f32) -> u8 {
    (linear.clamp(0.0, 1.0).powf(1.0 / GAMMA) * f32::from(u8::MAX)).round() as u8
}

/// `make_footprints` gives every structure's and box's footprint on a map of curvature radius
/// `r_m`.
///
/// Example: `let footprints = make_footprints(&map, 40.0)?;`
fn make_footprints(map: &Map, r_m: f64) -> Result<Vec<Footprint>> {
    let mut footprints: Vec<Footprint> = Vec::new();
    for structure in &map.structures {
        let shapes: Vec<Shape> = structure
            .shapes
            .iter()
            .map(Shape::FromMap)
            .collect::<std::result::Result<_, _>>()?;
        let reach_m: f64 = shapes
            .iter()
            .map(|shape| {
                let half_m: f64 = shape.half.max_element();
                (shape.centre.x.abs() + half_m).hypot(shape.centre.z.abs() + half_m)
            })
            .fold(0.0, f64::max);
        if reach_m >= r_m {
            return Err(format!(
                "map {}: structure {:?} reaches past the horizon",
                map.name, structure.name
            )
            .into());
        }
        footprints.push(make_footprint(
            to_frame(&structure.frame),
            r_m * (reach_m / r_m).atanh(),
            r_m,
            Form::Shapes(shapes),
        ));
    }
    for map_box in &map.boxes {
        let [hx, _, hz] = map_box.half_size_m.map(|h| (h / r_m).sinh());
        footprints.push(make_footprint(
            to_frame(&map_box.frame),
            r_m * (1.0 + hx * hx + hz * hz).sqrt().acosh(),
            r_m,
            Form::Box {
                half_size_m: map_box.half_size_m,
                material: map_box.material,
            },
        ));
    }
    Ok(footprints)
}

/// `make_footprint` makes the footprint of a form with frame `frame` that stays within `radius_m`
/// of its origin.
///
/// Example: `let footprint = make_footprint(frame, 9.0, 40.0, form);`
fn make_footprint(frame: DMat4, radius_m: f64, r_m: f64, form: Form) -> Footprint {
    let [u_m, v_m, h_m] = get_fermi_coordinates(frame.col(AXIS_W), r_m);
    Footprint {
        to_local: invert_lorentz(frame),
        center_h_m: h_m,
        center_m: [u_m, v_m],
        radius_m,
        form,
    }
}

/// `find_console_points` gives every console shape of a map's structures: the ground point (u, v)
/// under its centre and its team (an index into `TEAM_NAMES`), from the materials named
/// "<team> console <n>".
///
/// Example: `for (point_m, team) in find_console_points(&map, 40.0) { ... }`
fn find_console_points(map: &Map, r_m: f64) -> Vec<([f64; 2], u32)> {
    let consoles = crate::voxel_world::find_consoles(map);
    let mut points: Vec<([f64; 2], u32)> = Vec::new();
    for structure in &map.structures {
        let frame: DMat4 = to_frame(&structure.frame);
        for shape in &structure.shapes {
            let Some(console) = consoles.iter().find(|c| c.material == shape.material) else {
                continue;
            };
            let q: DVec3 = DVec3::from_array(shape.centre_m) / r_m;
            let local: DVec4 = q.extend(1.0) / (1.0 - q.length_squared()).sqrt();
            let [u_m, v_m, _] = get_fermi_coordinates(frame * local, r_m);
            points.push(([u_m, v_m], console.team));
        }
    }
    points
}

/// `get_bounds_m` gives the corners (u, v) of what the picture shows: the boundary, else every
/// footprint, warp and spawn, with `MARGIN_M` around it.
///
/// Example: `let [min_m, max_m] = get_bounds_m(&map, &footprints, Some([128.0, 72.0]), 40.0);`
fn get_bounds_m(
    map: &Map,
    footprints: &[Footprint],
    boundary_m: Option<[f64; 2]>,
    r_m: f64,
) -> [[f64; 2]; 2] {
    let points_m: Vec<([f64; 2], f64)> = match boundary_m {
        Some(half_m) => vec![
            ([0.0, 0.0], 0.0),
            ([half_m[0], half_m[1]], 0.0),
            ([-half_m[0], -half_m[1]], 0.0),
        ],
        None => {
            let frames = (map.warps.iter())
                .map(|warp| (warp.frame, warp.schwarzschild_radius_m))
                .chain(map.spawns.iter().map(|spawn| (spawn.frame, 0.0)));
            let frame_points_m = frames.map(|(frame, radius_m)| {
                let [u_m, v_m, _] = get_fermi_coordinates(to_frame(&frame).col(AXIS_W), r_m);
                ([u_m, v_m], radius_m)
            });
            (footprints.iter())
                .map(|f| (f.center_m, f.radius_m))
                .chain(frame_points_m)
                .collect()
        }
    };
    let mut bounds_m: [[f64; 2]; 2] = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
    for (point_m, radius_m) in points_m {
        for a in 0..2 {
            bounds_m[0][a] = bounds_m[0][a].min(point_m[a] - radius_m - MARGIN_M);
            bounds_m[1][a] = bounds_m[1][a].max(point_m[a] + radius_m + MARGIN_M);
        }
    }
    bounds_m
}

#[cfg(test)]
mod tests {
    //! Tests of drawing a map from above.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Map | with a boundary, terrain, structures and consoles (Lunar Surface); without one, with boxes (the house) | `lunar_surface_fits_its_boundary`, `house_shows_its_boxes` |
    //! | Pixel | ground, a structure's top (blue and red), a box's top, a console | `lunar_surface_fits_its_boundary`, `house_shows_its_boxes` |
    //! | Player | at the world origin facing −z, which is +u | `player_points_along_u` |
    //! | Other players' count | none, one, several | `player_points_along_u`, `this_player_shows_over_other_players`, `other_players_show_in_their_teams_colour` |
    //! | Other player's team | blue, red | `other_players_show_in_their_teams_colour` |
    //! | Other player's place | within range, beyond it (not dimmed), off the picture (left out), under this player (this player on top) | `other_players_show_in_their_teams_colour`, `other_players_off_the_picture_are_left_out`, `this_player_shows_over_other_players` |
    //! | Other player's height | on the ground, above it (shown at the ground point under it) | `other_players_show_in_their_teams_colour` |
    //! | Pixel and the player's range | within it (as drawn), beyond it (darker), across its edge (a white rim) | `the_range_circle_dims_what_is_beyond` |
    //! | Fermi coordinates | the ground, above it | `fermi_coordinates_round_trip` |

    use super::*;
    use crate::map::get_map_folder;

    const MAX_SIZE_PX: (u32, u32) = (992, 536);
    const RANGE_M: f64 = 30.0;
    const EYE_HEIGHT_M: f64 = 1.7;

    /// `get_rgb` gives a picture's pixel at (x, y).
    ///
    /// Example: `let rgb = get_rgb(&pixels, (992, 536), 10, 20);`
    fn get_rgb(pixels: &[u8], size_px: (u32, u32), x: usize, y: usize) -> [u8; 3] {
        let i: usize = (x + size_px.0 as usize * y) * BYTES_PER_PIXEL;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    }

    #[test]
    fn fermi_coordinates_round_trip() {
        for (u_m, v_m, h_m) in [(0.0, 0.0, 0.0), (90.0, -50.0, 0.0), (-30.0, 60.0, 7.0)] {
            let [u2_m, v2_m, h2_m] =
                get_fermi_coordinates(make_fermi_point(u_m, v_m, h_m, 40.0), 40.0);
            assert!(
                (u2_m - u_m).abs() < 1e-9 && (v2_m - v_m).abs() < 1e-9 && (h2_m - h_m).abs() < 1e-9
            );
        }
    }

    #[test]
    fn lunar_surface_fits_its_boundary() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("Lunar Surface loads");
        let layout: MapLayout = MapLayout::FromMap(&map, MAX_SIZE_PX).expect("a layout");
        let (width, height) = layout.get_size_px();
        assert!(width <= MAX_SIZE_PX.0 && height <= MAX_SIZE_PX.1);
        assert!(width == MAX_SIZE_PX.0 || height == MAX_SIZE_PX.1);
        for (name, team) in [("blue pylon", 0), ("red pylon", 1)] {
            let structure = map
                .structures
                .iter()
                .find(|s| s.name == name)
                .expect("a pylon");
            let [u_m, v_m, _] =
                get_fermi_coordinates(to_frame(&structure.frame).col(AXIS_W), layout.r_m);
            let [x, y] = layout.get_px([u_m, v_m]).map(|p| p as usize);
            let [r, _, b] = get_rgb(&layout.pixels, layout.size_px, x, y);
            assert_eq!(
                b > r,
                team == 0,
                "{name} shows in its team's colour, not {r}, {b}"
            );
        }
        let (point_m, team) = find_console_points(&map, layout.r_m)[0];
        let console_px: [f64; 2] = layout.get_px(point_m);
        assert_eq!(
            get_rgb(
                &layout.pixels,
                layout.size_px,
                console_px[0] as usize,
                console_px[1] as usize
            ),
            TEAM_COLORS[team as usize]
        );
    }

    #[test]
    fn house_shows_its_boxes() {
        let map: Map = Map::Load(&get_map_folder("house_demo")).expect("the house loads");
        let layout: MapLayout = MapLayout::FromMap(&map, MAX_SIZE_PX).expect("a layout");
        let wall_px: [f64; 2] = layout.get_px({
            let [u_m, v_m, _] =
                get_fermi_coordinates(to_frame(&map.boxes[0].frame).col(AXIS_W), layout.r_m);
            [u_m, v_m]
        });
        let corner: [u8; 3] = get_rgb(&layout.pixels, layout.size_px, 0, 0);
        assert_ne!(
            get_rgb(
                &layout.pixels,
                layout.size_px,
                wall_px[0] as usize,
                wall_px[1] as usize
            ),
            corner
        );
    }

    #[test]
    fn the_range_circle_dims_what_is_beyond() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("Lunar Surface loads");
        let layout: MapLayout = MapLayout::FromMap(&map, MAX_SIZE_PX).expect("a layout");
        let pixels: Vec<u8> = layout.get_range_pixels([0.0, 0.0], RANGE_M);
        let brightness = |pixels: &[u8], point_m: [f64; 2]| -> u32 {
            let [x, y] = layout.get_px(point_m).map(|p| p as usize);
            get_rgb(pixels, layout.size_px, x, y)
                .iter()
                .map(|&c| u32::from(c))
                .sum()
        };
        let near_m: [f64; 2] = [RANGE_M / 2.0, -RANGE_M / 2.0];
        assert_eq!(
            brightness(&pixels, near_m),
            brightness(&layout.pixels, near_m)
        );
        let far_m: [f64; 2] = [-2.0 * RANGE_M, RANGE_M];
        assert!(brightness(&pixels, far_m) < brightness(&layout.pixels, far_m));
        let [x, y] = layout.get_px([0.0, 0.0]).map(|p| p as usize);
        let rim_count: usize = (x..layout.size_px.0 as usize)
            .filter(|&x| get_rgb(&pixels, layout.size_px, x, y) == RANGE_RIM_COLOR)
            .count();
        assert!(rim_count >= 1, "a rim crosses the line right of the middle");
    }

    #[test]
    fn player_points_along_u() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("Lunar Surface loads");
        let layout: MapLayout = MapLayout::FromMap(&map, MAX_SIZE_PX).expect("a layout");
        let pixels: Vec<u8> = layout.draw_players(DMat4::IDENTITY, 1, RANGE_M, &[]);
        let [x, y] = layout.get_px([0.0, 0.0]).map(|p| p as usize);
        assert_eq!(get_rgb(&pixels, layout.size_px, x, y), TEAM_COLORS[1]);
        let ahead_x: usize = x + (PLAYER_HEADING_PX - 1.0) as usize;
        assert_eq!(
            get_rgb(&pixels, layout.size_px, ahead_x, y),
            PLAYER_OUTLINE_COLOR
        );
        let behind_x: usize = x - (PLAYER_HEADING_PX - 1.0) as usize;
        assert_ne!(
            get_rgb(&pixels, layout.size_px, behind_x, y),
            PLAYER_OUTLINE_COLOR
        );
    }

    #[test]
    fn other_players_show_in_their_teams_colour() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("Lunar Surface loads");
        let layout: MapLayout = MapLayout::FromMap(&map, MAX_SIZE_PX).expect("a layout");
        let near_m: [f64; 3] = [RANGE_M / 2.0, -RANGE_M / 3.0, 0.0];
        let far_m: [f64; 3] = [-2.0 * RANGE_M, RANGE_M, EYE_HEIGHT_M];
        let others: Vec<(DVec4, u32)> = [(near_m, 0), (far_m, 1)]
            .map(|([u_m, v_m, h_m], team)| (make_fermi_point(u_m, v_m, h_m, layout.r_m), team))
            .to_vec();
        let pixels: Vec<u8> = layout.draw_players(DMat4::IDENTITY, 0, RANGE_M, &others);
        for ([u_m, v_m, _], team) in [(near_m, 0), (far_m, 1)] {
            let center_px: [f64; 2] = layout.get_px([u_m, v_m]);
            let [x, y] = center_px.map(|p| p as usize);
            assert_eq!(get_rgb(&pixels, layout.size_px, x, y), TEAM_COLORS[team]);
            let rim_x: usize =
                (center_px[0] + OTHER_PLAYER_RADIUS_PX + OTHER_PLAYER_OUTLINE_PX / 2.0) as usize;
            assert_eq!(
                get_rgb(&pixels, layout.size_px, rim_x, y),
                OTHER_PLAYER_OUTLINE_COLOR
            );
        }
    }

    #[test]
    fn other_players_off_the_picture_are_left_out() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("Lunar Surface loads");
        let layout: MapLayout = MapLayout::FromMap(&map, MAX_SIZE_PX).expect("a layout");
        let (width, _) = layout.get_size_px();
        let beyond_m: f64 = layout.corner_m[0] + 2.0 * f64::from(width) * layout.m_per_px;
        let others: Vec<(DVec4, u32)> = vec![(make_fermi_point(beyond_m, 0.0, 0.0, layout.r_m), 1)];
        assert!(
            layout.draw_players(DMat4::IDENTITY, 0, RANGE_M, &others)
                == layout.draw_players(DMat4::IDENTITY, 0, RANGE_M, &[])
        );
    }

    #[test]
    fn this_player_shows_over_other_players() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("Lunar Surface loads");
        let layout: MapLayout = MapLayout::FromMap(&map, MAX_SIZE_PX).expect("a layout");
        let others: Vec<(DVec4, u32)> = vec![(DMat4::IDENTITY.col(AXIS_W), 1)];
        let pixels: Vec<u8> = layout.draw_players(DMat4::IDENTITY, 0, RANGE_M, &others);
        let [x, y] = layout.get_px([0.0, 0.0]).map(|p| p as usize);
        assert_eq!(get_rgb(&pixels, layout.size_px, x, y), TEAM_COLORS[0]);
    }
}
