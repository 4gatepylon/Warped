//! `setting_rules.rs` says what every server setting may be, so settings are checked as a whole
//! where the game master saves them, with the reason when they are refused (W-209, game settings
//! checked when saved, with the reason): each setting has a kind (`SettingKind`: a switch, a
//! share, an amount, a zoom, a range, a choice), found from its key and its default, and settings
//! that bound each other form a group with a relation (`SettingGroup`, `Relation`: shares that add
//! up to 1, values in ascending order).
//!
//! Interface: `SettingKind`, `get_setting_kind`, `check_value`, `Relation`, `SettingGroup`,
//! `SETTING_GROUPS`, `check_group`, `check_settings`.
//!
//! Example: `check_settings(&merged, &defaults)?;`
//!
//! Must know:
//! - A key is the setting's dotted path in a level's `server_settings.yaml`
//!   (`beam_speckle.black_frac`). A group is checked on settings that have all of its keys.
//! - The levels check their own settings again when a round's server starts
//!   (`ServerSettings::FromYaml` in each level); these rules repeat those checks, and the app's
//!   test `the_shipped_settings_pass_the_rules` holds every shipped settings file to them.
//! - Limitations: a setting that is neither a switch, a number nor a known choice isn't checked.

use serde_json::Value;

/// `SUM_TOLERANCE` is how far shares that add up to 1 may be off, for float noise.
const SUM_TOLERANCE: f64 = 1e-9;
const FRACTION_SUFFIX: &str = "_frac";

/// `ZERO_ALLOWED_KEYS` are the numbers that may be 0; every other number must be above 0.
const ZERO_ALLOWED_KEYS: [&str; 35] = [
    "wormholes.sniper_hits_to_destroy",
    "wormholes.rifle_hits_to_destroy",
    "gravity.black_hole_acceleration_frac",
    "gravity.black_hole_min_radius_m",
    "gravity.black_hole_darkness_frac",
    "gravity.acceleration_m_per_s_per_s",
    "gravity.brake_m_per_s_per_s",
    "movement.shoot_after_moving_s",
    "map_edge.start_wall_frac",
    "sniper.reload_s",
    "sniper.noise_mean_rad",
    "sniper.noise_max_rad",
    "sniper.beam_speckle_mean_frac",
    "sniper.beam_speckle_max_frac",
    "rifle.noise_start_rad",
    "rifle.noise_max_rad",
    "rifle.beam_speckle_start_frac",
    "rifle.beam_speckle_max_frac",
    "beam_speckle.black_frac",
    "beam_speckle.white_frac",
    "beam_speckle.dropout_frac",
    "rifle.recoil_up_rad",
    "rifle.recoil_sideways_rad",
    "rifle.recoil_settle_s",
    "respawn.start_s",
    "respawn.step_s",
    "shooting.drop_m_per_s_per_s",
    "wormholes.throw_up_rad",
    "black_hole.time_dilation_per_general_relativity",
    "black_hole.max_lens_rad",
    "blizzard.radius_per_map_radius",
    "sandstorm.wind_per_map_wind",
    "glass.max_bounce_count",
    "glass.reflectivity_frac",
    "glass_lights.low_brightness_frac",
];

/// `AT_LEAST_ONE_KEYS` are the zooms: 1 for none.
const AT_LEAST_ONE_KEYS: [&str; 2] = ["aim.sniper_zoom", "aim.rifle_zoom"];

/// `BETWEEN_KEYS` are the numbers with a range of their own: the key, its least and its most.
const BETWEEN_KEYS: [(&str, f64, f64); 2] = [
    ("space.curvature_per_map_curvature", 1.0, 2.0),
    ("team_domes.circumference_frac", 0.0, 0.5),
];

/// `AMOUNT_FRAC_KEYS` are the keys ending in `_frac` that are factors, not shares: they may pass 1.
const AMOUNT_FRAC_KEYS: [&str; 1] = ["flashlight.brightness_frac"];

/// `CHOICE_KEYS` are the settings that are one of a few words: the key and its words.
const CHOICE_KEYS: [(&str, &[&str]); 1] = [(
    "gravity.profile",
    &["constant", "stronger_near_mouth", "harmonic"],
)];

