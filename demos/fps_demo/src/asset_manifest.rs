//! `asset_manifest` lists every downloaded asset of the demo, where it comes from, and where it is
//! saved under `.assets/`, so that `download-assets` and the game agree on paths.
//!
//! Interface: `ASSETS_DIR_NAME`, `HDRI`, `SURFACE_TEXTURES`, `LUNAR_SURFACE_TEXTURES`,
//! `PROP_MODELS`, `RIFLE_MODEL`,
//! `ROBOT_MODEL`, `PolyHavenModel`, `PolyHavenTexture`, and the `*_path` functions.
//!
//! Example: `let path = hdri_path(&assets_dir);`
//!
//! Must know:
//! - Limitations: every Poly Haven asset is CC0; the robot is CC0 from the three.js repository.

use std::path::{Path, PathBuf};

/// `ASSETS_DIR_NAME` is the git-ignored folder, inside `demos/fps_demo/`, that holds the assets.
pub const ASSETS_DIR_NAME: &str = ".assets";

/// `POLY_HAVEN_FILES_API_URL` is the Poly Haven endpoint that lists the files of one asset.
pub const POLY_HAVEN_FILES_API_URL: &str = "https://api.polyhaven.com/files";

/// `PolyHavenHdri` is one equirectangular HDR sky from Poly Haven.
pub struct PolyHavenHdri {
    pub id: &'static str,
    pub resolution: &'static str,
}

/// `PolyHavenTexture` is one PBR surface texture set (base color, normal, ARM) from Poly Haven.
pub struct PolyHavenTexture {
    pub id: &'static str,
    pub resolution: &'static str,
}

/// `PolyHavenModel` is one glTF model, with its textures, from Poly Haven.
pub struct PolyHavenModel {
    pub id: &'static str,
    pub resolution: &'static str,
}

/// `DirectDownload` is one file downloaded from a fixed URL.
pub struct DirectDownload {
    pub id: &'static str,
    pub url: &'static str,
    pub file_name: &'static str,
    pub source_page_url: &'static str,
}

/// `HDRI` is the sky that lights the arena; the sun direction is read from its brightest pixel.
pub const HDRI: PolyHavenHdri = PolyHavenHdri {
    id: "kloofendal_48d_partly_cloudy_puresky",
    resolution: "4k",
};

/// `SURFACE_TEXTURES` are the tiling materials of the arena's walls, floors and containers.
pub const SURFACE_TEXTURES: [PolyHavenTexture; 8] = [
    PolyHavenTexture {
        id: "asphalt_02",
        resolution: "2k",
    },
    PolyHavenTexture {
        id: "concrete_floor_worn_001",
        resolution: "2k",
    },
    PolyHavenTexture {
        id: "brick_wall_02",
        resolution: "2k",
    },
    PolyHavenTexture {
        id: "concrete_wall_008",
        resolution: "2k",
    },
    PolyHavenTexture {
        id: "corrugated_iron_02",
        resolution: "2k",
    },
    PolyHavenTexture {
        id: "metal_plate",
        resolution: "2k",
    },
    PolyHavenTexture {
        id: "rusty_metal_02",
        resolution: "2k",
    },
    PolyHavenTexture {
        id: "concrete_pavement_02",
        resolution: "2k",
    },
];

/// `LUNAR_SURFACE_TEXTURES` are the regolith sets of `../hyperbolic_wormholes_rust/`'s Lunar
/// Surface ground (scans of about 3.1 m and 3.2 m of ground); the FPS demo doesn't load them.
pub const LUNAR_SURFACE_TEXTURES: [PolyHavenTexture; 2] = [
    PolyHavenTexture {
        id: "moon_01",
        resolution: "2k",
    },
    PolyHavenTexture {
        id: "moon_meteor_02",
        resolution: "2k",
    },
];

/// `RIFLE_MODEL` is the first-person weapon.
pub const RIFLE_MODEL: PolyHavenModel = PolyHavenModel {
    id: "bolt_action_rifle_7_62",
    resolution: "2k",
};

/// `PROP_MODELS` are the props placed around the arena.
pub const PROP_MODELS: [PolyHavenModel; 15] = [
    PolyHavenModel {
        id: "concrete_road_barrier",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "Barrel_01",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "barrel_03",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "old_military_crate",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "wooden_crate_02",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "metal_trash_can",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "utility_box_01",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "fire_hydrant",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "street_lamp_01",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "exterior_aircon_unit",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "old_tyre",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "portable_generator",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "propane_tank",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "cardboard_box_01",
        resolution: "1k",
    },
    PolyHavenModel {
        id: "mounted_fluorescent_lights",
        resolution: "1k",
    },
];

