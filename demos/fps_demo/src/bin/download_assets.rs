//! `download-assets` downloads every asset in `asset_manifest` into `demos/fps_demo/.assets/`.
//!
//! Interface: `--dry-run` prints the files it would download and downloads nothing. Files that
//! already exist with the expected size are skipped, so a second run downloads nothing. The two
//! Sketchfab models are optional: `download-assets` fetches them only when `SKETCHFAB_API_TOKEN`
//! is set, and otherwise says where to unzip them by hand.
//!
//! Example: `cargo run --release --bin download-assets -- --dry-run`
//!
//! Must know:
//! - Side effects: `download-assets` writes only inside `.assets/`, which `make clean` keeps.

#[path = "../asset_manifest.rs"]
#[allow(dead_code)]
mod asset_manifest;

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use clap::Parser;
use serde_json::Value;

use asset_manifest::{
    HDRI, POLY_HAVEN_FILES_API_URL, PROP_MODELS, PolyHavenModel, RIFLE_MODEL, ROBOT_MODEL,
    SKETCHFAB_DOWNLOAD_API_URL, SKETCHFAB_MODELS, SURFACE_TEXTURES, SketchfabModel,
};

const USER_AGENT: &str = "warped-fps-demo-asset-downloader";
const SKETCHFAB_TOKEN_ENV: &str = "SKETCHFAB_API_TOKEN";
const TEXTURE_MAP_API_NAMES: [(&str, &str); 3] =
    [("Diffuse", "diff"), ("nor_gl", "nor_gl"), ("arm", "arm")];

/// `Arguments` is the command line.
#[derive(Parser)]
#[command(
    about = "Download the demo's CC0 assets into .assets/.",
    after_help = "Example: download-assets --dry-run"
)]
struct Arguments {
    /// Print what would be downloaded and download nothing.
    #[arg(long)]
    dry_run: bool,
}

/// `PlannedFile` is one file to download.
struct PlannedFile {
    url: String,
    path: PathBuf,
    expected_size_bytes: Option<u64>,
}

/// `fetch_json` downloads and parses one JSON document.
///
/// Example: `fetch_json("https://api.polyhaven.com/files/asphalt_02")`.
fn fetch_json(url: &str) -> Value {
    ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .unwrap_or_else(|error| panic!("GET {url} failed: {error}"))
        .into_json()
        .unwrap_or_else(|error| panic!("GET {url} did not return JSON: {error}"))
}

/// `plan_file` turns one Poly Haven `{url, size}` entry into a `PlannedFile`.
///
/// Example: `plan_file(&files["hdri"]["4k"]["hdr"], path)`.
fn plan_file(entry: &Value, path: PathBuf) -> PlannedFile {
    let url: &str = entry["url"]
        .as_str()
        .unwrap_or_else(|| panic!("no url in {entry}"));
    PlannedFile {
        url: url.to_string(),
        path,
        expected_size_bytes: entry["size"].as_u64(),
    }
}

/// `plan_model` lists the `.gltf`, `.bin` and texture files of one Poly Haven model.
///
/// Example: `plan_model(assets_dir, &RIFLE_MODEL)`.
fn plan_model(assets_dir: &Path, model: &PolyHavenModel) -> Vec<PlannedFile> {
    let files: Value = fetch_json(&format!("{POLY_HAVEN_FILES_API_URL}/{}", model.id));
    let gltf_entry: &Value = &files["gltf"][model.resolution]["gltf"];
    let mut planned_files: Vec<PlannedFile> = vec![plan_file(
        gltf_entry,
        asset_manifest::model_gltf_path(assets_dir, model),
    )];
    let includes = gltf_entry["include"]
        .as_object()
        .unwrap_or_else(|| panic!("model {} has no include list", model.id));
    for (relative_path, entry) in includes {
        assert!(
            Path::new(relative_path)
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
            "model {} include path {relative_path:?} must stay inside its folder",
            model.id
        );
        let path: PathBuf = asset_manifest::model_dir(assets_dir, model).join(relative_path);
        planned_files.push(plan_file(entry, path));
    }
    planned_files
}

/// `plan_downloads` lists every file of the manifest.
///
/// Example: `let planned_files = plan_downloads(Path::new(".assets"));`
fn plan_downloads(assets_dir: &Path) -> Vec<PlannedFile> {
    let mut planned_files: Vec<PlannedFile> = Vec::new();
    let hdri_files: Value = fetch_json(&format!("{POLY_HAVEN_FILES_API_URL}/{}", HDRI.id));
    planned_files.push(plan_file(
        &hdri_files["hdri"][HDRI.resolution]["hdr"],
        asset_manifest::hdri_path(assets_dir),
    ));
    for texture in &SURFACE_TEXTURES {
        let files: Value = fetch_json(&format!("{POLY_HAVEN_FILES_API_URL}/{}", texture.id));
        for (api_name, map_name) in TEXTURE_MAP_API_NAMES {
            planned_files.push(plan_file(
                &files[api_name][texture.resolution]["jpg"],
                asset_manifest::texture_map_path(assets_dir, texture, map_name),
            ));
        }
    }
    planned_files.extend(plan_model(assets_dir, &RIFLE_MODEL));
    for model in &PROP_MODELS {
        planned_files.extend(plan_model(assets_dir, model));
    }
    planned_files.push(PlannedFile {
        url: ROBOT_MODEL.url.to_string(),
        path: asset_manifest::direct_download_path(assets_dir, &ROBOT_MODEL),
        expected_size_bytes: None,
    });
    planned_files
}

