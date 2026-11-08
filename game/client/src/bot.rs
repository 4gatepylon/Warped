//! `bot.rs` runs a bot player without a window (`--bot`): it joins a game server, plays for
//! `--exit-after-s` seconds and reports its hits in its run folder.
//!
//! Interface: `run_bot`.
//!
//! Example: `run_bot(&arguments, renderer)?;`

use std::path::PathBuf;

use serde_json::json;

use crate::render_metal::FrameData;
use crate::run_folder::{make_run_folder, save_png};
use crate::settings::ServerSettings;
use crate::voxel_world::VoxelWorld;

use crate::cli::Arguments;
use crate::renderer::Renderer;
use crate::server_process::get_server_address;
use crate::server_process::load_map;
use crate::{BYTES_PER_PIXEL, Result, map, multiplayer, net};

/// `run_bot` plays as a bot without a window for `--exit-after-s` seconds (see
/// `multiplayer::run_bot`), saving its view at each hit under `.artifacts/bot/<run_id>/`, and
/// prints how many players it hit and how often it was hit.
///
/// Example: `run_bot(&arguments, renderer)?;`
pub(crate) fn run_bot(arguments: &Arguments, mut renderer: Box<dyn Renderer>) -> Result<()> {
    let map: map::Map = load_map(arguments.map, &ServerSettings::Builtin())?;
    let mut client: net::Client = net::Client::Connect(
        get_server_address(arguments)?,
        arguments.map.get_folder_name(),
        &net::read_game_password(arguments.game_password_file.as_deref())?,
    )?;
    client.join(None)?;
    let folder: PathBuf = make_run_folder("bot", arguments.label.as_deref())?;
    let mut saved_count: usize = 0;
    let duration_s: f64 = arguments
        .exit_after_s
        .expect("validate_arguments checked --bot");
    let mut save = |frame: FrameData, victim_id: u32| -> Result<()> {
        let size: (u32, u32) = {
            let (w, h) = frame.get_render_size();
            (w as u32, h as u32)
        };
        let mut pixels: Vec<u8> = vec![0; size.0 as usize * size.1 as usize * BYTES_PER_PIXEL];
        renderer.render(frame, size, &mut pixels)?;
        saved_count += 1;
        save_png(
            &folder.join(format!("hit_{saved_count:03}_player_{victim_id}.png")),
            &pixels,
            size.0,
            size.1,
        )
    };
    let (hit_count, eliminated_count) = multiplayer::run_voxel_bot(
        &VoxelWorld::FromMap(&map)?,
        &mut client,
        duration_s,
        arguments.black_hole_strength,
        |frame, id| save(FrameData::Voxel(frame), id),
    )?;
    if arguments.json {
        println!(
            "{}",
            json!({"hit_count": hit_count, "eliminated_count": eliminated_count, "run_folder": folder})
        );
    } else {
        println!("The bot hit {hit_count} players and was hit {eliminated_count} times.");
        println!("Run folder: {}", folder.display());
    }
    Ok(())
}
