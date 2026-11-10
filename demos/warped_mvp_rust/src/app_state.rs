//! `app_state.rs` is the app without its window: which screen is shown (`screens.rs`), every
//! menu's state, the games heard on the network, the match this player is in (a `MatchClient`),
//! the game this app hosts (`children.rs`), and the player's settings. The window passes it
//! inputs and draws its page; once a frame, `update` polls the network and follows the match into
//! and out of its rounds' levels.
//!
//! Interface: `App` (`New`, `handle`, `click`, `update`, `get_page`, `should_quit`, `set_map_held`,
//! `is_rebinding`), `Ports`, `get_address_shapes`, `get_credits_shapes`, `parse_address`.
//!
//! Example: `app.handle(MenuInput::Down); app.update(); let (shapes, size, pictures) = app.get_page();`
//!
//! Must know:
//! - Gotchas: a round's level plays through its library (`take_level_to_play`, then `end_level`);
//!   a level without one shows the stand-in screen (`game_menu::get_level_shapes`).

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Instant;

use warped_shared::app::{LevelEnd, LevelJoin, play_level_fn_t};
use warped_shared::game_settings::SettingsMessage;
use warped_shared::match_menu::{PageClick, get_click_target, get_match_lines};
use warped_shared::match_protocol::{
    LevelEntry, MATCH_PORT, MatchClient, MatchListener, MatchView, Phase,
};
use warped_shared::match_state::Team;
use warped_shared::network_scope;
use warped_shared::page::{Shape, get_page_point};
use warped_shared::player_settings::PerPlayerSettings;

use crate::children::{GameKind, HostedGame, Programs, describe_game_kind, make_run_folder};
use crate::game_menu::{
    GameAction, GameMenu, LevelMenu, LevelMenuAction, get_level_shapes, get_round,
};
use crate::game_settings_page::{GameSettingsOutcome, GameSettingsPage};
use crate::level_library::get_play_level;
use crate::levels_manifest::Picture;
use crate::main_menu::{MainAction, MainMenu, MainRow};
use crate::menu_page::{LINE, MARGIN, MenuInput, MenuRow, push_heading, push_rows, push_title};
use crate::player_settings_page::{PageOutcome, PlayerSettingsPage};
use crate::screens::{Navigator, Screen};

/// `DOUBLE_CLICK_COUNT` is SDL's click count from which a click on a level starts it at once.
const DOUBLE_CLICK_COUNT: u8 = 2;
const PLAY_RESEND_S: f64 = 0.5;
const SETTINGS_REQUEST_PERIOD_S: f64 = 0.5;
const SMALL_PAGE_SIZE: [f32; 2] = [640.0, 455.0];
const ADDRESS_HELP: &str =
    "Type the address (e.g. 192.168.1.20 or 192.168.1.20:47850). Return joins, Escape goes back.";
const CREDITS_HELP: &str = "Escape: back";
const CREDITS_LINES: [&str; 9] = [
    "Warped: a game of five levels in curved space, one window.",
    "",
    "TEXT      SDL3's built-in 8x8 debug font (libsdl.org, zlib)",
    "CODE      Rust crates: sdl3, objc2, objc2-metal, serde, clap, image,",
    "  each under its own open-source license",
    "LEVELS    each level's models, textures and sounds are credited on",
    "  its own menu's CREDITS page, as the level's README lists them",
    "",
    "Everything else is made in code in this repository.",
];

/// `Ports` is the UDP ports a game this app hosts uses (its match server's, and each round's
/// level server's), the port it listens on for games on the network, and whether it listens on
/// the loopback interface alone (tests: nothing open to the network).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ports {
    pub match_port: u16,
    pub level_port: u16,
    pub discovery_port: u16,
    pub is_loopback_only: bool,
}

/// `parse_address` reads a typed match server address, an IP address with an optional `:port`
/// (`MATCH_PORT` without one), or `None` if it isn't one.
///
/// Example: `assert_eq!(parse_address("10.0.0.2"), "10.0.0.2:47850".parse().ok());`
pub fn parse_address(text: &str) -> Option<SocketAddr> {
    text.parse::<SocketAddr>().ok().or_else(|| {
        text.parse::<IpAddr>()
            .ok()
            .map(|ip| SocketAddr::new(ip, MATCH_PORT))
    })
}

/// `get_address_shapes` gives the page where the player types an address, with `text` typed so
/// far and a message.
///
/// Example: `let (shapes, size) = get_address_shapes("192.168.1.", None);`
pub fn get_address_shapes(text: &str, message: Option<&str>) -> (Vec<Shape>, [f32; 2]) {
    let mut shapes: Vec<Shape> = Vec::new();
    push_title(
        &mut shapes,
        SMALL_PAGE_SIZE,
        "JOIN A GAME BY ITS ADDRESS",
        ADDRESS_HELP,
    );
    let rows: Vec<Option<MenuRow>> = vec![Some(MenuRow {
        text: format!("Address: {text}_"),
        is_enabled: true,
    })];
    push_rows(&mut shapes, 4.0 * LINE, &rows, Some(0));
    if let Some(message) = message {
        push_heading(&mut shapes, [MARGIN, 7.0 * LINE], message);
    }
    (shapes, SMALL_PAGE_SIZE)
}

/// `get_credits_shapes` gives the credits page.
///
/// Example: `let (shapes, size) = get_credits_shapes();`
pub fn get_credits_shapes() -> (Vec<Shape>, [f32; 2]) {
    let mut shapes: Vec<Shape> = Vec::new();
    push_title(&mut shapes, SMALL_PAGE_SIZE, "CREDITS", CREDITS_HELP);
    let rows: Vec<Option<MenuRow>> = CREDITS_LINES
        .iter()
        .map(|line| {
            Some(MenuRow {
                text: line.to_string(),
                is_enabled: true,
            })
        })
        .collect();
    push_rows(&mut shapes, 3.0 * LINE, &rows, None);
    (shapes, SMALL_PAGE_SIZE)
}

