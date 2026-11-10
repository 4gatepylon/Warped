//! `scene` loads every asset into the renderer: the arena's surfaces, the props, the robot,
//! both guns in hand as Lunar Surface draws them (the Poly Haven bolt-action rifle for the sniper
//! and the Sketchfab energy revolver for the rifle), and the sky. It also picks exposure and light levels from the sky.
//!
//! Example:
//!     let scene: Scene = Scene::load(&mut renderer, &assets_dir, &textures_dir, &arena);

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::{Mat4, Vec3, Vec4};

use warped_shared::app_bundle::get_model_path;
use warped_shared::avatar_model::{AvatarClip, BODY_MODEL};

use crate::asset_manifest::{self, PolyHavenTexture};
use crate::assets::gltf_model::{Model, ModelOptions, load_model};
use crate::assets::sky::{Sky, load_sky};
use crate::assets::textures::{TextureRequest, TextureSource, decode_textures};
use crate::game::collision::Aabb;
use crate::game::guns::GunModel;
use crate::game::world::{Arena, FAR_LEAF_DETAIL, NEAR_LEAF_DETAIL, Surface};
use crate::render::{
    AlphaMode, Atmosphere, DrawItem, EnvironmentData, MaterialData, MaterialHandle, MeshData,
    MeshHandle, OccluderBox, Renderer, TextureFormat, TextureHandle,
    tiles::{MeshTile, split_mesh_into_tiles},
};

/// `GROUND_ALBEDO` colors the sky's empty lower hemisphere.
const GROUND_ALBEDO: Vec3 = Vec3::new(0.11, 0.1, 0.09);
/// `MIDDLE_EXPOSURE` is the exposed brightness of a white surface in full daylight.
const MIDDLE_EXPOSURE: f32 = 1.9;
/// `SWAMP_DIM_FRAC` darkens the exposure below full daylight for the dim, swampy mood.
const SWAMP_DIM_FRAC: f32 = 0.6;
/// `SWAMP_FOG_TINT_RGB` turns the sky-coloured haze murky green.
const SWAMP_FOG_TINT_RGB: Vec3 = Vec3::new(0.8, 0.95, 0.75);
/// `SWAMP_SKY_TINT_RGB` grades the clear-day HDRI towards a murky green.
const SWAMP_SKY_TINT_RGB: Vec3 = Vec3::new(0.72, 0.88, 0.66);
/// `SWAMP_SKY_SATURATION_FRAC` is how much of the HDRI's colour saturation the grade keeps.
const SWAMP_SKY_SATURATION_FRAC: f32 = 0.35;
/// `SWAMP_SUN_FRAC` weakens the sun against the sky, for softer, hazier shadows.
const SWAMP_SUN_FRAC: f32 = 0.55;
/// `MIN_SUN_PER_SKY_IRRADIANCE_FRAC` is the least ratio of sun to sky light on flat ground; a
/// clear day is about 3 to 5, and HDRIs with a clipped sun come out near 1.
const MIN_SUN_PER_SKY_IRRADIANCE_FRAC: f32 = 3.5;
/// `ROBOT_HEIGHT_M` is the height the players' and robots' body is drawn at: Lunar Surface's
/// `AVATAR_HEIGHT_M`, so the teal robot is the same size in every level.
const ROBOT_HEIGHT_M: f32 = 2.0;
/// `ROBOT_EMISSIVE_SCALE` dims the body model's glowing face (its glTF's emissive strength of 10)
/// so the forest's bloom shows its eyes and mouth in their yellow, as Lunar Surface draws them,
/// rather than as a white star.
const ROBOT_EMISSIVE_SCALE: f32 = 0.1;
/// `ROBOT_CLIPS` are the body's clips the robots play, in the order `Scene::robot_clips` keeps
/// their animations.
const ROBOT_CLIPS: [AvatarClip; 5] = [
    AvatarClip::Idle,
    AvatarClip::Walk,
    AvatarClip::Run,
    AvatarClip::Jump,
    AvatarClip::Death,
];
/// `SURFACE_TEXTURE_SIZE_PX` is the size limit of arena textures.
const SURFACE_TEXTURE_SIZE_PX: u32 = 2048;
/// `STATIC_TILE_M` is the width of the tiles the arena's surfaces are cut into, so each pass
/// draws only the tiles in its view.
const STATIC_TILE_M: f32 = 8.0;

