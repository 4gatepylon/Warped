//! `assets` loads the demo's downloaded assets: textures, glTF models, and the HDR sky.
//!
//! Example:
//!     let model: Model = gltf_model::load_model(&mut renderer, &path, &options);

pub mod gltf_model;
pub mod sky;
pub mod textures;