/// `SettingKind` is what a setting may be: a switch (on or off), a share (0 to 1), an amount (a
/// size, distance, time, speed, force, angle, count or ratio: never negative), a zoom (1 or
/// more), a number between its own least and most, or one of a few words.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SettingKind {
    Switch,
    Share,
    Amount,
    AtLeastOne,
    Between { min: f64, max: f64 },
    Choice(&'static [&'static str]),
}

/// `get_setting_kind` gives the kind of the setting `key` whose default is `default`, or `None`
/// for a setting these rules don't cover (a word that isn't a known choice, a list, a section).
///
/// Example: `assert_eq!(get_setting_kind("beam_speckle.black_frac", &json!(0.1)), Some(SettingKind::Share));`
pub fn get_setting_kind(key: &str, default: &Value) -> Option<SettingKind> {
    if let Some((_, words)) = CHOICE_KEYS
        .iter()
        .find(|(choice_key, _)| *choice_key == key)
    {
        return Some(SettingKind::Choice(words));
    }
    match default {
        Value::Bool(_) => Some(SettingKind::Switch),
        Value::Number(_) => {
            if let Some(&(_, min, max)) =
                BETWEEN_KEYS.iter().find(|(between, _, _)| *between == key)
            {
                return Some(SettingKind::Between { min, max });
            }
            if AT_LEAST_ONE_KEYS.contains(&key) {
                return Some(SettingKind::AtLeastOne);
            }
            let is_share: bool = key.ends_with(FRACTION_SUFFIX) && !AMOUNT_FRAC_KEYS.contains(&key);
            Some(if is_share {
                SettingKind::Share
            } else {
                SettingKind::Amount
            })
        }
        _ => None,
    }
}

/// `check_value` fails, naming `key` and what it may be, when `value` isn't a `kind`: a number
/// must be finite, above 0 (or 0 where `ZERO_ALLOWED_KEYS` has the key), whole where `default`
/// is, and inside its kind's range.
///
/// Example: `check_value("beam_speckle.black_frac", SettingKind::Share, &json!(0.2), &json!(0.1))?;`
pub fn check_value(
    key: &str,
    kind: SettingKind,
    value: &Value,
    default: &Value,
) -> Result<(), String> {
    match kind {
        SettingKind::Switch => match value {
            Value::Bool(_) => Ok(()),
            _ => Err(format!("{key} is a switch: on or off, not {value}")),
        },
        SettingKind::Choice(words) => match value.as_str() {
            Some(word) if words.contains(&word) => Ok(()),
            _ => Err(format!("{key} is one of {}, not {value}", words.join(", "))),
        },
        _ => {
            let x: f64 = value
                .as_f64()
                .filter(|x| x.is_finite())
                .ok_or_else(|| format!("{key} is a number, not {value}"))?;
            let is_zero_allowed: bool = ZERO_ALLOWED_KEYS.contains(&key);
            if x < 0.0 || (x == 0.0 && !is_zero_allowed) {
                let least: &str = if is_zero_allowed {
                    "0 or more"
                } else {
                    "above 0"
                };
                return Err(format!("{key} must be {least}, not {x}"));
            }
            if default.is_u64() && x.fract() != 0.0 {
                return Err(format!("{key} must be a whole number, not {x}"));
            }
            match kind {
                SettingKind::Share if x > 1.0 => {
                    Err(format!("{key} is a share: from 0 to 1, not {x}"))
                }
                SettingKind::AtLeastOne if x < 1.0 => {
                    Err(format!("{key} must be 1 or more (1 for none), not {x}"))
                }
                SettingKind::Between { min, max } if x < min || x > max => {
                    Err(format!("{key} must be from {min} to {max}, not {x}"))
                }
                _ => Ok(()),
            }
        }
    }
}

/// `Relation` is how the settings of a group bound each other: shares of one whole add up to 1;
/// values in ascending order are each at most the next (`Ascending`) or below it
/// (`StrictlyAscending`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Relation {
    SumsToOne,
    Ascending,
    StrictlyAscending,
}

/// `SettingGroup` is settings that bound each other: their keys, in the relation's order, and
/// the relation.
pub struct SettingGroup {
    pub keys: &'static [&'static str],
    pub relation: Relation,
}