/// `GunViewmodel` is a gun model drawn in hand: its parts, and `muzzle_from_gltf`, which takes its
/// glTF's space to its muzzle's (the muzzle at the origin, the barrel along -z) at its size in hand,
/// as Lunar Surface's `viewmodel.rs` places it.
pub struct GunViewmodel {
    pub model: Model,
    pub muzzle_from_gltf: Mat4,
}

/// `Scene` is everything loaded into the renderer; `far_leaf_meshes` gives, by the number of each
/// leaf tile's mesh (at `NEAR_LEAF_DETAIL`), that tile's mesh at `FAR_LEAF_DETAIL`, to draw from
/// afar (by mesh, so leaving draws out of `static_draws`, as `--hide-canopy` does, keeps it right).
pub struct Scene {
    pub static_draws: Vec<DrawItem>,
    pub static_surfaces: Vec<Surface>,
    pub far_leaf_meshes: HashMap<u32, MeshHandle>,
    pub solids: Vec<Aabb>,
    pub robot: Model,
    pub robot_scale: f32,
    pub robot_clips: [usize; ROBOT_CLIPS.len()],
    pub robot_material_names: Vec<String>,
    pub revolver: GunViewmodel,
    pub bolt_action_rifle: GunViewmodel,
    pub light_scale: f32,
    pub beacon: Model,
}

/// `surface_texture` returns a surface's Poly Haven texture set.
fn surface_texture(surface: Surface) -> &'static PolyHavenTexture {
    let index: usize = Surface::ALL.iter().position(|s| *s == surface).unwrap_or(0);
    &asset_manifest::SURFACE_TEXTURES[index]
}

/// `create_surface_materials` decodes the forest's texture sets and makes the materials of
/// every surface: the textured ones as they are, rope and paints as tinted bark and planks, and
/// consoles as glowing, untextured.
fn create_surface_materials(
    renderer: &mut dyn Renderer,
    textures_dir: &Path,
    emissive_scale: f32,
) -> HashMap<Surface, MaterialHandle> {
    let maps: [(&str, TextureFormat); 3] = [
        ("diff", TextureFormat::Rgba8Srgb),
        ("nor_gl", TextureFormat::Rg8Linear),
        ("arm", TextureFormat::Rgba8Linear),
    ];
    let requests: Vec<TextureRequest> = Surface::ALL[..Surface::TEXTURED_COUNT]
        .iter()
        .flat_map(|surface: &Surface| {
            maps.iter().map(move |(map_name, format)| TextureRequest {
                source: TextureSource::File(asset_manifest::texture_map_path(
                    textures_dir,
                    surface_texture(*surface),
                    map_name,
                )),
                format: *format,
                max_size_px: SURFACE_TEXTURE_SIZE_PX,
            })
        })
        .collect();
    let handles: Vec<TextureHandle> = decode_textures(&requests)
        .iter()
        .map(|texture| renderer.create_texture(texture))
        .collect();
    let textured = |index: usize, base_color_factor: Vec4| MaterialData {
        base_color_texture: Some(handles[index * 3]),
        normal_texture: Some(handles[index * 3 + 1]),
        occlusion_roughness_metal_texture: Some(handles[index * 3 + 2]),
        has_occlusion_in_red: true,
        base_color_factor,
        ..MaterialData::default()
    };
    let glowing = |color: Vec3| MaterialData {
        base_color_factor: (color * 0.2).extend(1.0),
        emissive_factor: color * emissive_scale * 2.0,
        roughness_factor: 0.4,
        ..MaterialData::default()
    };
    Surface::ALL
        .iter()
        .enumerate()
        .map(|(index, surface)| {
            let data: MaterialData = match surface {
                Surface::Rope => textured(1, Vec4::new(1.9, 1.6, 1.1, 1.0)),
                Surface::RedPaint => textured(2, Vec4::new(1.5, 0.35, 0.25, 1.0)),
                Surface::BluePaint => textured(2, Vec4::new(0.35, 0.55, 1.6, 1.0)),
                Surface::RedConsole => glowing(Vec3::new(1.0, 0.18, 0.1)),
                Surface::BlueConsole => glowing(Vec3::new(0.15, 0.4, 1.0)),
                Surface::Horizon => glowing(Vec3::ZERO),
                Surface::Accretion => glowing(Vec3::new(0.6, 0.22, 0.06)),
                Surface::Throat => glowing(Vec3::new(0.16, 0.08, 0.45)),
                Surface::Wax => MaterialData {
                    base_color_factor: Vec4::new(0.85, 0.8, 0.65, 1.0),
                    roughness_factor: 0.6,
                    ..MaterialData::default()
                },
                Surface::Flame => glowing(Vec3::new(1.6, 0.8, 0.3)),
                Surface::Ground => textured(index, Vec4::new(0.6, 0.66, 0.5, 1.0)),
                Surface::Bark => textured(index, Vec4::new(0.78, 0.84, 0.62, 1.0)),
                Surface::Leaves => textured(index, Vec4::new(0.55, 0.78, 0.42, 1.0)),
                _ => textured(index, Vec4::ONE),
            };
            (*surface, renderer.create_material(&data))
        })
        .collect()
}

