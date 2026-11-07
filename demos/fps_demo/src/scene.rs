//! `scene` loads every asset into the renderer: the arena's surfaces, the props, the robot,
//! the first-person weapon (the Sketchfab energy revolver and arms, or the Poly Haven rifle if
//! they are missing), and the sky. It also picks exposure and light levels from the sky.
//!
//! Example:
//!     let scene: Scene = Scene::load(&mut renderer, &assets_dir, &arena, &options);

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use glam::{EulerRot, Mat4, Quat, Vec3, Vec4};

use crate::asset_manifest::{self, PolyHavenTexture};
use crate::assets::gltf_model::{Model, ModelOptions, load_model};
use crate::assets::sky::{Sky, load_sky};
use crate::assets::textures::{TextureRequest, TextureSource, decode_textures};
use crate::game::collision::Aabb;
use crate::game::weapon::ViewmodelPose;
use crate::game::world::{Arena, Surface};
use crate::render::{
    AlphaMode, Atmosphere, DrawItem, EnvironmentData, MaterialData, MaterialHandle, MeshHandle,
    OccluderBox, Renderer, TextureFormat, TextureHandle,
};

/// `GROUND_ALBEDO` colors the sky's empty lower hemisphere.
const GROUND_ALBEDO: Vec3 = Vec3::new(0.11, 0.1, 0.09);
/// `MIDDLE_EXPOSURE` is the exposed brightness of a white surface in full daylight.
const MIDDLE_EXPOSURE: f32 = 1.9;
/// `MIN_SUN_PER_SKY_IRRADIANCE_FRAC` is the least ratio of sun to sky light on flat ground; a
/// clear day is about 3 to 5, and HDRIs with a clipped sun come out near 1.
const MIN_SUN_PER_SKY_IRRADIANCE_FRAC: f32 = 3.5;
/// `ROBOT_HEIGHT_M` is the height the robot model is scaled to.
const ROBOT_HEIGHT_M: f32 = 1.85;
/// `SURFACE_TEXTURE_SIZE_PX` is the size limit of arena textures.
const SURFACE_TEXTURE_SIZE_PX: u32 = 2048;
/// `PROP_TEXTURE_SIZE_PX` is the size limit of prop textures.
const PROP_TEXTURE_SIZE_PX: u32 = 1024;
/// `ARMS_POSE_NODE` is the arms model's static pose with both arms reaching forward; the other
/// five poses are display poses.
const ARMS_POSE_NODE: &str = "Arms_Obj.001";

/// `SceneOptions` are the loading choices.
pub struct SceneOptions {
    pub should_use_rifle: bool,
}

/// `Viewmodel` is the first-person weapon: its parts, placement, and where its muzzle is.
pub struct Viewmodel {
    pub weapon: Model,
    pub arms: Option<Model>,
    pub arms_local: Mat4,
    pub weapon_local: Mat4,
    pub pose: ViewmodelPose,
    pub muzzle_local_m: Vec3,
    pub is_energy_weapon: bool,
}

/// `Scene` is everything loaded into the renderer.
pub struct Scene {
    pub static_draws: Vec<DrawItem>,
    pub solids: Vec<Aabb>,
    pub robot: Model,
    pub robot_scale: f32,
    pub robot_clips: [usize; 4],
    pub robot_material_names: Vec<String>,
    pub viewmodel: Viewmodel,
    pub light_scale: f32,
}

/// `surface_texture` returns a surface's Poly Haven texture set.
fn surface_texture(surface: Surface) -> &'static PolyHavenTexture {
    let index: usize = Surface::ALL.iter().position(|s| *s == surface).unwrap_or(0);
    &asset_manifest::SURFACE_TEXTURES[index]
}

