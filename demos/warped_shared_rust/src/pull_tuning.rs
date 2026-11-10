//! `pull_tuning.rs` lets a player change the pull's and the sprint's numbers with keys while
//! playing a level, to find the right ones in the real game (W-202, pull toward wormholes and
//! black holes in the game settings; the user: "try to get the 3D realistic experience for me to
//! test out the wormholes physics"). It is on only when the environment variable
//! `WARPED_TUNE_PULL` is 1. Every change is written to `settings_latest.json` (and appended to
//! `settings_log.jsonl`) in a run folder the level makes, and `P` writes `approved.json`, so the
//! numbers a player settles on can be read from disk and made the defaults.
//!
//! Interface: `PullTuner` (`FromEnv`, `New`, `handle_key`), `TUNE_PULL_VARIABLE`, `TUNING_KEYS_HELP`.
//!
//! Example: `if let Some(message) = tuner.handle_key(scancode, &mut gravity, &mut movement, make_folder) { show(message); }`
//!
//! Must know:
//! - The keys: `[` `]` the pull's radius, `;` `'` its strength, `\` its shape, `5` `6` the drift's
//!   braking, `7` `8` how fast the sprint slows down, `9` `0` how fast it speeds up, `Backspace`
//!   back to the built-in numbers, `P` approve.
//! - Gotchas: the changes are this window's only (movement is computed in the window); other
//!   players keep the server's settings.

use std::fmt::Display;
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

use sdl3::keyboard::Scancode;
use serde_json::{Value, json};

use crate::play_settings::{GravitySettings, MovementSettings, PullProfile};

/// `TUNE_PULL_VARIABLE` is the environment variable that turns the tuning keys on when it is 1.
pub const TUNE_PULL_VARIABLE: &str = "WARPED_TUNE_PULL";
/// `TUNING_KEYS_HELP` names the tuning keys in one line, for a level to show when tuning starts.
pub const TUNING_KEYS_HELP: &str =
    "Tune: [ ] radius  ; ' pull  \\ shape  5 6 brake  7 8 sprint down  9 0 up  P ok";
const LATEST_FILE: &str = "settings_latest.json";
const LOG_FILE: &str = "settings_log.jsonl";
const APPROVED_FILE: &str = "approved.json";

/// `Knob` is one number the keys change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Knob {
    Radius,
    Strength,
    Brake,
    SprintDown,
    SprintUp,
}

/// `KNOB_KEYS` is each knob's key down and key up, its step, and its lowest and highest value.
const KNOB_KEYS: [(Knob, Scancode, Scancode, f64, f64, f64); 5] = [
    (
        Knob::Radius,
        Scancode::LeftBracket,
        Scancode::RightBracket,
        0.25,
        1.0,
        30.0,
    ),
    (
        Knob::Strength,
        Scancode::Semicolon,
        Scancode::Apostrophe,
        0.5,
        0.0,
        60.0,
    ),
    (Knob::Brake, Scancode::_5, Scancode::_6, 1.0, 0.0, 60.0),
    (Knob::SprintDown, Scancode::_7, Scancode::_8, 1.0, 1.0, 80.0),
    (Knob::SprintUp, Scancode::_9, Scancode::_0, 1.0, 1.0, 80.0),
];

/// `PullTuner` is the tuning keys' state: the run folder once a change has made it, and how many
/// changes there have been.
pub struct PullTuner {
    folder: Option<PathBuf>,
    change_count: u32,
}

impl PullTuner {
    /// `FromEnv` gives a tuner if `WARPED_TUNE_PULL` is 1, else `None` (the keys then do nothing).
    ///
    /// Example: `let pull_tuner: Option<PullTuner> = PullTuner::FromEnv();`
    #[allow(non_snake_case)]
    pub fn FromEnv() -> Option<PullTuner> {
        (std::env::var(TUNE_PULL_VARIABLE).as_deref() == Ok("1")).then(PullTuner::New)
    }

    /// `New` gives a tuner that has made no change yet.
    ///
    /// Example: `let mut tuner: PullTuner = PullTuner::New();`
    #[allow(non_snake_case)]
    pub fn New() -> PullTuner {
        PullTuner {
            folder: None,
            change_count: 0,
        }
    }