/// `draw_items_for` makes the draws of one placed model.
pub fn draw_items_for(model: &Model, world: Mat4, does_cast_shadow: bool) -> Vec<DrawItem> {
    let center: Vec3 = world.transform_point3((model.bounds_min_m + model.bounds_max_m) * 0.5);
    let scale: f32 = world.x_axis.truncate().length();
    let radius_m: f32 = (model.bounds_max_m - model.bounds_min_m).length() * 0.5 * scale;
    model
        .parts
        .iter()
        .map(|part| DrawItem {
            mesh: part.mesh,
            material: part.material,
            model: world,
            tint: Vec4::ONE,
            joint_offset: None,
            bounds_center_m: center,
            bounds_radius_m: radius_m,
            does_cast_shadow,
            body_flash_frac: None,
        })
        .collect()
}

/// `grade_sky_for_swamp` desaturates and tints the sky, and weakens its sun, for the swampy mood.
///
/// Example: `grade_sky_for_swamp(&mut sky);`
fn grade_sky_for_swamp(sky: &mut Sky) {
    let grade = |rgb: Vec3| {
        let grey: f32 = rgb.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        Vec3::splat(grey).lerp(rgb, SWAMP_SKY_SATURATION_FRAC) * SWAMP_SKY_TINT_RGB
    };
    for pixel in sky.rgb_pixels.as_chunks_mut::<3>().0 {
        let rgb: Vec3 = grade(Vec3::new(pixel[0], pixel[1], pixel[2]));
        pixel.copy_from_slice(&rgb.to_array());
    }
    for coefficient in &mut sky.irradiance_sh9 {
        *coefficient = grade(*coefficient);
    }
    sky.sun_illuminance_rgb = grade(sky.sun_illuminance_rgb) * SWAMP_SUN_FRAC;
}