/// `create_surface_materials` decodes the arena's texture sets and makes their materials.
fn create_surface_materials(
    renderer: &mut dyn Renderer,
    assets_dir: &Path,
) -> HashMap<Surface, MaterialHandle> {
    let maps: [(&str, TextureFormat); 3] = [
        ("diff", TextureFormat::Rgba8Srgb),
        ("nor_gl", TextureFormat::Rg8Linear),
        ("arm", TextureFormat::Rgba8Linear),
    ];
    let requests: Vec<TextureRequest> = Surface::ALL
        .iter()
        .flat_map(|surface: &Surface| {
            maps.iter().map(move |(map_name, format)| TextureRequest {
                source: TextureSource::File(asset_manifest::texture_map_path(
                    assets_dir,
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
    Surface::ALL
        .iter()
        .enumerate()
        .map(|(index, surface)| {
            let material: MaterialHandle = renderer.create_material(&MaterialData {
                base_color_texture: Some(handles[index * 3]),
                normal_texture: Some(handles[index * 3 + 1]),
                occlusion_roughness_metal_texture: Some(handles[index * 3 + 2]),
                has_occlusion_in_red: true,
                ..MaterialData::default()
            });
            (*surface, material)
        })
        .collect()
}

/// `prop_mesh_filter` keeps one variant of the Poly Haven packs that hold several.
fn prop_mesh_filter(model_id: &str) -> Box<dyn Fn(&str) -> bool> {
    match model_id {
        "fire_hydrant" => Box::new(|name: &str| !name.contains("aged")),
        "metal_trash_can" => Box::new(|name: &str| !name.contains("rust")),
        "old_military_crate" => Box::new(|name: &str| name.ends_with("_a")),
        "exterior_aircon_unit" => Box::new(|name: &str| !name.contains("rusted")),
        "mounted_fluorescent_lights" => Box::new(|name: &str| name.ends_with("_a")),
        _ => Box::new(|_: &str| true),
    }
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
        })
        .collect()
}

/// `world_aabb` returns the world bounds of a model's local bounds under a transform.
fn world_aabb(model: &Model, world: Mat4) -> Aabb {
    let mut min_m: Vec3 = Vec3::splat(f32::MAX);
    let mut max_m: Vec3 = Vec3::splat(f32::MIN);
    for corner in 0..8 {
        let local: Vec3 = Vec3::new(
            if corner & 1 == 0 {
                model.bounds_min_m.x
            } else {
                model.bounds_max_m.x
            },
            if corner & 2 == 0 {
                model.bounds_min_m.y
            } else {
                model.bounds_max_m.y
            },
            if corner & 4 == 0 {
                model.bounds_min_m.z
            } else {
                model.bounds_max_m.z
            },
        );
        let point: Vec3 = world.transform_point3(local);
        min_m = min_m.min(point);
        max_m = max_m.max(point);
    }
    Aabb { min_m, max_m }
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
    let fog_color_rgb: Vec3 = up_irradiance_rgb * Vec3::new(0.95, 0.98, 1.05);
    let atmosphere: Atmosphere = Atmosphere {
        exposure: MIDDLE_EXPOSURE / daylight_radiance.max(1e-4),
        ibl_intensity_frac,
        fog_color_rgb,
        fog_density_per_m: 0.0035,
        fog_height_falloff_per_m: 0.12,
        fog_base_height_m: 0.0,
        fog_sun_inscatter_frac: 0.6,
        fog_max_opacity_frac: 0.7,
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

/// `REVOLVER_ANCHOR_INSET_M` is how far the revolver's pivot sits below its top and ahead of its
/// rear; the pivot is the point the hip and ADS offsets place.
const REVOLVER_ANCHOR_INSET_M: Vec3 = Vec3::new(0.0, 0.02, 0.05);
/// `REVOLVER_BORE_DEPTH_M` is how far the bore sits below the revolver's top.
const REVOLVER_BORE_DEPTH_M: f32 = 0.05;
/// `REVOLVER_HIP_OFFSET_M` is the revolver pivot's camera-space position when hip firing.
const REVOLVER_HIP_OFFSET_M: Vec3 = Vec3::new(0.085, -0.115, -0.27);
/// `REVOLVER_ADS_OFFSET_M` puts the collimator reticle on the screen centre.
const REVOLVER_ADS_OFFSET_M: Vec3 = Vec3::new(0.0, -0.013, -0.22);
/// `ARMS_OFFSET_M` places the right arm's hand behind the revolver grip.
const ARMS_OFFSET_M: Vec3 = Vec3::new(0.03, -0.17, -0.14);
/// `ARMS_SKIN_TINT_RGB` tones down the arms texture's saturated orange skin to match the scene.
const ARMS_SKIN_TINT_RGB: Vec3 = Vec3::new(0.62, 0.58, 0.6);
/// `ARMS_M_PER_MODEL_UNIT` scales the arms model to human size.
const ARMS_M_PER_MODEL_UNIT: f32 = 0.17;
/// `ARMS_MIN_X` keeps only the right arm; the left hand would float in the air.
const ARMS_MIN_X: f32 = 0.0;

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

/// `load_viewmodel` loads the energy revolver and arms, or the rifle as a fallback.
fn load_viewmodel(
    renderer: &mut dyn Renderer,
    assets_dir: &Path,
    options: &SceneOptions,
    emissive_scale: f32,
) -> Viewmodel {
    let revolver_path =
        asset_manifest::sketchfab_gltf_path(assets_dir, asset_manifest::SKETCHFAB_REVOLVER_DIR);
    let arms_path =
        asset_manifest::sketchfab_gltf_path(assets_dir, asset_manifest::SKETCHFAB_ARMS_DIR);
    if revolver_path.exists() && !options.should_use_rifle {
        let weapon: Model = load_model(
            renderer,
            &revolver_path,
            &ModelOptions {
                should_include_mesh_node: &is_railgun_revolver_node,
                should_recenter: false,
                emissive_scale: emissive_scale * 4.0,
                ..ModelOptions::default()
            },
        );
        println!(
            "revolver bounds {:?} .. {:?}",
            weapon.bounds_min_m, weapon.bounds_max_m
        );
        let arms: Option<Model> = arms_path.exists().then(|| {
            let filter = |name: &str| name == ARMS_POSE_NODE;
            let arms: Model = load_model(
                renderer,
                &arms_path,
                &ModelOptions {
                    should_include_mesh_node: &filter,
                    min_x_m: ARMS_MIN_X,
                    base_color_tint_rgb: ARMS_SKIN_TINT_RGB,
                    ..ModelOptions::default()
                },
            );
            println!(
                "arms bounds {:?} .. {:?}",
                arms.bounds_min_m, arms.bounds_max_m
            );
            arms
        });
        let anchor_m: Vec3 = Vec3::new(
            (weapon.bounds_min_m.x + weapon.bounds_max_m.x) * 0.5,
            weapon.bounds_max_m.y - REVOLVER_ANCHOR_INSET_M.y,
            weapon.bounds_min_m.z + REVOLVER_ANCHOR_INSET_M.z,
        );
        let arms_local: Mat4 = Mat4::from_translation(ARMS_OFFSET_M)
            * Mat4::from_scale(Vec3::splat(ARMS_M_PER_MODEL_UNIT));
        let muzzle_local_m: Vec3 = Vec3::new(
            anchor_m.x,
            weapon.bounds_max_m.y - REVOLVER_BORE_DEPTH_M,
            weapon.bounds_max_m.z,
        );
        return Viewmodel {
            weapon,
            arms,
            arms_local,
            weapon_local: Mat4::from_rotation_y(std::f32::consts::PI)
                * Mat4::from_translation(-anchor_m),
            pose: ViewmodelPose {
                hip_offset_m: REVOLVER_HIP_OFFSET_M,
                ads_offset_m: REVOLVER_ADS_OFFSET_M,
                base_rotation: Quat::IDENTITY,
            },
            muzzle_local_m,
            is_energy_weapon: true,
        };
    }
    let weapon: Model = load_model(
        renderer,
        &asset_manifest::model_gltf_path(assets_dir, &asset_manifest::RIFLE_MODEL),
        &ModelOptions {
            should_recenter: false,
            ..ModelOptions::default()
        },
    );
    println!(
        "rifle bounds {:?} .. {:?}",
        weapon.bounds_min_m, weapon.bounds_max_m
    );
    Viewmodel {
        weapon,
        arms: None,
        arms_local: Mat4::IDENTITY,
        weapon_local: Mat4::IDENTITY,
        pose: ViewmodelPose {
            hip_offset_m: Vec3::new(0.14, -0.16, -0.3),
            ads_offset_m: Vec3::new(0.0, -0.1, -0.24),
            base_rotation: Quat::from_euler(EulerRot::YXZ, 0.0, 0.0, 0.0),
        },
        muzzle_local_m: Vec3::new(0.0, 0.0, -0.8),
        is_energy_weapon: false,
    }
}

impl Scene {
    /// `load` loads everything and uploads the environment and occluders.
    ///
    /// Example:
    ///     let scene: Scene = Scene::load(&mut renderer, &assets_dir, &arena, &options);
    pub fn load(
        renderer: &mut dyn Renderer,
        assets_dir: &Path,
        arena: &Arena,
        options: &SceneOptions,
    ) -> Scene {
        let start: Instant = Instant::now();
        let sky: Sky = load_sky(&asset_manifest::hdri_path(assets_dir), GROUND_ALBEDO);
        let (environment, daylight_radiance) = environment_from_sky(sky);
        renderer.set_environment(&environment);
        let emissive_scale: f32 = daylight_radiance;
        let light_scale: f32 = daylight_radiance * 6.0;
        println!(
            "sky and lighting ready in {:.1}s",
            start.elapsed().as_secs_f32()
        );

        let surface_materials: HashMap<Surface, MaterialHandle> =
            create_surface_materials(renderer, assets_dir);
        let mut static_draws: Vec<DrawItem> = Vec::new();
        for (surface, mesh_data) in arena.surface_meshes() {
            let mesh: MeshHandle = renderer.create_mesh(&mesh_data);
            static_draws.push(DrawItem {
                mesh,
                material: surface_materials[&surface],
                model: Mat4::IDENTITY,
                tint: Vec4::ONE,
                joint_offset: None,
                bounds_center_m: Vec3::ZERO,
                bounds_radius_m: 1.0e4,
                does_cast_shadow: true,
            });
        }
        let mut solids: Vec<Aabb> = arena.boxes.iter().map(|world_box| world_box.aabb).collect();
        let occluders: Vec<OccluderBox> = arena
            .boxes
            .iter()
            .skip(1)
            .map(|world_box| OccluderBox {
                min_m: world_box.aabb.min_m,
                max_m: world_box.aabb.max_m,
            })
            .collect();
        renderer.set_occluders(&occluders, arena.bounds_min_m, arena.bounds_max_m);
        println!("arena ready in {:.1}s", start.elapsed().as_secs_f32());

        let mut props: HashMap<&str, Model> = HashMap::new();
        for prop in &asset_manifest::PROP_MODELS {
            let filter: Box<dyn Fn(&str) -> bool> = prop_mesh_filter(prop.id);
            let model: Model = load_model(
                renderer,
                &asset_manifest::model_gltf_path(assets_dir, prop),
                &ModelOptions {
                    should_include_mesh_node: filter.as_ref(),
                    max_texture_size_px: PROP_TEXTURE_SIZE_PX,
                    emissive_scale: emissive_scale * 3.0,
                    ..ModelOptions::default()
                },
            );
            props.insert(prop.id, model);
        }
        for placement in &arena.props {
            let model: &Model = &props[placement.model_id];
            let world: Mat4 = Mat4::from_rotation_translation(
                Quat::from_rotation_y(placement.yaw_rad),
                placement.position_m,
            );
            static_draws.extend(draw_items_for(model, world, true));
            if placement.is_solid {
                solids.push(world_aabb(model, world));
            }
        }
        let fluorescent: &Model = &props["mounted_fluorescent_lights"];
        for light in &arena.lights {
            if light.color_rgb.z > 0.6 {
                let top_m: f32 = fluorescent.bounds_max_m.y;
                let world: Mat4 =
                    Mat4::from_translation(light.position_m + Vec3::Y * (0.6 - top_m))
                        * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2);
                static_draws.extend(draw_items_for(fluorescent, world, false));
            }
        }
        println!("props ready in {:.1}s", start.elapsed().as_secs_f32());

        let robot_path =
            asset_manifest::direct_download_path(assets_dir, &asset_manifest::ROBOT_MODEL);
        let robot: Model = load_model(
            renderer,
            &robot_path,
            &ModelOptions {
                is_animated: true,
                should_recenter: false,
                ..ModelOptions::default()
            },
        );
        let robot_height_m: f32 = robot.bounds_max_m.y - robot.bounds_min_m.y;
        let robot_scale: f32 = ROBOT_HEIGHT_M / robot_height_m.max(1e-3);
        let skeleton = robot.skeleton.as_ref().expect("the robot is animated");
        let clip = |name: &str| skeleton.animation_index(name).unwrap_or(0);
        let robot_clips: [usize; 4] = [
            clip("Idle"),
            clip("Walking"),
            clip("Running"),
            clip("Death"),
        ];
        let robot_material_names: Vec<String> = robot.material_names.clone();
        println!(
            "robot bounds {:?} .. {:?}",
            robot.bounds_min_m, robot.bounds_max_m
        );

        let viewmodel: Viewmodel = load_viewmodel(renderer, assets_dir, options, emissive_scale);
        println!("all assets ready in {:.1}s", start.elapsed().as_secs_f32());
        let _ = AlphaMode::Opaque;
        Scene {
            static_draws,
            solids,
            robot,
            robot_scale,
            robot_clips,
            robot_material_names,
            viewmodel,
            light_scale,
        }
    }
}
