//! `key_bindings.rs` reads which keys, mouse buttons and wheel do what in the game window (the `keys`
//! of `player_settings.yaml`), so each player can choose their own controls.
//!
//! Interface: `Action`, `Control`, `KeyBindings` (`FromNames`, `get_all`, `set_controls`,
//! `get_names`, `to_names`, `is_held`, `matches`, `get_actions`, `get_label`, `describe`),
//! `get_pressed_control`.
//!
//! Example: `let keys = KeyBindings::FromNames(&names, "mine.yaml")?;`
//!
//! Must know:
//! - Every action is required and unknown actions or controls fail, naming them, so a typo can't
//!   silently leave an action without a key.

use std::collections::BTreeMap;
use std::error::Error;

use sdl3::event::Event;
use sdl3::keyboard::{KeyboardState, Scancode};
use sdl3::mouse::{MouseButton, MouseState};

const WHEEL_NAME: &str = "Wheel";
const MOUSE_BUTTON_NAMES: [(&str, MouseButton); 5] = [
    ("Mouse Left", MouseButton::Left),
    ("Mouse Right", MouseButton::Right),
    ("Mouse Middle", MouseButton::Middle),
    ("Mouse X1", MouseButton::X1),
    ("Mouse X2", MouseButton::X2),
];
const ACTION_NAMES: [(Action, &str); 21] = [
    (Action::MoveForward, "move_forward"),
    (Action::MoveBack, "move_back"),
    (Action::MoveLeft, "move_left"),
    (Action::MoveRight, "move_right"),
    (Action::Jump, "jump"),
    (Action::Sprint, "sprint"),
    (Action::Crouch, "crouch"),
    (Action::Shoot, "shoot"),
    (Action::Aim, "aim"),
    (Action::Flashlight, "flashlight"),
    (Action::TurnOffConsole, "turn_off_console"),
    (Action::Gun1, "gun_1"),
    (Action::Gun2, "gun_2"),
    (Action::SwitchGun, "switch_gun"),
    (Action::SwitchView, "switch_view"),
    (Action::JoinBlue, "join_blue"),
    (Action::JoinRed, "join_red"),
    (Action::LowerResolution, "lower_resolution"),
    (Action::RaiseResolution, "raise_resolution"),
    (Action::ReleaseMouse, "release_mouse"),
    (Action::OpenMenu, "open_menu"),
];
const PRESS_ONLY_ACTIONS: [Action; 10] = [
    Action::Gun1,
    Action::Gun2,
    Action::SwitchGun,
    Action::SwitchView,
    Action::JoinBlue,
    Action::JoinRed,
    Action::LowerResolution,
    Action::RaiseResolution,
    Action::ReleaseMouse,
    Action::OpenMenu,
];
const HELP_GROUPS: [(&[Action], &str); 13] = [
    (
        &[
            Action::MoveForward,
            Action::MoveLeft,
            Action::MoveBack,
            Action::MoveRight,
        ],
        "move",
    ),
    (&[Action::Sprint], "run"),
    (&[Action::Jump], "jump"),
    (
        &[Action::Crouch],
        "crouch (shoot standing still or crouched)",
    ),
    (&[Action::Shoot], "shoot"),
    (&[Action::Aim], "aim"),
    (&[Action::Flashlight], "flashlight"),
    (&[Action::Gun1, Action::Gun2, Action::SwitchGun], "guns"),
    (&[Action::SwitchView], "third person"),
    (
        &[Action::TurnOffConsole],
        "hold on an enemy console to turn it off",
    ),
    (&[Action::LowerResolution, Action::RaiseResolution], "scale"),
    (&[Action::ReleaseMouse], "release"),
    (&[Action::OpenMenu], "menu"),
];

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `Action` is something a player does with a key, a mouse button or the wheel.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Action {
    MoveForward,
    MoveBack,
    MoveLeft,
    MoveRight,
    Jump,
    Sprint,
    Crouch,
    Shoot,
    Aim,
    Flashlight,
    TurnOffConsole,
    Gun1,
    Gun2,
    SwitchGun,
    SwitchView,
    JoinBlue,
    JoinRed,
    LowerResolution,
    RaiseResolution,
    ReleaseMouse,
    OpenMenu,
}

/// `Control` is a key (by its place on the keyboard), a mouse button, or the mouse wheel turned
/// either way.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Control {
    Key(Scancode),
    Mouse(MouseButton),
    Wheel,
}

impl Control {
    /// `FromName` reads a control's name as `player_settings.yaml` writes it, or `None` for an
    /// unknown name.
    ///
    /// Example: `let shift = Control::FromName("Left Shift");`
    #[allow(non_snake_case)]
    fn FromName(name: &str) -> Option<Control> {
        if name == WHEEL_NAME {
            return Some(Control::Wheel);
        }
        MOUSE_BUTTON_NAMES
            .iter()
            .find(|(button_name, _)| *button_name == name)
            .map(|&(_, button)| Control::Mouse(button))
            .or_else(|| Scancode::from_name(name).map(Control::Key))
    }