/// `environment_from_sky` builds the renderer's environment and picks exposure from the sky.
fn environment_from_sky(sky: Sky) -> (EnvironmentData, f32) {
    let up_irradiance_rgb: Vec3 = {
        let n: Vec3 = Vec3::Y;
        let basis: [f32; 9] = [
            0.282095,
            0.488603 * n.y,
            0.488603 * n.z,
            0.488603 * n.x,
            0.0,
            0.0,
            0.315392 * (3.0 * n.z * n.z - 1.0),
            0.0,
            0.546274 * (n.x * n.x - n.y * n.y),
        ];
        sky.irradiance_sh9
            .iter()
            .zip(basis)
            .map(|(c, b)| *c * b)
            .sum::<Vec3>()
    };
    let luminance = |color: Vec3| color.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    let sun_radiance: f32 =
        luminance(sky.sun_illuminance_rgb) * sky.sun_direction.y / std::f32::consts::PI;
    let ibl_intensity_frac: f32 = (sun_radiance
        / (MIN_SUN_PER_SKY_IRRADIANCE_FRAC * luminance(up_irradiance_rgb)).max(1e-4))
    .min(1.0);
    let daylight_radiance: f32 = sun_radiance + luminance(up_irradiance_rgb) * ibl_intensity_frac;
    println!(
        "sky: sun {:?} illuminance {:.1}, sky irradiance/pi {:.2}, white-surface radiance {:.2}",
        sky.sun_direction,
        luminance(sky.sun_illuminance_rgb),
        luminance(up_irradiance_rgb),
        daylight_radiance
    );
    let fog_color_rgb: Vec3 = up_irradiance_rgb * SWAMP_FOG_TINT_RGB;
    let atmosphere: Atmosphere = Atmosphere {
        exposure: SWAMP_DIM_FRAC * MIDDLE_EXPOSURE / daylight_radiance.max(1e-4),
        ibl_intensity_frac,
        fog_color_rgb,
        fog_density_per_m: 0.012,
        fog_height_falloff_per_m: 0.12,
        fog_base_height_m: 0.0,
        fog_sun_inscatter_frac: 0.6,
        fog_max_opacity_frac: 0.85,
        bounce_color_rgb: up_irradiance_rgb * Vec3::new(0.36, 0.32, 0.28),
        bloom_mix_frac: 0.06,
    };
    let environment: EnvironmentData = EnvironmentData {
        width: sky.width,
        height: sky.height,
        rgb_pixels: sky.rgb_pixels,
        irradiance_sh9: sky.irradiance_sh9,
        sun_direction: sky.sun_direction,
        sun_illuminance_rgb: sky.sun_illuminance_rgb,
        sun_angular_radius_rad: sky.sun_angular_radius_rad,
        atmosphere,
    };
    (environment, daylight_radiance)
}

/// `REVOLVER_SCALE` and `BOLT_ACTION_RIFLE_SCALE` are how many times their modelled size the
/// guns are drawn in hand, `BOLT_ACTION_RIFLE_TURN_RAD` turns the rifle so that its barrel, along
/// +x in its glTF, points along +z like the revolver's, and `MUZZLE_SLICE_M` is how far behind a gun
/// model's front its vertices count as the muzzle's face: Lunar Surface's `viewmodel.rs`.
const REVOLVER_SCALE: f32 = 1.6;
const BOLT_ACTION_RIFLE_SCALE: f32 = 0.28;
const BOLT_ACTION_RIFLE_TURN_RAD: f32 = -std::f32::consts::FRAC_PI_2;
const MUZZLE_SLICE_M: f32 = 0.01;
/// `REVOLVER_EMISSIVE_PER_SCENE` brightens the revolver's glowing parts against the scene's.
const REVOLVER_EMISSIVE_PER_SCENE: f32 = 4.0;

/// `RAILGUN_REVOLVER_NODE_PARTS` are name parts of the revolver's railgun variant; the file also
/// holds a ballistic variant beside it.
const RAILGUN_REVOLVER_NODE_PARTS: [&str; 8] = [
    "Drum",
    "BulletsC",
    "Extractor",
    "Receiver_1",
    "RailGun",
    "Glass",
    "Collimator",
    "VATS",
];

/// `is_railgun_revolver_node` says whether a revolver mesh node belongs to the railgun variant.
fn is_railgun_revolver_node(name: &str) -> bool {
    RAILGUN_REVOLVER_NODE_PARTS
        .iter()
        .any(|part| name.contains(part))
}

/// `get_muzzle_m` gives a gun model's muzzle in model space: the middle of its front face (the
/// vertices within `MUZZLE_SLICE_M` of its frontmost, the largest z), on that front.
///
/// Example: `let muzzle_m: Vec3 = get_muzzle_m(&positions_m);`
fn get_muzzle_m(positions_m: &[Vec3]) -> Vec3 {
    let front_z_m: f32 = positions_m.iter().map(|p| p.z).fold(f32::MIN, f32::max);
    let (face_min_m, face_max_m) = positions_m
        .iter()
        .filter(|p| p.z > front_z_m - MUZZLE_SLICE_M)
        .fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
    ((face_min_m + face_max_m) * 0.5).with_z(front_z_m)
}

/// `get_muzzle_from_model` takes a gun model's space to its muzzle's: `muzzle_m` to the origin,
/// the barrel along -z (the camera's ahead), drawn `scale` times its modelled size.
///
/// Example: `let muzzle_from_model: Mat4 = get_muzzle_from_model(muzzle_m, 1.6);`
fn get_muzzle_from_model(muzzle_m: Vec3, scale: f32) -> Mat4 {
    Mat4::from_rotation_y(std::f32::consts::PI)
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_translation(-muzzle_m)
}