/// `App` is the whole app's state without its window.
pub struct App {
    navigator: Navigator,
    main_menu: MainMenu,
    game_menu: GameMenu,
    level_menu: LevelMenu,
    player_page: Option<PlayerSettingsPage>,
    game_settings_page: Option<GameSettingsPage>,
    settings_requested_at: Option<Instant>,
    address_text: String,
    address_message: Option<String>,
    levels: Vec<LevelEntry>,
    manifest_path: PathBuf,
    pictures: Vec<Option<Picture>>,
    listener: Result<MatchListener, String>,
    client: Option<MatchClient>,
    hosted: Option<HostedGame>,
    hosted_team: Option<Team>,
    pending_play: Option<(Option<usize>, Option<Instant>)>,
    /// `pending_now` is a double-clicked choice (W-204, double-click a level to enter at once)
    /// the match server is still to start without its break, and when it was last asked.
    pending_now: Option<(Option<usize>, Option<Instant>)>,
    left_round: Option<(usize, usize)>,
    level_round: Option<(usize, usize)>,
    is_level_started: bool,
    is_map_held: bool,
    player_settings: PerPlayerSettings,
    player_settings_path: PathBuf,
    ports: Ports,
    should_quit: bool,
}

impl App {
    /// `New` starts on the main menu with the manifest's `levels` (read from `manifest_path`)
    /// and their map `pictures`, the player's settings from `player_settings_path`, listening for
    /// games on `ports.discovery_port`.
    ///
    /// Example: `let app = App::New(levels, &manifest, pictures, mine, &path, ports);`
    #[allow(non_snake_case)]
    pub fn New(
        levels: Vec<LevelEntry>,
        manifest_path: &Path,
        pictures: Vec<Option<Picture>>,
        player_settings: PerPlayerSettings,
        player_settings_path: &Path,
        ports: Ports,
    ) -> App {
        App {
            navigator: Navigator::New(),
            main_menu: MainMenu::New(),
            game_menu: GameMenu::New(),
            level_menu: LevelMenu::New(),
            player_page: None,
            game_settings_page: None,
            settings_requested_at: None,
            address_text: String::new(),
            address_message: None,
            levels,
            manifest_path: manifest_path.to_path_buf(),
            pictures,
            listener: MatchListener::Bind(
                network_scope::get_listen_ip(ports.is_loopback_only),
                ports.discovery_port,
            )
            .map_err(|error| error.to_string()),
            client: None,
            hosted: None,
            hosted_team: None,
            pending_play: None,
            pending_now: None,
            left_round: None,
            level_round: None,
            is_level_started: false,
            is_map_held: false,
            player_settings,
            player_settings_path: player_settings_path.to_path_buf(),
            ports,
            should_quit: false,
        }
    }

    /// `should_quit` says whether the player chose to quit.
    ///
    /// Example: `while !app.should_quit() { ... }`
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// `set_map_held` says whether the map key (Tab) is held.
    ///
    /// Example: `app.set_map_held(true);`
    pub fn set_map_held(&mut self, is_held: bool) {
        self.is_map_held = is_held;
    }

    /// `is_rebinding` says whether the player settings page waits for a control to bind.
    ///
    /// Example: `if app.is_rebinding() { ... }`
    pub fn is_rebinding(&self) -> bool {
        self.navigator.get_screen() == Screen::PlayerSettings
            && self
                .player_page
                .as_ref()
                .is_some_and(PlayerSettingsPage::is_rebinding)
    }

    /// `get_view` gives the latest view of the match this player is in.
    ///
    /// Example: `let view = app.get_view();`
    fn get_view(&self) -> Option<&MatchView> {
        self.client.as_ref().and_then(|client| client.view.as_ref())
    }

    /// `get_team` gives the team this player plays for in the match they are in.
    ///
    /// Example: `let team = app.get_team();`
    fn get_team(&self) -> Option<Team> {
        self.client.as_ref().and_then(|client| client.team)
    }

    /// `join` joins the match server at `server` (as `team`, if known) and shows its game menu.
    ///
    /// Example: `self.join(server, None)?;`
    fn join(&mut self, server: SocketAddr, team: Option<Team>) -> Result<(), String> {
        let client: MatchClient =
            MatchClient::Connect(server, team).map_err(|error| error.to_string())?;
        self.client = Some(client);
        self.game_menu = GameMenu::New();
        self.left_round = None;
        self.level_round = None;
        self.navigator.enter_game();
        Ok(())
    }

    /// `leave` leaves the match (the server forgets this player after a few seconds) for the main
    /// menu; a game this app hosts goes on.
    ///
    /// Example: `self.leave();`
    fn leave(&mut self) {
        self.client = None;
        self.pending_play = None;
        self.pending_now = None;
        self.game_settings_page = None;
        self.navigator.leave_game();
    }

    /// `stop_hosting` stops the game this app hosts, if any; it fails if one of the game's
    /// processes is still alive afterwards.
    ///
    /// Example: `app.stop_hosting()?;`
    pub fn stop_hosting(&mut self) -> Result<(), String> {
        match self.hosted.take() {
            Some(mut hosted) => hosted.stop().map_err(|error| error.to_string()),
            None => Ok(()),
        }
    }

    /// `request_play` asks the match server, as the game master, to play `choice` (a level's
    /// index freely, or the match with `None`), sending it until the server's view shows it.
    ///
    /// Example: `app.request_play(Some(2));`
    pub fn request_play(&mut self, choice: Option<usize>) {
        if self.pending_play.map(|(pending, _)| pending) != Some(choice) {
            self.pending_play = Some((choice, None));
        }
    }

    /// `is_in_game_menu` says whether the game menu is shown (no level, no page over it).
    ///
    /// Example: `if app.is_in_game_menu() { ... }`
    pub fn is_in_game_menu(&self) -> bool {
        self.navigator.get_screen() == Screen::GameMenu
    }