    /// `get_name` gives the control's name as `player_settings.yaml` writes it.
    ///
    /// Example: `let name = Control::Wheel.get_name();`
    fn get_name(self) -> String {
        match self {
            Control::Key(scancode) => scancode.name().to_string(),
            Control::Mouse(button) => MOUSE_BUTTON_NAMES
                .iter()
                .find(|(_, b)| *b == button)
                .map_or("Mouse", |(name, _)| name)
                .to_string(),
            Control::Wheel => WHEEL_NAME.to_string(),
        }
    }
}

/// `get_pressed_control` gives the control a window event presses: a key going down (not a
/// repeat), a mouse button going down, or the wheel turning; `None` for any other event.
///
/// Example: `if let Some(control) = get_pressed_control(&event) { ... }`
pub fn get_pressed_control(event: &Event) -> Option<Control> {
    match *event {
        Event::KeyDown {
            scancode: Some(scancode),
            repeat: false,
            ..
        } => Some(Control::Key(scancode)),
        Event::MouseButtonDown { mouse_btn, .. } => Some(Control::Mouse(mouse_btn)),
        Event::MouseWheel { y, .. } if y != 0.0 => Some(Control::Wheel),
        _ => None,
    }
}

/// `KeyBindings` is the controls of every action.
pub struct KeyBindings {
    controls: BTreeMap<Action, Vec<Control>>,
}

impl KeyBindings {
    /// `FromNames` reads key bindings from each action's control names (`player_settings.yaml`'s
    /// `keys`), naming `source` in errors: every action needs one or more known controls, and the
    /// wheel only for actions done once per press.
    ///
    /// Example: `let keys = KeyBindings::FromNames(&names, "mine.yaml")?;`
    #[allow(non_snake_case)]
    pub fn FromNames(names: &BTreeMap<String, Vec<String>>, source: &str) -> Result<KeyBindings> {
        let action_list: String = ACTION_NAMES.map(|(_, name)| name).join(", ");
        if let Some(unknown) = names
            .keys()
            .find(|key| !ACTION_NAMES.iter().any(|(_, name)| name == key))
        {
            return Err(format!(
                "key bindings {source}: {unknown} isn't an action; the actions are {action_list}"
            )
            .into());
        }
        let mut controls: BTreeMap<Action, Vec<Control>> = BTreeMap::new();
        for (action, name) in ACTION_NAMES {
            let Some(control_names) = names.get(name).filter(|list| !list.is_empty()) else {
                return Err(
                    format!("key bindings {source}: {name} needs at least one control").into(),
                );
            };
            let mut action_controls: Vec<Control> = Vec::new();
            for control_name in control_names {
                let control: Control = Control::FromName(control_name).ok_or_else(|| {
                    format!("key bindings {source}: {control_name:?} (for {name}) isn't a key, mouse button or Wheel; keys use SDL's names, such as W, Space, Left Shift, Escape or \"1\"")
                })?;
                if control == Control::Wheel && !PRESS_ONLY_ACTIONS.contains(&action) {
                    return Err(format!(
                        "key bindings {source}: {name} is held, so it can't use the Wheel"
                    )
                    .into());
                }
                action_controls.push(control);
            }
            controls.insert(action, action_controls);
        }
        Ok(KeyBindings { controls })
    }