/// `load_gun_viewmodel` loads `gun_model` from the fps_demo's assets and finds its muzzle, as
/// Lunar Surface's `Viewmodel::Load` does: the revolver's railgun parts drawn 1.6 times their size,
/// or the whole bolt-action rifle, turned, at 0.28 times.
///
/// Example: `let revolver = load_gun_viewmodel(renderer, assets_dir, GunModel::Revolver, 1.0);`
fn load_gun_viewmodel(
    renderer: &mut dyn Renderer,
    assets_dir: &Path,
    gun_model: GunModel,
    emissive_scale: f32,
) -> GunViewmodel {
    let (path, turn_rad, scale, emissive_scale): (PathBuf, f32, f32, f32) = match gun_model {
        GunModel::Revolver => (
            asset_manifest::sketchfab_gltf_path(assets_dir, asset_manifest::SKETCHFAB_REVOLVER_DIR),
            0.0,
            REVOLVER_SCALE,
            emissive_scale * REVOLVER_EMISSIVE_PER_SCENE,
        ),
        GunModel::BoltActionRifle => (
            asset_manifest::model_gltf_path(assets_dir, &asset_manifest::RIFLE_MODEL),
            BOLT_ACTION_RIFLE_TURN_RAD,
            BOLT_ACTION_RIFLE_SCALE,
            emissive_scale,
        ),
    };
    let model: Model = load_model(
        renderer,
        &path,
        &ModelOptions {
            should_include_mesh_node: match gun_model {
                GunModel::Revolver => &is_railgun_revolver_node,
                GunModel::BoltActionRifle => &|_: &str| true,
            },
            should_recenter: false,
            emissive_scale,
            should_keep_positions: true,
            ..ModelOptions::default()
        },
    );
    let turn: Mat4 = Mat4::from_rotation_y(turn_rad);
    let turned_m: Vec<Vec3> = model
        .positions_m
        .iter()
        .map(|position_m| turn.transform_point3(*position_m))
        .collect();
    GunViewmodel {
        muzzle_from_gltf: get_muzzle_from_model(get_muzzle_m(&turned_m), scale) * turn,
        model,
    }
}

impl Scene {
    /// `get_robot_clip` gives the index of `clip`'s animation in the robot's skeleton.
    ///
    /// Example: `let walk: usize = scene.get_robot_clip(AvatarClip::Walk);`
    pub fn get_robot_clip(&self, clip: AvatarClip) -> usize {
        self.robot_clips[ROBOT_CLIPS
            .iter()
            .position(|&c| c == clip)
            .expect("every clip the robots play is listed")]
    }