    /// `handle_key` reacts to a pressed key: a tuning key changes `gravity` or `movement` within
    /// its range (or puts the built-in numbers back, or approves), saves the numbers into the
    /// run folder (`make_folder` makes it at the first change), and gives a line saying the
    /// numbers now and whether they were saved; any other key gives `None` and changes nothing.
    ///
    /// Example: `let message: Option<String> = tuner.handle_key(Scancode::RightBracket, &mut gravity, &mut movement, || make_run_folder("tune_pull", None));`
    pub fn handle_key<E: Display>(
        &mut self,
        scancode: Scancode,
        gravity: &mut GravitySettings,
        movement: &mut MovementSettings,
        make_folder: impl FnOnce() -> Result<PathBuf, E>,
    ) -> Option<String> {
        let mut is_approved: bool = false;
        if let Some((knob, sign)) = find_knob(scancode) {
            let (_, _, _, step, low, high) = KNOB_KEYS[knob as usize];
            let slot: &mut f64 = match knob {
                Knob::Radius => &mut gravity.radius_m,
                Knob::Strength => &mut gravity.acceleration_m_per_s_per_s,
                Knob::Brake => &mut gravity.brake_m_per_s_per_s,
                Knob::SprintDown => &mut movement.sprint_down_m_per_s_per_s,
                Knob::SprintUp => &mut movement.sprint_up_m_per_s_per_s,
            };
            *slot = (*slot + sign * step).clamp(low, high);
        } else {
            match scancode {
                Scancode::Backslash => {
                    gravity.profile = match gravity.profile {
                        PullProfile::Constant => PullProfile::StrongerNearMouth,
                        PullProfile::StrongerNearMouth => PullProfile::Harmonic,
                        PullProfile::Harmonic => PullProfile::Constant,
                    };
                }
                Scancode::Backspace => {
                    let builtin: GravitySettings = GravitySettings::default();
                    gravity.radius_m = builtin.radius_m;
                    gravity.acceleration_m_per_s_per_s = builtin.acceleration_m_per_s_per_s;
                    gravity.profile = builtin.profile;
                    gravity.brake_m_per_s_per_s = builtin.brake_m_per_s_per_s;
                    movement.sprint_up_m_per_s_per_s = BUILTIN_SPRINT_RAMP_M_PER_S_PER_S;
                    movement.sprint_down_m_per_s_per_s = BUILTIN_SPRINT_RAMP_M_PER_S_PER_S;
                }
                Scancode::P => is_approved = true,
                _ => return None,
            }
        }
        self.change_count += 1;
        let numbers: String = format!(
            "pull {:.2} m, {:.1} m/s2, {}, brake {:.0}; sprint up {:.0} down {:.0}",
            gravity.radius_m,
            gravity.acceleration_m_per_s_per_s,
            get_profile_name(gravity.profile),
            gravity.brake_m_per_s_per_s,
            movement.sprint_up_m_per_s_per_s,
            movement.sprint_down_m_per_s_per_s
        );
        let saved: Result<(), String> = self.save(gravity, movement, is_approved, make_folder);
        Some(match (saved, is_approved) {
            (Ok(()), true) => format!("APPROVED: {numbers}"),
            (Ok(()), false) => numbers,
            (Err(error), _) => format!("NOT SAVED ({error}): {numbers}"),
        })
    }

    /// `save` writes the numbers to the run folder: the latest, a line in the log, and
    /// `approved.json` when `is_approved`.
    fn save<E: Display>(
        &mut self,
        gravity: &GravitySettings,
        movement: &MovementSettings,
        is_approved: bool,
        make_folder: impl FnOnce() -> Result<PathBuf, E>,
    ) -> Result<(), String> {
        if self.folder.is_none() {
            self.folder = Some(make_folder().map_err(|error| error.to_string())?);
        }
        let folder: &PathBuf = self.folder.as_ref().expect("made above");
        let numbers: Value = json!({
            "change_count": self.change_count,
            "is_approved": is_approved,
            "gravity": {
                "radius_m": gravity.radius_m,
                "acceleration_m_per_s_per_s": gravity.acceleration_m_per_s_per_s,
                "profile": get_profile_name(gravity.profile),
                "brake_m_per_s_per_s": gravity.brake_m_per_s_per_s,
            },
            "movement": {
                "sprint_up_m_per_s_per_s": movement.sprint_up_m_per_s_per_s,
                "sprint_down_m_per_s_per_s": movement.sprint_down_m_per_s_per_s,
            },
        });
        let write = |name: &str| -> Result<(), String> {
            fs::write(folder.join(name), format!("{numbers:#}\n"))
                .map_err(|error| error.to_string())
        };
        write(LATEST_FILE)?;
        if is_approved {
            write(APPROVED_FILE)?;
        }
        let mut log: fs::File = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(folder.join(LOG_FILE))
            .map_err(|error| error.to_string())?;
        writeln!(log, "{numbers}").map_err(|error| error.to_string())
    }
}