    /// `start_game` hosts a game of `kind` playing `choice` (the match, or one level) and joins
    /// it: alone or with bots, the player is blue at once (the allies are blue too); hosting for
    /// others, they pick a team in the game menu.
    ///
    /// Example: `self.start_game(GameKind::Alone, Some(2))?;`
    pub fn start_game(&mut self, kind: GameKind, choice: Option<usize>) -> Result<(), String> {
        if self.hosted.is_some() {
            return Err("You already host a game: go back to it, or stop it first.".to_string());
        }
        let team: Option<Team> = (kind != GameKind::Host).then_some(Team::Blue);
        let programs: Programs =
            Programs::Find(&self.manifest_path).map_err(|error| error.to_string())?;
        let folder: PathBuf =
            make_run_folder("warped", &format_utc_now()).map_err(|error| error.to_string())?;
        let hosted: HostedGame = HostedGame::Start(
            &programs,
            kind,
            team.unwrap_or(Team::Blue),
            (self.ports.match_port, self.ports.level_port),
            self.ports.is_loopback_only,
            &folder,
        )
        .map_err(|error| error.to_string())?;
        let server: SocketAddr = hosted.server;
        self.hosted = Some(hosted);
        self.hosted_team = team;
        self.join(server, team)?;
        self.pending_play = choice.map(|level| (Some(level), None));
        Ok(())
    }

