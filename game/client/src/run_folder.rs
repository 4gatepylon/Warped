//! `run_folder.rs` makes a run's folder under `.artifacts/<script>/<run_id>/` (with its `command.txt`),
//! names runs and logs by the UTC time, and saves frames there as PNGs.
//!
//! Interface: `make_run_folder`, `format_utc_now`, `save_png`.
//!
//! Example: `let folder: PathBuf = make_run_folder("bench_views", None)?;`

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Result;

const SECONDS_PER_DAY: u64 = 86_400;

/// `make_run_folder` creates `.artifacts/<script>/<run_id>/` in the crate folder, with
/// `command.txt` holding the command line and the git commit, and returns its path.
///
/// Example: `let folder = make_run_folder("bench_views", None)?;`
pub(crate) fn make_run_folder(script: &str, label: Option<&str>) -> Result<PathBuf> {
    let crate_folder: &Path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut run_id: String = format_utc_now();
    if let Some(label) = label {
        run_id = format!("{run_id}-{label}");
    }
    let folder: PathBuf = crate_folder.join(".artifacts").join(script).join(&run_id);
    if folder.exists() {
        return Err(format!(
            "run folder {} already exists; wait a second or pass another --label",
            folder.display()
        )
        .into());
    }
    fs::create_dir_all(&folder)?;
    let commit: String = std::process::Command::new("git")
        .args(["describe", "--always", "--dirty"])
        .current_dir(crate_folder)
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())?;
    let command: String = std::env::args().collect::<Vec<String>>().join(" ");
    fs::write(
        folder.join("command.txt"),
        format!("{command}\ncommit: {commit}\n"),
    )?;
    Ok(folder)
}

/// `format_utc_now` formats the current UTC time as `YYYYMMDD-HHMMSS` (civil-from-days, Howard
/// Hinnant's algorithm).
///
/// Example: `let run_id = format_utc_now();`
pub(crate) fn format_utc_now() -> String {
    let seconds: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs();
    let (days, time_of_day) = (
        (seconds / SECONDS_PER_DAY) as i64,
        seconds % SECONDS_PER_DAY,
    );
    let z: i64 = days + 719_468;
    let era: i64 = z.div_euclid(146_097);
    let day_of_era: i64 = z - era * 146_097;
    let year_of_era: i64 =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year: i64 = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index: i64 = (5 * day_of_year + 2) / 153;
    let day: i64 = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month: i64 = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year: i64 = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        time_of_day / 3600,
        time_of_day / 60 % 60,
        time_of_day % 60
    )
}

/// `save_png` writes an RGBA image as a PNG file.
///
/// Example: `save_png(&folder.join("spawn.png"), &pixels, 1280, 800)?;`
pub(crate) fn save_png(path: &Path, pixels: &[u8], width_px: u32, height_px: u32) -> Result<()> {
    let file: fs::File = fs::File::create(path)?;
    let mut encoder: png::Encoder<std::io::BufWriter<fs::File>> =
        png::Encoder::new(std::io::BufWriter::new(file), width_px, height_px);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(pixels)?;
    Ok(())
}