/// `SETTING_GROUPS` are the groups of the game's settings.
pub const SETTING_GROUPS: [SettingGroup; 5] = [
    SettingGroup {
        keys: &[
            "beam_speckle.black_frac",
            "beam_speckle.white_frac",
            "beam_speckle.dropout_frac",
        ],
        relation: Relation::SumsToOne,
    },
    SettingGroup {
        keys: &["rifle.noise_start_rad", "rifle.noise_max_rad"],
        relation: Relation::Ascending,
    },
    SettingGroup {
        keys: &[
            "rifle.beam_speckle_start_frac",
            "rifle.beam_speckle_max_frac",
        ],
        relation: Relation::Ascending,
    },
    SettingGroup {
        keys: &["map_edge.start_wall_frac", "map_edge.stop_wall_frac"],
        relation: Relation::StrictlyAscending,
    },
    SettingGroup {
        keys: &[
            "glass_lights.low_brightness_frac",
            "glass_lights.high_brightness_frac",
        ],
        relation: Relation::Ascending,
    },
];

/// `find_at` gives the value at a dotted `key` of `settings`, or `None`.
///
/// Example: `let black = find_at(&settings, "beam_speckle.black_frac");`
fn find_at<'a>(settings: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.')
        .try_fold(settings, |node, part| node.get(part))
}

/// `check_group` fails, naming the group's keys, what they must do and their values, when
/// `settings` has every key of `group` as a number and they break its relation.
///
/// Example: `check_group(&SETTING_GROUPS[0], &settings)?;`
pub fn check_group(group: &SettingGroup, settings: &Value) -> Result<(), String> {
    let found: Vec<f64> = group
        .keys
        .iter()
        .filter_map(|key| find_at(settings, key).and_then(Value::as_f64))
        .collect();
    if found.len() != group.keys.len() {
        return Ok(());
    }
    let is_kept: bool = match group.relation {
        Relation::SumsToOne => (found.iter().sum::<f64>() - 1.0).abs() <= SUM_TOLERANCE,
        Relation::Ascending => found.windows(2).all(|pair| pair[0] <= pair[1]),
        Relation::StrictlyAscending => found.windows(2).all(|pair| pair[0] < pair[1]),
    };
    if is_kept {
        return Ok(());
    }
    let (section, names): (String, Vec<&str>) = split_section(group.keys);
    let values: String = found
        .iter()
        .map(f64::to_string)
        .collect::<Vec<String>>()
        .join(" and ");
    Err(match group.relation {
        Relation::SumsToOne => format!(
            "{section}{} are shares of one whole and must add up to 1, not {}",
            names.join(" + "),
            (found.iter().sum::<f64>() / SUM_TOLERANCE).round() * SUM_TOLERANCE
        ),
        Relation::Ascending => format!(
            "{section}{} must be in ascending order, not {values}",
            names.join(", ")
        ),
        Relation::StrictlyAscending => format!(
            "{section}{} must each be below the next, not {values}",
            names.join(", ")
        ),
    })
}

/// `split_section` gives the section every key of `keys` is in, as `<section>: ` (empty when
/// they aren't in one section), and the keys without it, so a message names the section once.
///
/// Example: `let (section, names) = split_section(&["rifle.noise_start_rad", "rifle.noise_max_rad"]);`
fn split_section<'a>(keys: &[&'a str]) -> (String, Vec<&'a str>) {
    let split: Option<Vec<(&str, &str)>> = keys.iter().map(|key| key.rsplit_once('.')).collect();
    match split {
        Some(parts) if parts.iter().all(|(section, _)| *section == parts[0].0) => (
            format!("{}: ", parts[0].0),
            parts.iter().map(|(_, name)| *name).collect(),
        ),
        _ => (String::new(), keys.to_vec()),
    }
}