/// `is_downloaded` tells whether a planned file already exists with the expected size.
///
/// Example: `if is_downloaded(&planned_file) { continue; }`
fn is_downloaded(planned_file: &PlannedFile) -> bool {
    match (
        fs::metadata(&planned_file.path),
        planned_file.expected_size_bytes,
    ) {
        (Ok(metadata), Some(expected_size_bytes)) => metadata.len() == expected_size_bytes,
        (Ok(metadata), None) => metadata.len() > 0,
        (Err(_), _) => false,
    }
}

/// `download_file` downloads one file, writing it to a temporary name first so that an
/// interrupted download never looks complete.
///
/// Example: `download_file(&planned_file);`
fn download_file(planned_file: &PlannedFile) {
    let parent_dir: &Path = planned_file
        .path
        .parent()
        .expect("asset paths have a parent");
    fs::create_dir_all(parent_dir).unwrap_or_else(|error| panic!("mkdir {parent_dir:?}: {error}"));
    let response = ureq::get(&planned_file.url)
        .set("User-Agent", USER_AGENT)
        .call()
        .unwrap_or_else(|error| panic!("GET {} failed: {error}", planned_file.url));
    let mut file_bytes: Vec<u8> = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut file_bytes)
        .unwrap_or_else(|error| panic!("reading {} failed: {error}", planned_file.url));
    if let Some(expected_size_bytes) = planned_file.expected_size_bytes {
        assert_eq!(
            file_bytes.len() as u64,
            expected_size_bytes,
            "{} has the wrong size",
            planned_file.url
        );
    }
    let temp_path: PathBuf = planned_file.path.with_extension("part");
    fs::write(&temp_path, &file_bytes).unwrap_or_else(|error| panic!("{temp_path:?}: {error}"));
    fs::rename(&temp_path, &planned_file.path)
        .unwrap_or_else(|error| panic!("{:?}: {error}", planned_file.path));
}

/// `download_sketchfab_model` fetches one Sketchfab model's glTF archive with an API token and
/// unzips it into its directory.
///
/// Example: `download_sketchfab_model(assets_dir, &SKETCHFAB_MODELS[0], &token);`
fn download_sketchfab_model(assets_dir: &Path, model: &SketchfabModel, token: &str) {
    let api_url: String = format!("{SKETCHFAB_DOWNLOAD_API_URL}/{}/download", model.uid);
    let archives: Value = ureq::get(&api_url)
        .set("User-Agent", USER_AGENT)
        .set("Authorization", &format!("Token {token}"))
        .call()
        .unwrap_or_else(|error| {
            panic!("GET {api_url} failed (check {SKETCHFAB_TOKEN_ENV}): {error}")
        })
        .into_json()
        .unwrap_or_else(|error| panic!("GET {api_url} did not return JSON: {error}"));
    let model_dir: PathBuf = assets_dir.join(model.dir);
    let zip_path: PathBuf = model_dir.with_extension("zip");
    download_file(&PlannedFile {
        url: archives["gltf"]["url"]
            .as_str()
            .unwrap_or_else(|| panic!("{api_url} returned no glTF archive: {archives}"))
            .to_string(),
        path: zip_path.clone(),
        expected_size_bytes: archives["gltf"]["size"].as_u64(),
    });
    let status = Command::new("unzip")
        .arg("-o")
        .arg("-q")
        .arg(&zip_path)
        .arg("-d")
        .arg(&model_dir)
        .status()
        .unwrap_or_else(|error| panic!("running unzip failed: {error}"));
    assert!(status.success(), "unzip {zip_path:?} failed");
}

/// `fetch_sketchfab_models` downloads the missing Sketchfab models when a token is set and
/// otherwise prints how to add them by hand.
///
/// Example: `fetch_sketchfab_models(assets_dir, false);`
fn fetch_sketchfab_models(assets_dir: &Path, is_dry_run: bool) {
    let token: Option<String> = std::env::var(SKETCHFAB_TOKEN_ENV).ok();
    for model in &SKETCHFAB_MODELS {
        if asset_manifest::sketchfab_gltf_path(assets_dir, model.dir).exists() {
            continue;
        }
        let model_dir: PathBuf = assets_dir.join(model.dir);
        match &token {
            Some(token) if !is_dry_run => {
                eprintln!("downloading {} from Sketchfab", model.title);
                download_sketchfab_model(assets_dir, model, token);
            }
            Some(_) => eprintln!("would download {} from Sketchfab", model.title),
            None => eprintln!(
                "optional: {} is missing; set {SKETCHFAB_TOKEN_ENV} or unzip its glTF download \
                 from https://sketchfab.com/3d-models/{} into {}",
                model.title,
                model.uid,
                model_dir.display()
            ),
        }
    }
}

/// `main` plans the downloads, then downloads the files that are missing.
fn main() {
    let arguments: Arguments = Arguments::parse();
    let assets_dir: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(asset_manifest::ASSETS_DIR_NAME);
    let planned_files: Vec<PlannedFile> = plan_downloads(&assets_dir);
    let missing_files: Vec<&PlannedFile> = planned_files
        .iter()
        .filter(|planned_file| !is_downloaded(planned_file))
        .collect();
    let missing_size_bytes: u64 = missing_files
        .iter()
        .filter_map(|planned_file| planned_file.expected_size_bytes)
        .sum();
    eprintln!(
        "{} of {} files to download (about {} MB) into {}",
        missing_files.len(),
        planned_files.len(),
        missing_size_bytes / 1_000_000,
        assets_dir.display()
    );
    for (index, planned_file) in missing_files.iter().enumerate() {
        eprintln!(
            "[{}/{}] {}",
            index + 1,
            missing_files.len(),
            planned_file.url
        );
        if !arguments.dry_run {
            download_file(planned_file);
        }
    }
    fetch_sketchfab_models(&assets_dir, arguments.dry_run);
    println!("{}", assets_dir.display());
}