    /// `set_team` makes this player play for `team`, moving a lone player's sitting-out bot.
    ///
    /// Example: `self.set_team(Team::Red)?;`
    fn set_team(&mut self, team: Team) -> Result<(), String> {
        let Some(client) = self.client.as_mut() else {
            return Ok(());
        };
        client.team = Some(team);
        if let Some(hosted) = &mut self.hosted
            && hosted.server == client.server
        {
            self.hosted_team = Some(team);
            hosted
                .move_sit_out_bot(team)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// `open_player_settings` opens the player's settings page over the screen shown.
    ///
    /// Example: `self.open_player_settings();`
    fn open_player_settings(&mut self) {
        self.player_page = Some(PlayerSettingsPage::New(
            &self.player_settings,
            &self.player_settings_path,
        ));
        self.navigator.open(Screen::PlayerSettings);
    }

    /// `open_game_settings` opens the game master's settings page over the game menu; it asks the
    /// match server for the settings at once.
    ///
    /// Example: `app.open_game_settings();`
    pub fn open_game_settings(&mut self) {
        let level_names: Vec<String> = self.get_view().map_or(Vec::new(), |view| {
            view.levels.iter().map(|level| level.name.clone()).collect()
        });
        self.game_settings_page = Some(GameSettingsPage::New(level_names));
        self.settings_requested_at = None;
        self.navigator.open(Screen::GameSettings);
    }

    /// `has_game_settings` says whether the game master's settings page is open and shows the
    /// match server's settings (not still waiting for them).
    ///
    /// Example: `if app.has_game_settings() { ... }`
    pub fn has_game_settings(&self) -> bool {
        self.game_settings_page
            .as_ref()
            .is_some_and(|page| !page.is_waiting())
    }

    /// `describe_game` says who is in the game this player is in, for the play log: the game
    /// this app hosts (alone, with how many bots, for others) or one it joined, and the players
    /// on each team as the match server last counted them (bots too).
    ///
    /// Example: `let game: String = app.describe_game();`
    pub fn describe_game(&self) -> String {
        let kind: String = match &self.hosted {
            Some(hosted) => describe_game_kind(hosted.kind),
            None => "a game joined".to_string(),
        };
        match self.get_view() {
            Some(view) => format!(
                "{kind}; {} blue, {} red",
                view.team_counts[0], view.team_counts[1]
            ),
            None => kind,
        }
    }

    /// `has_unsaved_game_settings` says whether the game master's settings page is open with
    /// changes the match server doesn't have.
    ///
    /// Example: `if app.has_unsaved_game_settings() { ... }`
    pub fn has_unsaved_game_settings(&self) -> bool {
        self.game_settings_page
            .as_ref()
            .is_some_and(GameSettingsPage::has_unsaved_changes)
    }

    /// `step_game_setting` raises the number of the open game settings page's row `row_key`
    /// (`shared.<key>` or `level_<index>.<key>`) by one step, as the Right arrow does; it fails
    /// if the page isn't open or has no such row.
    ///
    /// Example: `app.step_game_setting("shared.respawn.start_s")?;`
    pub fn step_game_setting(&mut self, row_key: &str) -> Result<(), String> {
        self.game_settings_page
            .as_mut()
            .ok_or("the game settings page isn't open")?
            .select(row_key)?;
        self.handle_game_settings(MenuInput::Right);
        Ok(())
    }

    /// `save_game_settings` saves the open game settings page's changes as its SAVE SETTINGS row
    /// does: checked on the page, then sent to the match server.
    ///
    /// Example: `app.save_game_settings();`
    pub fn save_game_settings(&mut self) {
        let page: &mut GameSettingsPage = self
            .game_settings_page
            .as_mut()
            .expect("the game settings page is open");
        let outcome: GameSettingsOutcome = page.save();
        self.apply_game_settings_outcome(outcome);
    }

    /// `handle_game_settings` applies an input to the game master's settings page
    /// (`apply_game_settings_outcome`).
    ///
    /// Example: `self.handle_game_settings(MenuInput::Right);`
    fn handle_game_settings(&mut self, input: MenuInput) {
        let page: &mut GameSettingsPage = self
            .game_settings_page
            .as_mut()
            .expect("the game settings page is open");
        let outcome: GameSettingsOutcome = page.handle(input);
        self.apply_game_settings_outcome(outcome);
    }

    /// `apply_game_settings_outcome` does what the game settings page asked for after an input:
    /// nothing, send the match server a message (a save), or close the page.
    ///
    /// Example: `self.apply_game_settings_outcome(outcome);`
    fn apply_game_settings_outcome(&mut self, outcome: GameSettingsOutcome) {
        let page: &mut GameSettingsPage = self
            .game_settings_page
            .as_mut()
            .expect("the game settings page is open");
        match outcome {
            GameSettingsOutcome::Stay => {}
            GameSettingsOutcome::Send(message) => {
                let result: Result<(), String> = match &mut self.client {
                    Some(client) => client
                        .send_settings(&message)
                        .map_err(|error| error.to_string()),
                    None => Err("You left the game.".to_string()),
                };
                if let Err(error) = result {
                    page.show_message(error);
                }
            }
            GameSettingsOutcome::Close => {
                self.game_settings_page = None;
                self.navigator.go_back();
            }
        }
    }

    /// `click` applies a left click at `at` in a window of `size` (both in window points): on the
    /// game menu, a click on a level of the match's status plays it freely (the game master's
    /// choice), and during free play the "back to the match" button goes back. A double click
    /// (`click_count` 2 or more) also asks the match server to start at once, without the break
    /// before the round (W-204, double-click a level to enter at once).
    ///
    /// Example: `app.click([320.0, 120.0], [1280.0, 800.0], 1);`
    pub fn click(&mut self, at: [f32; 2], size: [f32; 2], click_count: u8) {
        if self.navigator.get_screen() != Screen::GameMenu {
            return;
        }
        let (_, page_size, _) = self.get_page();
        let page_at: [f32; 2] = get_page_point(at, size, page_size);
        let Some(target) = self
            .get_view()
            .and_then(|view| get_click_target(view, page_at))
        else {
            return;
        };
        let choice: Option<usize> = match target {
            PageClick::Level(level) => Some(level),
            PageClick::Match => None,
        };
        self.pending_play = Some((choice, None));
        self.game_menu.message = Some(if click_count >= DOUBLE_CLICK_COUNT {
            self.pending_now = Some((choice, None));
            "Asked the match server to start at once.".to_string()
        } else {
            "Asked the match server; everyone follows after the break (double-click to skip it)."
                .to_string()
        });
    }

    /// `handle` applies one input to the screen shown.
    ///
    /// Example: `app.handle(MenuInput::Activate);`
    pub fn handle(&mut self, input: MenuInput) {
        match self.navigator.get_screen() {
            Screen::MainMenu => self.handle_main_menu(input),
            Screen::JoinByAddress => self.handle_address(input),
            Screen::PlayerSettings => {
                let page: &mut PlayerSettingsPage = self
                    .player_page
                    .as_mut()
                    .expect("the settings page is open");
                if page.handle(input) == PageOutcome::Close {
                    self.player_page = None;
                    if self.player_settings_path.exists() {
                        match PerPlayerSettings::Load(&self.player_settings_path) {
                            Ok(settings) => self.player_settings = settings,
                            Err(error) => eprintln!("warped: {error}"),
                        }
                    }
                    self.navigator.go_back();
                }
            }
            Screen::Credits => {
                if input == MenuInput::Back {
                    self.navigator.go_back();
                }
            }
            Screen::GameSettings => self.handle_game_settings(input),
            Screen::GameMenu => self.handle_game_menu(input),
            Screen::Level => {
                if input == MenuInput::Back {
                    self.level_menu = LevelMenu::New();
                    self.navigator.go_back();
                }
            }
            Screen::LevelMenu => match self.level_menu.handle(input) {
                LevelMenuAction::Stay => {}
                LevelMenuAction::Resume => self.navigator.go_back(),
                LevelMenuAction::OpenPlayerSettings => self.open_player_settings(),
                LevelMenuAction::ExitToGameMenu => {
                    self.left_round = self.level_round;
                    self.level_round = None;
                    self.navigator.leave_level();
                }
                LevelMenuAction::Quit => self.should_quit = true,
            },
        }
    }

    /// `handle_main_menu` applies an input to the main menu.
    ///
    /// Example: `self.handle_main_menu(MenuInput::Activate);`
    fn handle_main_menu(&mut self, input: MenuInput) {
        let rows: Vec<MainRow> = self.get_main_rows();
        let result: Result<(), String> =
            match self.main_menu.handle(input, &rows, self.levels.len()) {
                MainAction::Stay => Ok(()),
                MainAction::Join(server) => self.join(server, None),
                MainAction::OpenJoinByAddress => {
                    self.address_message = None;
                    self.navigator.open(Screen::JoinByAddress);
                    Ok(())
                }
                MainAction::Start { kind, choice } => self.start_game(kind, choice),
                MainAction::BackToGame => {
                    let server: SocketAddr = self.hosted.as_ref().expect("a hosted game").server;
                    self.join(server, self.hosted_team)
                }
                MainAction::StopHosting => self.stop_hosting(),
                MainAction::OpenPlayerSettings => {
                    self.open_player_settings();
                    Ok(())
                }
                MainAction::OpenCredits => {
                    self.navigator.open(Screen::Credits);
                    Ok(())
                }
                MainAction::Quit => {
                    self.should_quit = true;
                    Ok(())
                }
            };
        if let Err(error) = result {
            self.main_menu.message = Some(error);
        }
    }

    /// `handle_address` applies an input to the address page.
    ///
    /// Example: `self.handle_address(MenuInput::Typed('1'));`
    fn handle_address(&mut self, input: MenuInput) {
        match input {
            MenuInput::Typed(c) => self.address_text.push(c),
            MenuInput::Erase => {
                self.address_text.pop();
            }
            MenuInput::Back => self.navigator.go_back(),
            MenuInput::Activate => match parse_address(&self.address_text) {
                Some(server) => {
                    if let Err(error) = self.join(server, None) {
                        self.address_message = Some(error);
                    }
                }
                None => {
                    self.address_message =
                        Some(format!("{:?} isn't an IP address", self.address_text));
                }
            },
            _ => {}
        }
    }

    /// `handle_game_menu` applies an input to the game menu.
    ///
    /// Example: `self.handle_game_menu(MenuInput::Activate);`
    fn handle_game_menu(&mut self, input: MenuInput) {
        let team: Option<Team> = self.get_team();
        let rows = self.game_menu.get_rows(team);
        let view: Option<MatchView> = self.get_view().cloned();
        let result: Result<(), String> =
            match self.game_menu.handle(input, &rows, team, view.as_ref()) {
                GameAction::Stay => Ok(()),
                GameAction::SetTeam(team) => self.set_team(team),
                GameAction::Rejoin => {
                    self.left_round = None;
                    Ok(())
                }
                GameAction::Play(choice) => {
                    self.pending_play = Some((choice, None));
                    self.game_menu.message = Some(
                        "Asked the match server; everyone follows after the break.".to_string(),
                    );
                    Ok(())
                }
                GameAction::OpenGameSettings => {
                    self.open_game_settings();
                    Ok(())
                }
                GameAction::OpenPlayerSettings => {
                    self.open_player_settings();
                    Ok(())
                }
                GameAction::Leave => {
                    self.leave();
                    Ok(())
                }
            };
        if let Err(error) = result {
            self.game_menu.message = Some(error);
        }
    }

    /// `poll_network` polls the network: the games heard, the match this player is in (sending the
    /// game master's pending choice until the server takes it), and the game this app hosts
    /// (whether its server stopped). A level calls it once a frame through its `MatchLink`.
    ///
    /// Example: `app.poll_network();`
    pub fn poll_network(&mut self) {
        if let Ok(listener) = &mut self.listener
            && let Err(error) = listener.poll()
        {
            self.listener = Err(error.to_string());
        }
        if let Some(hosted) = &mut self.hosted {
            let stopped: Option<String> = match hosted.has_stopped() {
                Ok(status) => status,
                Err(error) => Some(error.to_string()),
            };
            if let Some(why) = stopped {
                let message: String = format!(
                    "The game you hosted stopped ({why}); see {}",
                    hosted.run_folder.join("match_server.log").display()
                );
                let server: SocketAddr = hosted.server;
                self.hosted = None;
                if self
                    .client
                    .as_ref()
                    .is_some_and(|client| client.server == server)
                {
                    self.leave();
                }
                self.main_menu.message = Some(message);
            }
        }
        let Some(client) = &mut self.client else {
            return;
        };
        if let Err(error) = client.poll() {
            self.main_menu.message = Some(format!("Lost the game: {error}"));
            self.leave();
            return;
        }
        if let Some(page) = &mut self.game_settings_page {
            if self
                .settings_requested_at
                .is_none_or(|at| at.elapsed().as_secs_f64() >= SETTINGS_REQUEST_PERIOD_S)
            {
                if let Err(error) = client.send_settings(&SettingsMessage::Request) {
                    page.show_message(error.to_string());
                }
                self.settings_requested_at = Some(Instant::now());
            }
            if let Some(settings) = &client.settings {
                page.update(settings);
            }
        }
        if let (Some((choice, sent_at)), Some(view)) = (self.pending_play, &client.view) {
            if view.state.get_free_play().map(|free_play| free_play.level) == choice {
                self.pending_play = None;
            } else if sent_at.is_none_or(|at| at.elapsed().as_secs_f64() >= PLAY_RESEND_S) {
                if let Err(error) = client.send_play(choice, false) {
                    self.game_menu.message = Some(error.to_string());
                }
                self.pending_play = Some((choice, Some(Instant::now())));
            }
        }
        if let (Some((choice, sent_at)), Some(view)) = (self.pending_now, &client.view) {
            let is_chosen: bool =
                view.state.get_free_play().map(|free_play| free_play.level) == choice;
            let is_waiting: bool = matches!(view.phase, Phase::Intermission { .. });
            if is_chosen && !is_waiting {
                self.pending_now = None;
            } else if sent_at.is_none_or(|at| at.elapsed().as_secs_f64() >= PLAY_RESEND_S) {
                if let Err(error) = client.send_play(choice, true) {
                    self.game_menu.message = Some(error.to_string());
                }
                self.pending_now = Some((choice, Some(Instant::now())));
            }
        }
    }

    /// `update` polls the network once a frame (`poll_network`), then enters the round's level or
    /// leaves it as the match goes.
    ///
    /// Example: `app.update();`
    pub fn update(&mut self) {
        self.poll_network();
        let Some(client) = &self.client else {
            return;
        };
        let round: Option<(usize, usize)> = client.view.as_ref().and_then(get_round);
        let is_on_team: bool = client.team.is_some();
        if self.navigator.is_in_level() && round != self.level_round {
            self.level_round = None;
            self.navigator.leave_level();
        }
        if self.navigator.get_screen() == Screen::GameMenu
            && is_on_team
            && round.is_some()
            && round != self.left_round
        {
            self.level_round = round;
            self.left_round = None;
            self.is_level_started = false;
            self.navigator.enter_level();
        }
    }

    /// `take_level_to_play` gives, once per round entered, what the level shown should play
    /// (its manifest entry, its server, the team and the player's settings file) when it has a
    /// library (`level_library`); `None` otherwise, and then the stand-in screen shows.
    ///
    /// Example: `if let Some((play, join)) = app.take_level_to_play() { ... }`
    pub fn take_level_to_play(&mut self) -> Option<(play_level_fn_t, LevelJoin)> {
        if self.navigator.get_screen() != Screen::Level || self.is_level_started {
            return None;
        }
        let (_, level) = self.level_round?;
        let client: &MatchClient = self.client.as_ref()?;
        let view: &MatchView = client.view.as_ref()?;
        let Phase::Playing { port, .. } = view.phase else {
            return None;
        };
        let entry: LevelEntry = view.levels.get(level)?.clone();
        let play: play_level_fn_t = get_play_level(&entry)?;
        self.is_level_started = true;
        Some((
            play,
            LevelJoin {
                level: entry,
                server: SocketAddr::new(client.server.ip(), port),
                team: client.team?,
                player_settings_path: self.player_settings_path.clone(),
            },
        ))
    }

    /// `is_level_round_over` says whether the round the level plays has ended (another round, a
    /// break, or the match left), after `poll_network`.
    ///
    /// Example: `if app.is_level_round_over() { ... }`
    pub fn is_level_round_over(&self) -> bool {
        self.get_view().and_then(get_round) != self.level_round || self.level_round.is_none()
    }

    /// `get_match_lines` gives the in-level menu's MATCH page for this player, or `None` before
    /// the server's first view.
    ///
    /// Example: `menu.set_match_lines(app.get_match_lines());`
    pub fn get_match_lines(&self) -> Option<Vec<(String, bool)>> {
        Some(get_match_lines(self.get_view()?, self.get_team()))
    }

    /// `get_level_view` gives the latest view of the match, for a level's `MatchLink`.
    ///
    /// Example: `let view = app.get_level_view();`
    pub fn get_level_view(&self) -> Option<&MatchView> {
        self.get_view()
    }

    /// `end_level` goes back to the game menu after a level returned `result`: when the round
    /// ended, the menu shows the match's progress; when the player left, the round isn't joined
    /// again (the game menu's Rejoin does); closing the window quits; a failure shows on the game
    /// menu. The player's settings file is read again, since the level's menu may have saved it.
    ///
    /// Example: `app.end_level(Ok(LevelEnd::LeaveLevel));`
    pub fn end_level(&mut self, result: Result<LevelEnd, String>) {
        match result {
            Ok(LevelEnd::RoundOver { .. }) => {}
            Ok(LevelEnd::LeaveLevel) => self.left_round = self.level_round,
            Ok(LevelEnd::CloseWindow) => self.should_quit = true,
            Err(error) => {
                self.left_round = self.level_round;
                self.game_menu.message = Some(format!("The level stopped: {error}"));
            }
        }
        self.level_round = None;
        if self.navigator.is_in_level() {
            self.navigator.leave_level();
        }
        if self.player_settings_path.exists() {
            match PerPlayerSettings::Load(&self.player_settings_path) {
                Ok(settings) => self.player_settings = settings,
                Err(error) => self.game_menu.message = Some(error.to_string()),
            }
        }
    }

    /// `get_main_rows` gives the main menu's rows now.
    ///
    /// Example: `let rows = self.get_main_rows();`
    fn get_main_rows(&self) -> Vec<MainRow> {
        let found = match &self.listener {
            Ok(listener) => listener.get_found(),
            Err(_) => &[],
        };
        self.main_menu.get_rows(found, self.hosted.is_some())
    }

    /// `get_view_pictures` gives the map pictures of `view`'s levels, found by name among the
    /// manifest's (a remote server may list them differently).
    ///
    /// Example: `let pictures = self.get_view_pictures(&view);`
    fn get_view_pictures(&self, view: &MatchView) -> Vec<Option<Picture>> {
        view.levels
            .iter()
            .map(|level| {
                self.levels
                    .iter()
                    .position(|known| known.name == level.name)
                    .and_then(|index| self.pictures[index].clone())
            })
            .collect()
    }

    /// `get_page` gives the screen shown: its shapes, its size in page units, and the pictures
    /// its `Thumbnail` shapes name.
    ///
    /// Example: `let (shapes, size, pictures) = app.get_page();`
    pub fn get_page(&self) -> (Vec<Shape>, [f32; 2], Vec<Option<Picture>>) {
        let team: Option<Team> = self.get_team();
        let view: Option<&MatchView> = self.get_view();
        let pictures: Vec<Option<Picture>> =
            view.map_or(Vec::new(), |view| self.get_view_pictures(view));
        let (shapes, size): (Vec<Shape>, [f32; 2]) = match self.navigator.get_screen() {
            Screen::MainMenu => {
                let problem: Option<&str> = self.listener.as_ref().err().map(String::as_str);
                self.main_menu
                    .get_shapes(&self.get_main_rows(), &self.levels, problem)
            }
            Screen::JoinByAddress => {
                get_address_shapes(&self.address_text, self.address_message.as_deref())
            }
            Screen::PlayerSettings => self
                .player_page
                .as_ref()
                .expect("the settings page is open")
                .get_shapes(),
            Screen::Credits => get_credits_shapes(),
            Screen::GameSettings => self
                .game_settings_page
                .as_ref()
                .expect("the game settings page is open")
                .get_shapes(),
            Screen::GameMenu => {
                let label: String = self
                    .client
                    .as_ref()
                    .map_or(String::new(), |client| client.server.to_string());
                self.game_menu
                    .get_shapes(&self.game_menu.get_rows(team), team, view, &label)
            }
            Screen::Level => match (view, team, self.level_round) {
                (Some(view), Some(team), Some(round)) => {
                    get_level_shapes(view, team, round, self.is_map_held)
                }
                _ => self.level_menu.get_shapes(view, team),
            },
            Screen::LevelMenu => self.level_menu.get_shapes(view, team),
        };
        (shapes, size, pictures)
    }
}

/// `format_utc_now` gives the time now in UTC as `YYYYMMDD-HHMMSS`, a run folder's id.
///
/// Example: `let run_id = format_utc_now();`
pub fn format_utc_now() -> String {
    const SECONDS_PER_DAY: u64 = 86_400;
    const DAYS_PER_ERA: i64 = 146_097;
    const UNIX_DAY_OFFSET: i64 = 719_468;
    let seconds: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs();
    let (days, of_day): (i64, u64) = (
        (seconds / SECONDS_PER_DAY) as i64,
        seconds % SECONDS_PER_DAY,
    );
    let z: i64 = days + UNIX_DAY_OFFSET;
    let era: i64 = z.div_euclid(DAYS_PER_ERA);
    let day_of_era: i64 = z - era * DAYS_PER_ERA;
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
        of_day / 3600,
        of_day / 60 % 60,
        of_day % 60
    )
}

#[cfg(test)]
mod tests {
    //! Unit tests for `app_state.rs`: the app's flow through a real match server's protocol, with
    //! the server stood in for by a socket on this test's ports (48306 to 48309).
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Address | an IP, an IP with a port, garbage | `addresses_parse_with_the_default_port` |
    //! | Following the match | no team (stays in the game menu), a round starts (level), Escape (in-level menu), exit (game menu, not re-entered), rejoin (level), the round ends (game menu), Escape (main menu) | `the_app_follows_the_match_into_and_out_of_levels` |
    //! | Game master | a choice sent until the server's view shows it | `the_game_masters_choice_is_sent_until_taken` |
    //! | Clicks on the game menu | elsewhere (nothing), on the first level (free play there, after the break), a double click on it (also asked to start at once) | `clicking_a_level_plays_it` |
    //! | Game master's settings | the page asks, shows the answer, sends a step up, Escape back to the game menu (UDP 48392, 48393) | `the_game_masters_settings_go_to_the_match_server` |
    //! | Run id | the format | `run_ids_are_utc_times` |