const BUILTIN_SPRINT_RAMP_M_PER_S_PER_S: f64 = 15.0;

/// `find_knob` gives the knob a key changes and which way (-1 or 1), if it is a knob's key.
fn find_knob(scancode: Scancode) -> Option<(Knob, f64)> {
    KNOB_KEYS.iter().find_map(|&(knob, down, up, ..)| {
        if scancode == down {
            Some((knob, -1.0))
        } else if scancode == up {
            Some((knob, 1.0))
        } else {
            None
        }
    })
}

/// `get_profile_name` gives a profile's name as the settings file writes it.
fn get_profile_name(profile: PullProfile) -> &'static str {
    match profile {
        PullProfile::Constant => "constant",
        PullProfile::StrongerNearMouth => "stronger_near_mouth",
        PullProfile::Harmonic => "harmonic",
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the tuning keys.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Key | each knob's two keys (one step each way, held at its lowest and highest), the shape key (all three shapes, round), Backspace (the built-in numbers), P (approved), another key (nothing changes, nothing saved) | `keys_change_the_numbers_within_their_ranges` |
    //! | Saving | the first change (the folder is made, the latest and the log written), a later one (the log grows), approval (`approved.json`), a folder that can't be made (the line says not saved, the numbers still change) | `changes_are_saved_to_the_run_folder` |
    //! | Environment | the variable unset, 1, another value | `tuning_is_on_only_when_asked` |
    //! | Help line | fits the menu's 64 characters wide HUD line twice over | `the_help_names_every_key` |

    use super::*;

    /// `make_movement` gives movement settings with the built-in sprint ramps.
    fn make_movement() -> MovementSettings {
        MovementSettings {
            walk_speed_per_map_speed: 1.0,
            sprint_speed_per_map_speed: 1.0,
            sprint_up_m_per_s_per_s: BUILTIN_SPRINT_RAMP_M_PER_S_PER_S,
            sprint_down_m_per_s_per_s: BUILTIN_SPRINT_RAMP_M_PER_S_PER_S,
            jump_height_m: 1.25,
            jump_time_per_map_jump_time: 1.0,
            crouch_speed_frac: 0.5,
            crouch_eye_height_frac: 0.6,
            crouch_s: 0.15,
            shoot_after_moving_s: 0.25,
        }
    }

    /// `make_test_folder` gives an empty folder for a test under the crate's `.build/`.
    fn make_test_folder(name: &str) -> PathBuf {
        let folder: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".build")
            .join(format!("test_pull_tuning_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        folder
    }

    #[test]
    fn keys_change_the_numbers_within_their_ranges() {
        let folder: PathBuf = make_test_folder("keys");
        let (mut gravity, mut movement) = (GravitySettings::default(), make_movement());
        let mut tuner: PullTuner = PullTuner::New();
        let mut press =
            |scancode: Scancode, gravity: &mut GravitySettings, movement: &mut MovementSettings| {
                let made: PathBuf = folder.clone();
                tuner.handle_key(scancode, gravity, movement, || {
                    fs::create_dir_all(&made).map(|()| made.clone())
                })
            };
        assert_eq!(press(Scancode::W, &mut gravity, &mut movement), None);
        assert!(!folder.exists());
        let line: String = press(Scancode::RightBracket, &mut gravity, &mut movement).unwrap();
        assert_eq!(gravity.radius_m, 5.25);
        assert!(
            line.starts_with("pull 5.25 m, 11.9 m/s2, constant"),
            "{line}"
        );
        press(Scancode::LeftBracket, &mut gravity, &mut movement);
        assert_eq!(gravity.radius_m, 5.0);
        for _ in 0..200 {
            press(Scancode::Semicolon, &mut gravity, &mut movement);
            press(Scancode::_0, &mut gravity, &mut movement);
            press(Scancode::_7, &mut gravity, &mut movement);
            press(Scancode::_6, &mut gravity, &mut movement);
        }
        assert_eq!(gravity.acceleration_m_per_s_per_s, 0.0);
        assert_eq!(movement.sprint_up_m_per_s_per_s, 80.0);
        assert_eq!(movement.sprint_down_m_per_s_per_s, 1.0);
        assert_eq!(gravity.brake_m_per_s_per_s, 60.0);
        press(Scancode::Apostrophe, &mut gravity, &mut movement);
        assert_eq!(gravity.acceleration_m_per_s_per_s, 0.5);
        for expected in [
            PullProfile::StrongerNearMouth,
            PullProfile::Harmonic,
            PullProfile::Constant,
        ] {
            press(Scancode::Backslash, &mut gravity, &mut movement);
            assert_eq!(gravity.profile, expected);
        }
        press(Scancode::Backspace, &mut gravity, &mut movement);
        assert_eq!(gravity, GravitySettings::default());
        assert_eq!(movement, make_movement());
        let approved: String = press(Scancode::P, &mut gravity, &mut movement).unwrap();
        assert!(approved.starts_with("APPROVED: pull 5.00 m"), "{approved}");
        fs::remove_dir_all(&folder).expect("the test's folder");
    }

    #[test]
    fn changes_are_saved_to_the_run_folder() {
        let folder: PathBuf = make_test_folder("saving");
        let (mut gravity, mut movement) = (GravitySettings::default(), make_movement());
        let mut tuner: PullTuner = PullTuner::New();
        let make = |folder: &PathBuf| {
            let made: PathBuf = folder.clone();
            move || fs::create_dir_all(&made).map(|()| made.clone())
        };
        tuner.handle_key(
            Scancode::RightBracket,
            &mut gravity,
            &mut movement,
            make(&folder),
        );
        let read = |name: &str| -> Value {
            serde_json::from_str(&fs::read_to_string(folder.join(name)).expect(name)).expect(name)
        };
        assert_eq!(read(LATEST_FILE)["gravity"]["radius_m"], 5.25);
        assert_eq!(read(LATEST_FILE)["change_count"], 1);
        assert!(!folder.join(APPROVED_FILE).exists());
        tuner.handle_key(Scancode::_9, &mut gravity, &mut movement, make(&folder));
        tuner.handle_key(Scancode::P, &mut gravity, &mut movement, make(&folder));
        assert_eq!(
            read(APPROVED_FILE)["movement"]["sprint_up_m_per_s_per_s"],
            14.0
        );
        assert_eq!(read(APPROVED_FILE)["is_approved"], true);
        let log: String = fs::read_to_string(folder.join(LOG_FILE)).expect("the log");
        assert_eq!(log.lines().count(), 3);
        fs::remove_dir_all(&folder).expect("the test's folder");
        let mut unlucky: PullTuner = PullTuner::New();
        let line: String = unlucky
            .handle_key(Scancode::RightBracket, &mut gravity, &mut movement, || {
                Err::<PathBuf, String>("no folder".to_string())
            })
            .unwrap();
        assert!(
            line.starts_with("NOT SAVED (no folder): pull 5.50 m"),
            "{line}"
        );
    }

    #[test]
    fn tuning_is_on_only_when_asked() {
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN TESTS READING THE VARIABLE IN PARALLEL WILL RACE
        unsafe { std::env::remove_var(TUNE_PULL_VARIABLE) };
        assert!(PullTuner::FromEnv().is_none());
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN TESTS READING THE VARIABLE IN PARALLEL WILL RACE
        unsafe { std::env::set_var(TUNE_PULL_VARIABLE, "yes") };
        assert!(PullTuner::FromEnv().is_none());
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN TESTS READING THE VARIABLE IN PARALLEL WILL RACE
        unsafe { std::env::set_var(TUNE_PULL_VARIABLE, "1") };
        assert!(PullTuner::FromEnv().is_some());
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN TESTS READING THE VARIABLE IN PARALLEL WILL RACE
        unsafe { std::env::remove_var(TUNE_PULL_VARIABLE) };
    }

    #[test]
    fn the_help_names_every_key() {
        for key in ["[", "]", ";", "'", "\\", "5", "6", "7", "8", "9", "0", "P"] {
            assert!(TUNING_KEYS_HELP.contains(key), "{key}");
        }
        assert!(TUNING_KEYS_HELP.chars().count() <= 128);
    }
}