    /// `load` loads everything and uploads the environment and occluders.
    ///
    /// Example:
    ///     let scene: Scene = Scene::load(&mut renderer, &assets_dir, &textures_dir, &arena);
    pub fn load(
        renderer: &mut dyn Renderer,
        assets_dir: &Path,
        textures_dir: &Path,
        arena: &Arena,
    ) -> Scene {
        let start: Instant = Instant::now();
        let mut sky: Sky = load_sky(&asset_manifest::hdri_path(assets_dir), GROUND_ALBEDO);
        grade_sky_for_swamp(&mut sky);
        let (environment, daylight_radiance) = environment_from_sky(sky);
        renderer.set_environment(&environment);
        let emissive_scale: f32 = daylight_radiance;
        let light_scale: f32 = daylight_radiance * 6.0;
        println!(
            "sky and lighting ready in {:.1}s",
            start.elapsed().as_secs_f32()
        );

        let surface_materials: HashMap<Surface, MaterialHandle> =
            create_surface_materials(renderer, textures_dir, emissive_scale);
        let mut static_draws: Vec<DrawItem> = Vec::new();
        let mut static_surfaces: Vec<Surface> = Vec::new();
        let mut far_leaf_meshes: HashMap<u32, MeshHandle> = HashMap::new();
        for (surface, mesh_data) in arena.surface_meshes() {
            if surface == Surface::Leaves {
                let near_tiles: Vec<MeshData> =
                    arena.leaf_tile_meshes(STATIC_TILE_M, NEAR_LEAF_DETAIL);
                let far_tiles: Vec<MeshData> =
                    arena.leaf_tile_meshes(STATIC_TILE_M, FAR_LEAF_DETAIL);
                for (near, far) in near_tiles.into_iter().zip(far_tiles) {
                    let tile: MeshTile = MeshTile::Bounding(near);
                    let mesh: MeshHandle = renderer.create_mesh(&tile.mesh);
                    far_leaf_meshes.insert(mesh.0, renderer.create_mesh(&far));
                    static_surfaces.push(surface);
                    static_draws.push(DrawItem {
                        mesh,
                        material: surface_materials[&surface],
                        model: Mat4::IDENTITY,
                        tint: Vec4::ONE,
                        joint_offset: None,
                        bounds_center_m: tile.bounds_center_m,
                        bounds_radius_m: tile.bounds_radius_m,
                        does_cast_shadow: true,
                        body_flash_frac: None,
                    });
                }
                continue;
            }
            for tile in split_mesh_into_tiles(&mesh_data, STATIC_TILE_M) {
                static_surfaces.push(surface);
                let mesh: MeshHandle = renderer.create_mesh(&tile.mesh);
                static_draws.push(DrawItem {
                    mesh,
                    material: surface_materials[&surface],
                    model: Mat4::IDENTITY,
                    tint: Vec4::ONE,
                    joint_offset: None,
                    bounds_center_m: tile.bounds_center_m,
                    bounds_radius_m: tile.bounds_radius_m,
                    does_cast_shadow: true,
                    body_flash_frac: None,
                });
            }
        }
        let solids: Vec<Aabb> = arena.boxes.iter().map(|world_box| world_box.aabb).collect();
        let occluders: Vec<OccluderBox> = arena
            .occluders()
            .iter()
            .map(|aabb| OccluderBox {
                min_m: aabb.min_m,
                max_m: aabb.max_m,
            })
            .collect();
        renderer.set_occluders(&occluders, arena.bounds_min_m, arena.bounds_max_m);
        println!("arena ready in {:.1}s", start.elapsed().as_secs_f32());

        let robot_path: PathBuf = get_model_path(BODY_MODEL);
        let robot: Model = load_model(
            renderer,
            &robot_path,
            &ModelOptions {
                is_animated: true,
                should_recenter: false,
                emissive_scale: ROBOT_EMISSIVE_SCALE,
                ..ModelOptions::default()
            },
        );
        let robot_height_m: f32 = robot.bounds_max_m.y - robot.bounds_min_m.y;
        let robot_scale: f32 = ROBOT_HEIGHT_M / robot_height_m.max(1e-3);
        let skeleton = robot.skeleton.as_ref().expect("the robot is animated");
        let clip = |name: &str| skeleton.animation_index(name).unwrap_or(0);
        let robot_clips: [usize; ROBOT_CLIPS.len()] = ROBOT_CLIPS.map(|c| clip(c.get_name()));
        let robot_material_names: Vec<String> = robot.material_names.clone();
        println!(
            "robot bounds {:?} .. {:?}",
            robot.bounds_min_m, robot.bounds_max_m
        );

        let revolver: GunViewmodel =
            load_gun_viewmodel(renderer, assets_dir, GunModel::Revolver, emissive_scale);
        let bolt_action_rifle: GunViewmodel = load_gun_viewmodel(
            renderer,
            assets_dir,
            GunModel::BoltActionRifle,
            emissive_scale,
        );
        let beacon_path: PathBuf = crate::map::default_map_path()
            .parent()
            .and_then(Path::parent)
            .expect("the default map sits in common/maps/<map>/")
            .join("models/scifi_beacon/scene.gltf");
        let beacon: Model = load_model(
            renderer,
            &beacon_path,
            &ModelOptions {
                should_recenter: false,
                emissive_scale,
                ..ModelOptions::default()
            },
        );
        println!("all assets ready in {:.1}s", start.elapsed().as_secs_f32());
        let _ = AlphaMode::Opaque;
        Scene {
            static_draws,
            static_surfaces,
            far_leaf_meshes,
            solids,
            robot,
            robot_scale,
            robot_clips,
            robot_material_names,
            revolver,
            bolt_action_rifle,
            light_scale,
            beacon,
        }
    }
}