    use super::*;
    use std::net::UdpSocket;
    use std::time::Duration;
    use warped_shared::game_settings::{GameSettings, Overrides, parse_settings_message};
    use warped_shared::match_protocol::{Phase, parse_play};
    use warped_shared::match_state::MatchState;

    /// `FakeServer` answers the app's hellos with views it chooses, on a test port.
    struct FakeServer {
        socket: UdpSocket,
    }

    impl FakeServer {
        /// `answer` reads every datagram the app sent and answers each hello with `view`, giving
        /// the play choices it heard.
        ///
        /// Example: `let plays = server.answer(&view);`
        fn answer(&self, view: &MatchView) -> Vec<Option<usize>> {
            let mut buffer: Vec<u8> = vec![0; 65_000];
            let mut plays: Vec<Option<usize>> = Vec::new();
            while let Ok((length, sender)) = self.socket.recv_from(&mut buffer) {
                if let Ok(choice) = parse_play(&buffer[..length], view.levels.len()) {
                    plays.push(choice);
                }
                self.socket
                    .send_to(&serde_json::to_vec(view).unwrap(), sender)
                    .unwrap();
            }
            plays
        }
    }

    /// `make_app` gives an app with the shipped manifest's levels, listening on a test port.
    ///
    /// Example: `let app = make_app(48306);`
    fn make_app(discovery_port: u16) -> App {
        use crate::levels_manifest::{get_levels_manifest_path, load_levels};
        let manifest: PathBuf = get_levels_manifest_path();
        let levels: Vec<LevelEntry> = load_levels(&manifest).unwrap();
        let pictures: Vec<Option<Picture>> = vec![None; levels.len()];
        let ports: Ports = Ports {
            match_port: 48_308,
            level_port: 48_309,
            discovery_port,
            is_loopback_only: true,
        };
        let path: PathBuf =
            std::env::temp_dir().join("warped_app_test_settings_never_written.yaml");
        App::New(
            levels,
            &manifest,
            pictures,
            PerPlayerSettings::Builtin(),
            &path,
            ports,
        )
    }