/// `check_leaves` checks every setting under `defaults` at `key` (`""` for the top) against its
/// kind, failing on the first that breaks it or is missing from `settings`.
///
/// Example: `check_leaves(&settings, &defaults, "")?;`
fn check_leaves(settings: &Value, defaults: &Value, key: &str) -> Result<(), String> {
    if let Value::Object(fields) = defaults {
        return fields.iter().try_for_each(|(name, default)| {
            let path: String = match key {
                "" => name.clone(),
                _ => format!("{key}.{name}"),
            };
            let value: &Value = settings
                .get(name)
                .ok_or_else(|| format!("{path} is missing"))?;
            check_leaves(value, default, &path)
        });
    }
    match get_setting_kind(key, defaults) {
        Some(kind) => check_value(key, kind, settings, defaults),
        None => Ok(()),
    }
}

/// `check_settings` fails with the reason when `settings`, one level's server settings, break a
/// rule: each setting against its kind (found with its default in `defaults`, the level's own
/// settings file), then each group of `SETTING_GROUPS`.
///
/// Example: `check_settings(&merged, &defaults)?;`
pub fn check_settings(settings: &Value, defaults: &Value) -> Result<(), String> {
    check_leaves(settings, defaults, "")?;
    SETTING_GROUPS
        .iter()
        .try_for_each(|group| check_group(group, settings))
}

#[cfg(test)]
mod tests {
    //! Unit tests for `setting_rules.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Kind of a setting | switch, share, a `_frac` that is a factor, amount, zoom, own range, choice, a word that is no choice | `kinds_come_from_the_key_and_the_default` |
    //! | A number's value | inside; negative; 0 where allowed and where not; not whole where the default is; above a share's 1; below a zoom's 1; outside its range; not a number | `numbers_are_held_to_their_kind` |
    //! | A switch's and a choice's value | a switch, a number for a switch; a known word, another word | `switches_and_choices_are_held_to_their_kind` |
    //! | Group relation | shares adding up to 1 (within float noise) and not; ascending with equal values and descending; strictly ascending with equal values; a group whose keys the settings lack; keys in one section (named once) and in two | `groups_are_held_to_their_relation` |
    //! | Whole settings | fine; a bad value named; a broken group named; a setting missing | `settings_are_checked_as_a_whole` |

    use serde_json::json;

    use super::*;

    #[test]
    fn kinds_come_from_the_key_and_the_default() {
        for (key, default, kind) in [
            (
                "shooting.is_friendly_fire_on",
                json!(true),
                Some(SettingKind::Switch),
            ),
            (
                "beam_speckle.black_frac",
                json!(0.1),
                Some(SettingKind::Share),
            ),
            (
                "flashlight.brightness_frac",
                json!(2.0),
                Some(SettingKind::Amount),
            ),
            ("respawn.start_s", json!(3.0), Some(SettingKind::Amount)),
            ("aim.sniper_zoom", json!(2.5), Some(SettingKind::AtLeastOne)),
            (
                "space.curvature_per_map_curvature",
                json!(1.5),
                Some(SettingKind::Between { min: 1.0, max: 2.0 }),
            ),
            ("some.name", json!("word"), None),
        ] {
            assert_eq!(get_setting_kind(key, &default), kind, "{key}");
        }
        assert!(matches!(
            get_setting_kind("gravity.profile", &json!("constant")),
            Some(SettingKind::Choice(_))
        ));
    }

    #[test]
    fn numbers_are_held_to_their_kind() {
        let check = |key: &str, value: Value, default: Value| -> Result<(), String> {
            let kind: SettingKind = get_setting_kind(key, &default).unwrap();
            check_value(key, kind, &value, &default)
        };
        assert!(check("beam_speckle.black_frac", json!(0.4), json!(0.1)).is_ok());
        assert!(check("beam_speckle.black_frac", json!(0.0), json!(0.1)).is_ok());
        assert!(check("beam_speckle.black_frac", json!(-0.1), json!(0.1)).is_err());
        assert_eq!(
            check("beam_speckle.black_frac", json!(1.5), json!(0.1)).unwrap_err(),
            "beam_speckle.black_frac is a share: from 0 to 1, not 1.5"
        );
        assert_eq!(
            check("movement.walk_speed_per_map_speed", json!(0.0), json!(1.0)).unwrap_err(),
            "movement.walk_speed_per_map_speed must be above 0, not 0"
        );
        assert!(check("glass.max_bounce_count", json!(0), json!(2)).is_ok());
        assert_eq!(
            check("glass.max_bounce_count", json!(1.5), json!(2)).unwrap_err(),
            "glass.max_bounce_count must be a whole number, not 1.5"
        );
        assert!(check("aim.rifle_zoom", json!(0.5), json!(1.5)).is_err());
        assert!(check("space.curvature_per_map_curvature", json!(2.5), json!(1.5)).is_err());
        assert!(check("team_domes.circumference_frac", json!(0.0), json!(0.2)).is_err());
        assert!(check("respawn.start_s", json!("soon"), json!(3.0)).is_err());
    }