/// `ROBOT_MODEL` is the enemy bot: a rigged, animated robot.
pub const ROBOT_MODEL: DirectDownload = DirectDownload {
    id: "robot_expressive",
    url: "https://raw.githubusercontent.com/mrdoob/three.js/r170/examples/models/gltf/RobotExpressive/RobotExpressive.glb",
    file_name: "RobotExpressive.glb",
    source_page_url: "https://github.com/mrdoob/three.js/tree/r170/examples/models/gltf/RobotExpressive",
};

/// `hdri_path` returns where the HDRI is saved.
///
/// Example: `hdri_path(Path::new(".assets"))` is `.assets/hdri/<id>_4k.hdr`.
pub fn hdri_path(assets_dir: &Path) -> PathBuf {
    assets_dir
        .join("hdri")
        .join(format!("{}_{}.hdr", HDRI.id, HDRI.resolution))
}

/// `texture_map_path` returns where one map (`diff`, `nor_gl` or `arm`) of a texture set is saved.
///
/// Example: `texture_map_path(dir, &SURFACE_TEXTURES[0], "diff")`.
pub fn texture_map_path(assets_dir: &Path, texture: &PolyHavenTexture, map_name: &str) -> PathBuf {
    assets_dir.join("textures").join(texture.id).join(format!(
        "{}_{}_{}.jpg",
        texture.id, map_name, texture.resolution
    ))
}

/// `model_dir` returns the folder that holds a Poly Haven model's glTF and its textures.
///
/// Example: `model_dir(dir, &RIFLE_MODEL)`.
pub fn model_dir(assets_dir: &Path, model: &PolyHavenModel) -> PathBuf {
    assets_dir.join("models").join(model.id)
}

/// `model_gltf_path` returns where a Poly Haven model's `.gltf` file is saved.
///
/// Example: `model_gltf_path(dir, &RIFLE_MODEL)`.
pub fn model_gltf_path(assets_dir: &Path, model: &PolyHavenModel) -> PathBuf {
    model_dir(assets_dir, model).join(format!("{}_{}.gltf", model.id, model.resolution))
}

/// `direct_download_path` returns where a `DirectDownload` is saved.
///
/// Example: `direct_download_path(dir, &ROBOT_MODEL)`.
pub fn direct_download_path(assets_dir: &Path, download: &DirectDownload) -> PathBuf {
    assets_dir
        .join("models")
        .join(download.id)
        .join(download.file_name)
}

/// `SKETCHFAB_REVOLVER_DIR` is where the Sketchfab "Sci-fi revolver" glTF is unpacked; the demo
/// falls back to the Poly Haven rifle without it.
pub const SKETCHFAB_REVOLVER_DIR: &str = "sketchfab/sci_fi_revolver";

/// `SKETCHFAB_ARMS_DIR` is where the Sketchfab "FPS- Arms" glTF is unpacked.
pub const SKETCHFAB_ARMS_DIR: &str = "sketchfab/fps_arms";

/// `SketchfabModel` is an optional model that needs a Sketchfab account to download.
pub struct SketchfabModel {
    pub uid: &'static str,
    pub dir: &'static str,
    pub title: &'static str,
}

/// `SKETCHFAB_MODELS` are the energy revolver and the arms that hold it.
pub const SKETCHFAB_MODELS: [SketchfabModel; 2] = [
    SketchfabModel {
        uid: "f6bd1cbad4174048bcf02f8e52771adb",
        dir: SKETCHFAB_REVOLVER_DIR,
        title: "Sci-fi revolver by Ramhat",
    },
    SketchfabModel {
        uid: "22274e479c794377b58671f1652f61fc",
        dir: SKETCHFAB_ARMS_DIR,
        title: "FPS- Arms by 3DHaupt",
    },
];

/// `SKETCHFAB_DOWNLOAD_API_URL` is the Sketchfab Download API; `{uid}/download` returns
/// short-lived archive URLs.
pub const SKETCHFAB_DOWNLOAD_API_URL: &str = "https://api.sketchfab.com/v3/models";

/// `sketchfab_gltf_path` returns the `scene.gltf` of a Sketchfab download directory.
///
/// Example:
///     let path: PathBuf = sketchfab_gltf_path(&assets_dir, SKETCHFAB_REVOLVER_DIR);
pub fn sketchfab_gltf_path(assets_dir: &Path, sketchfab_dir: &str) -> PathBuf {
    assets_dir.join(sketchfab_dir).join("scene.gltf")
}