    /// `make_view` gives a view of a new match of `app`'s levels in `phase`.
    ///
    /// Example: `let view = make_view(&app, Phase::WaitingForTeams);`
    fn make_view(app: &App, phase: Phase) -> MatchView {
        MatchView {
            state: MatchState::New(app.levels.len()),
            phase,
            levels: app.levels.clone(),
            team_counts: [1, 1],
            match_left_s: None,
        }
    }

    /// `exchange` lets the app say hello and get `view` back, a few times.
    ///
    /// Example: `exchange(&mut app, &server, &view);`
    fn exchange(app: &mut App, server: &FakeServer, view: &MatchView) -> Vec<Option<usize>> {
        let mut plays: Vec<Option<usize>> = Vec::new();
        let deadline: Instant = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            app.update();
            plays.extend(server.answer(view));
            std::thread::sleep(Duration::from_millis(10));
            app.update();
            if app.get_view() == Some(view) {
                break;
            }
        }
        assert_eq!(app.get_view(), Some(view));
        plays
    }

    /// `start_server` binds a fake match server on `port`.
    ///
    /// Example: `let server = start_server(48307);`
    fn start_server(port: u16) -> FakeServer {
        let socket: UdpSocket = UdpSocket::bind(("127.0.0.1", port)).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(5)))
            .unwrap();
        FakeServer { socket }
    }

    #[test]
    fn addresses_parse_with_the_default_port() {
        assert_eq!(parse_address("10.0.0.2"), "10.0.0.2:47850".parse().ok());
        assert_eq!(
            parse_address("10.0.0.2:48304"),
            "10.0.0.2:48304".parse().ok()
        );
        assert_eq!(parse_address("10.0.0"), None);
    }

    #[test]
    fn the_app_follows_the_match_into_and_out_of_levels() {
        let server: FakeServer = start_server(48_307);
        let mut app: App = make_app(48_306);
        app.join("127.0.0.1:48307".parse().unwrap(), None).unwrap();
        let playing: MatchView = make_view(
            &app,
            Phase::Playing {
                round: 1,
                level: 2,
                port: 1,
            },
        );
        exchange(&mut app, &server, &playing);
        assert_eq!(app.navigator.get_screen(), Screen::GameMenu);
        app.set_team(Team::Red).unwrap();
        app.update();
        assert_eq!(app.navigator.get_screen(), Screen::Level);
        app.handle(MenuInput::Back);
        assert_eq!(app.navigator.get_screen(), Screen::LevelMenu);
        for _ in 0..2 {
            app.handle(MenuInput::Down);
        }
        app.handle(MenuInput::Activate);
        app.update();
        assert_eq!(app.navigator.get_screen(), Screen::GameMenu);
        app.handle_game_menu(MenuInput::Down);
        app.handle_game_menu(MenuInput::Activate);
        app.update();
        assert_eq!(app.navigator.get_screen(), Screen::Level);
        let over: MatchView = make_view(
            &app,
            Phase::RoundOver {
                round: 1,
                left_s: 3.0,
            },
        );
        let mut over: MatchView = over;
        over.state.record_round(Team::Red);
        exchange(&mut app, &server, &over);
        assert_eq!(app.navigator.get_screen(), Screen::GameMenu);
        app.handle(MenuInput::Back);
        assert_eq!(app.navigator.get_screen(), Screen::MainMenu);
        assert!(app.client.is_none());
    }

    #[test]
    fn the_game_masters_choice_is_sent_until_taken() {
        let server: FakeServer = start_server(48_305);
        let mut app: App = make_app(48_304);
        app.join("127.0.0.1:48305".parse().unwrap(), Some(Team::Blue))
            .unwrap();
        let waiting: MatchView = make_view(&app, Phase::WaitingForTeams);
        exchange(&mut app, &server, &waiting);
        app.pending_play = Some((Some(3), None));
        let plays: Vec<Option<usize>> = exchange(&mut app, &server, &waiting);
        assert_eq!(plays.first(), Some(&Some(3)));
        let mut freely: MatchView = waiting.clone();
        freely.state.start_free_play(3);
        exchange(&mut app, &server, &freely);
        app.update();
        assert!(app.pending_play.is_none());
    }

    #[test]
    fn the_game_masters_settings_go_to_the_match_server() {
        let server: FakeServer = start_server(48_392);
        let mut app: App = make_app(48_393);
        app.join("127.0.0.1:48392".parse().unwrap(), Some(Team::Blue))
            .unwrap();
        let waiting: MatchView = make_view(&app, Phase::WaitingForTeams);
        exchange(&mut app, &server, &waiting);
        app.open_game_settings();
        assert_eq!(app.navigator.get_screen(), Screen::GameSettings);
        assert!(!app.has_game_settings());
        let mut settings: GameSettings = GameSettings::New(vec![
            Some(serde_json::json!({"respawn": {"delay_s": 3.0}})),
            Some(serde_json::json!({"respawn": {"delay_s": 3.0}})),
        ]);
        let mut buffer: Vec<u8> = vec![0; 65_000];
        let deadline: Instant = Instant::now() + Duration::from_secs(2);
        let mut changes: Vec<SettingsMessage> = Vec::new();
        let mut has_changed: bool = false;
        while Instant::now() < deadline && changes.is_empty() {
            app.update();
            while let Ok((length, sender)) = server.socket.recv_from(&mut buffer) {
                match parse_settings_message(&buffer[..length], 2) {
                    Ok(message) => {
                        settings.apply(&message).unwrap();
                        if message != SettingsMessage::Request {
                            changes.push(message);
                        }
                        server
                            .socket
                            .send_to(&settings.to_answer(), sender)
                            .unwrap();
                    }
                    Err(_) => {
                        let view: Vec<u8> = serde_json::to_vec(&waiting).unwrap();
                        server.socket.send_to(&view, sender).unwrap();
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(10));
            app.update();
            let page: &GameSettingsPage = app.game_settings_page.as_ref().unwrap();
            if !page.is_waiting() && !has_changed {
                for input in [MenuInput::Down, MenuInput::Down, MenuInput::Down] {
                    app.handle(input);
                }
                app.handle(MenuInput::Right);
                for input in [MenuInput::Up, MenuInput::Up, MenuInput::Up] {
                    app.handle(input);
                }
                app.handle(MenuInput::Activate);
                has_changed = true;
            }
        }
        let mut saved: Overrides = Overrides {
            shared: std::collections::BTreeMap::new(),
            levels: vec![std::collections::BTreeMap::new(); 2],
        };
        saved
            .shared
            .insert("respawn.delay_s".to_string(), serde_json::json!(3.1));
        assert_eq!(changes, [SettingsMessage::Save(saved)]);
        assert!(app.has_game_settings());
        assert!(app.describe_game().starts_with("a game joined; "));
        app.handle(MenuInput::Back);
        assert_eq!(app.navigator.get_screen(), Screen::GameMenu);
        assert!(app.game_settings_page.is_none());
        assert!(!app.has_game_settings());
    }

    #[test]
    fn clicking_a_level_plays_it() {
        let server: FakeServer = start_server(48_303);
        let mut app: App = make_app(48_302);
        app.join("127.0.0.1:48303".parse().unwrap(), Some(Team::Blue))
            .unwrap();
        let waiting: MatchView = make_view(&app, Phase::WaitingForTeams);
        exchange(&mut app, &server, &waiting);
        let (_, page_size, _) = app.get_page();
        let window: [f32; 2] = [page_size[0] * 2.0, page_size[1] * 2.0];
        app.click([1.0, 1.0], window, 1);
        assert!(app.pending_play.is_none());
        app.click([2.0 * 30.0, 2.0 * 100.0], window, 1);
        assert_eq!(app.pending_play, Some((Some(0), None)));
        assert_eq!(app.pending_now, None);
        let (_, page_size, _) = app.get_page();
        let window: [f32; 2] = [page_size[0] * 2.0, page_size[1] * 2.0];
        app.click([2.0 * 30.0, 2.0 * 100.0], window, 2);
        assert_eq!(app.pending_now, Some((Some(0), None)));
        assert!(app.game_menu.message.as_ref().unwrap().contains("at once"));
    }

    #[test]
    fn run_ids_are_utc_times() {
        let run_id: String = format_utc_now();
        assert_eq!(run_id.len(), "20261109-120000".len());
        assert!(run_id.starts_with("20"));
        assert_eq!(run_id.as_bytes()[8], b'-');
    }
}