    /// `get_all` gives every action with its name in `player_settings.yaml`, in the file's order.
    ///
    /// Example: `for (action, name) in KeyBindings::get_all() { ... }`
    pub fn get_all() -> &'static [(Action, &'static str)] {
        &ACTION_NAMES
    }

    /// `set_controls` makes `controls` the only ones of `action`, failing (with the reason) on the
    /// wheel for an action that is held.
    ///
    /// Example: `keys.set_controls(Action::Jump, vec![Control::Key(Scancode::Return)])?;`
    pub fn set_controls(&mut self, action: Action, controls: Vec<Control>) -> Result<()> {
        if controls.contains(&Control::Wheel) && !PRESS_ONLY_ACTIONS.contains(&action) {
            return Err("that action is held, so it can't use the Wheel".into());
        }
        self.controls.insert(action, controls);
        Ok(())
    }

    /// `get_names` gives `action`'s controls by name, joined with commas.
    ///
    /// Example: `let shown = keys.get_names(Action::Sprint);`
    pub fn get_names(&self, action: Action) -> String {
        let names: Vec<String> = self.controls[&action]
            .iter()
            .map(|c| c.get_name())
            .collect();
        names.join(", ")
    }

    /// `to_names` gives every action's controls by name, as `player_settings.yaml`'s `keys` holds
    /// them.
    ///
    /// Example: `let names = keys.to_names();`
    pub fn to_names(&self) -> BTreeMap<String, Vec<String>> {
        ACTION_NAMES
            .iter()
            .map(|&(action, name)| {
                let names: Vec<String> = self.controls[&action]
                    .iter()
                    .map(|c| c.get_name())
                    .collect();
                (name.to_string(), names)
            })
            .collect()
    }

    /// `is_held` says whether any control of `action` is held now, by the keyboard's and the
    /// mouse's state.
    ///
    /// Example: `let forward = keys.is_held(Action::MoveForward, &keyboard, &mouse);`
    pub fn is_held(&self, action: Action, keyboard: &KeyboardState, mouse: &MouseState) -> bool {
        self.controls[&action].iter().any(|&control| match control {
            Control::Key(scancode) => keyboard.is_scancode_pressed(scancode),
            Control::Mouse(button) => mouse.is_mouse_button_pressed(button),
            Control::Wheel => false,
        })
    }

    /// `matches` says whether `control` is one of `action`'s.
    ///
    /// Example: `if keys.matches(Action::Gun1, control) { ... }`
    pub fn matches(&self, action: Action, control: Control) -> bool {
        self.controls[&action].contains(&control)
    }

    /// `get_actions` gives every action `control` is bound to.
    ///
    /// Example: `let actions = keys.get_actions(Control::Wheel);`
    pub fn get_actions(&self, control: Control) -> Vec<Action> {
        ACTION_NAMES
            .iter()
            .map(|&(action, _)| action)
            .filter(|&action| self.matches(action, control))
            .collect()
    }

    /// `get_label` gives the name of `action`'s first control, for on-screen hints.
    ///
    /// Example: `let key = keys.get_label(Action::JoinBlue);`
    pub fn get_label(&self, action: Action) -> String {
        self.controls[&action][0].get_name()
    }

    /// `describe` gives a one-line summary of the controls for the HUD, such as "W/A/S/D move,
    /// Left Shift run, ...".
    ///
    /// Example: `let help = keys.describe();`
    pub fn describe(&self) -> String {
        HELP_GROUPS
            .iter()
            .map(|(actions, label)| {
                let names: Vec<String> = actions.iter().map(|&a| self.get_label(a)).collect();
                format!("{} {label}", names.join("/"))
            })
            .collect::<Vec<String>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    //! Tests of reading key bindings.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Controls | keys (the view switch among them), mouse buttons (shooting, aiming), the wheel | `builtin_bindings_read_every_kind_of_control` |
    //! | Mistakes | an unknown action, a missing action, an unknown key, the wheel on a held action | `mistakes_name_what_is_wrong` |

    use super::*;

    const PLAYER_YAML: &str = include_str!("../player_settings.yaml");

    /// `read_keys` reads the key bindings in the `keys` section of a player settings file.
    ///
    /// Example: `let keys = read_keys(PLAYER_YAML)?;`
    fn read_keys(text: &str) -> Result<KeyBindings> {
        let file: BTreeMap<String, serde_yaml::Value> = serde_yaml::from_str(text)?;
        let names: BTreeMap<String, Vec<String>> = serde_yaml::from_value(file["keys"].clone())?;
        KeyBindings::FromNames(&names, "test")
    }

    #[test]
    fn builtin_bindings_read_every_kind_of_control() {
        let keys: KeyBindings = read_keys(PLAYER_YAML).expect("the built-in keys");
        assert!(keys.matches(Action::Sprint, Control::Key(Scancode::LShift)));
        assert!(keys.matches(Action::Gun1, Control::Key(Scancode::_1)));
        assert!(keys.matches(Action::Shoot, Control::Mouse(MouseButton::Left)));
        assert!(keys.matches(Action::Aim, Control::Mouse(MouseButton::Right)));
        assert!(keys.matches(Action::Flashlight, Control::Key(Scancode::F)));
        assert!(keys.matches(Action::SwitchGun, Control::Wheel));
        assert!(keys.matches(Action::SwitchView, Control::Key(Scancode::V)));
        assert!(keys.describe().starts_with("W/A/S/D move, Left Shift run"));
    }

    #[test]
    fn mistakes_name_what_is_wrong() {
        for (old, new, expected) in [
            ("move_forward:", "move_forwards:", "move_forwards"),
            ("crouch: [Left Ctrl, Right Ctrl]", "", "crouch"),
            ("jump: [Space]", "jump: [Spacebar]", "Spacebar"),
            ("jump: [Space]", "jump: [Wheel]", "jump is held"),
        ] {
            let error: String = read_keys(&PLAYER_YAML.replace(old, new))
                .err()
                .expect("a mistake fails")
                .to_string();
            assert!(error.contains(expected), "{error}");
        }
    }
}