    #[test]
    fn switches_and_choices_are_held_to_their_kind() {
        let switch: SettingKind = SettingKind::Switch;
        assert!(check_value("a.is_on", switch, &json!(false), &json!(true)).is_ok());
        assert!(check_value("a.is_on", switch, &json!(1), &json!(true)).is_err());
        let profile: SettingKind = get_setting_kind("gravity.profile", &json!("constant")).unwrap();
        assert!(
            check_value(
                "gravity.profile",
                profile,
                &json!("harmonic"),
                &json!("constant")
            )
            .is_ok()
        );
        assert_eq!(
            check_value(
                "gravity.profile",
                profile,
                &json!("fast"),
                &json!("constant")
            )
            .unwrap_err(),
            "gravity.profile is one of constant, stronger_near_mouth, harmonic, not \"fast\""
        );
    }

    #[test]
    fn groups_are_held_to_their_relation() {
        let speckle = |black: f64| json!({"beam_speckle": {"black_frac": black, "white_frac": 0.1, "dropout_frac": 0.8}});
        assert!(check_group(&SETTING_GROUPS[0], &speckle(0.1)).is_ok());
        assert!(check_group(&SETTING_GROUPS[0], &speckle(0.1 + 1e-12)).is_ok());
        let error: String = check_group(&SETTING_GROUPS[0], &speckle(0.2)).unwrap_err();
        assert_eq!(
            error,
            "beam_speckle: black_frac + white_frac + dropout_frac are shares of one whole and must add up to 1, not 1.1"
        );
        assert_eq!(
            split_section(&["rifle.noise_start_rad", "sniper.noise_max_rad"]),
            (
                String::new(),
                vec!["rifle.noise_start_rad", "sniper.noise_max_rad"]
            )
        );
        let noise =
            |start: f64| json!({"rifle": {"noise_start_rad": start, "noise_max_rad": 0.02}});
        assert!(check_group(&SETTING_GROUPS[1], &noise(0.02)).is_ok());
        assert!(check_group(&SETTING_GROUPS[1], &noise(0.03)).is_err());
        let edge =
            |start: f64| json!({"map_edge": {"start_wall_frac": start, "stop_wall_frac": 0.9}});
        assert!(check_group(&SETTING_GROUPS[3], &edge(0.5)).is_ok());
        assert!(check_group(&SETTING_GROUPS[3], &edge(0.9)).is_err());
        assert!(check_group(&SETTING_GROUPS[0], &json!({"rifle": {}})).is_ok());
    }

    #[test]
    fn settings_are_checked_as_a_whole() {
        let defaults: Value = json!({
            "beam_speckle": {"black_frac": 0.1, "white_frac": 0.1, "dropout_frac": 0.8},
            "respawn": {"start_s": 3.0},
        });
        assert!(check_settings(&defaults, &defaults).is_ok());
        let mut bad_value: Value = defaults.clone();
        bad_value["respawn"]["start_s"] = json!(-1.0);
        assert_eq!(
            check_settings(&bad_value, &defaults).unwrap_err(),
            "respawn.start_s must be 0 or more, not -1"
        );
        let mut bad_group: Value = defaults.clone();
        bad_group["beam_speckle"]["black_frac"] = json!(0.3);
        assert!(
            check_settings(&bad_group, &defaults)
                .unwrap_err()
                .contains("must add up to 1")
        );
        let missing: Value = json!({"respawn": {"start_s": 3.0}});
        assert_eq!(
            check_settings(&missing, &defaults).unwrap_err(),
            "beam_speckle is missing"
        );
    }
}
