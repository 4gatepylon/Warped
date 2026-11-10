//! `match_menu.rs` is the match's progression page: the five levels in their line with who
//! defends each, whose wavefront it is and who captured it, both wavefronts, who attacks now, the
//! round history and the match's result. It keeps no window state: the window draws its lines.
//!
//! Interface: `get_progression_lines`, `MAX_LINE_COUNT` (the terminal's page),
//! `get_match_lines` (the in-level menu's MATCH page), `get_progression_shapes`,
//! `get_progression_shapes_with_footer`, `get_click_target` and `PageClick` (a click chooses a
//! level or the match), `Shape` (from `page.rs`),
//! `PAGE_SIZE`, `HUD_RGB`, `HIGHLIGHT_RGB`, `DIM_RGB`, `TEAM_RGB` (the window's page).
//!
//! Example: `let lines = get_progression_lines(&view, Some(Team::Blue));`
//!
//! Must know:
//! - A line is `(text, is_highlighted)`; the level being played and the match's result are
//!   highlighted.
//! - Shapes are in page units (`PAGE_SIZE`, the hyperbolic menu's 8-unit debug font); the window
//!   scales them to fit.

use crate::match_protocol::{MatchView, Phase, describe_outcome};
use crate::match_state::{FreePlay, MatchState, Outcome, Ownership, Round, Team};
pub use crate::page::Shape;
use crate::page::push_text;

pub const MAX_LINE_COUNT: usize = 34;
const SHOWN_ROUND_COUNT: usize = 9;
const NO_TEAM: &str = "-";

/// `format_team` gives a team's name, or `NO_TEAM`.
///
/// Example: `assert_eq!(format_team(None), "-");`
fn format_team(team: Option<Team>) -> &'static str {
    team.map_or(NO_TEAM, Team::get_name)
}

/// `format_time` gives seconds as `m:ss`.
///
/// Example: `assert_eq!(format_time(75.4), "1:15");`
fn format_time(seconds: f64) -> String {
    let whole: u64 = seconds.ceil() as u64;
    format!("{}:{:02}", whole / 60, whole % 60)
}

/// `format_depth` gives a level's place on the line: `purple`, or the defending team and depth.
///
/// Example: `let text = format_depth(&state, 0);`
fn format_depth(state: &MatchState, level: usize) -> String {
    match state.get_level_defender(level) {
        None => "purple".to_string(),
        Some(team) => format!("{} {}", team.get_name(), state.get_level_depth(level)),
    }
}

/// `format_status` gives a level's offence column: who captured it, whose wavefront it is, or
/// that nobody has reached it.
///
/// Example: `let text = format_status(&state.get_ownership(3), &state, 3);`
fn format_status(ownership: &Ownership, state: &MatchState, level: usize) -> String {
    let won_round: Option<&Round> = state
        .rounds()
        .iter()
        .rev()
        .find(|round| round.level == level);
    match (ownership.captured_by, ownership.wavefront_of) {
        (Some(team), _) if level == state.get_purple_level() => {
            format!("won by {} (round 1, played once)", team.get_name())
        }
        (Some(team), _) => format!(
            "captured by {} (round {})",
            team.get_name(),
            won_round.map_or(0, |round| round.number)
        ),
        (None, Some(team)) => format!("{} wavefront", team.get_name()),
        (None, None) if level == state.get_purple_level() => "first round".to_string(),
        (None, None) => "not reached".to_string(),
    }
}

/// `describe_round_effect` says what a round in `view`'s history did to the match: the purple
/// round's winner becomes the attacker, an attacker that wins advances (or, in the round that ends
/// the match, wins the match), and a defender that wins holds and the roles swap.
///
/// Example: `let effect = describe_round_effect(&view, &round);`
fn describe_round_effect(view: &MatchView, round: &Round) -> &'static str {
    let is_match_won: bool = view.state.outcome() == Some(Outcome::Won(round.winner))
        && view.state.rounds().last() == Some(round);
    match round.attacker {
        None => "becomes the attacker",
        Some(team) if team == round.winner && is_match_won => "attacker wins the match",
        Some(team) if team == round.winner => "attacker advances",
        Some(_) => "defender holds, roles swap",
    }
}

/// `format_round` gives one line of the round history.
///
/// Example: `let line = format_round(&view, &round);`
fn format_round(view: &MatchView, round: &Round) -> String {
    let attack: String = match round.attacker {
        None => "no attacker".to_string(),
        Some(team) => format!("{} attacks", team.get_name()),
    };
    let effect: &str = describe_round_effect(view, round);
    format!(
        " #{:<2} {:<14} {:<12} won by {:<4} ({effect})",
        round.number,
        view.levels[round.level].name,
        attack,
        round.winner.get_name()
    )
}

/// `get_next_level` gives the level played next or now: the free play's level, else the match's
/// next level (`None` once the match is over).
///
/// Example: `let level = get_next_level(&view.state);`
fn get_next_level(state: &MatchState) -> Option<usize> {
    match state.get_free_play() {
        Some(free_play) => Some(free_play.level),
        None => state.get_next_level(),
    }
}

/// `format_phase` gives the line saying what the match is doing now.
///
/// Example: `let line = format_phase(&view);`
fn format_phase(view: &MatchView) -> String {
    let state: &MatchState = &view.state;
    if let Some(free_play) = state.get_free_play() {
        return format_free_phase(view, free_play);
    }
    let next: String = match state.get_next_level() {
        Some(level) => match state.get_attacker() {
            None => format!("{} (no attacker)", view.levels[level].name),
            Some(team) => format!(
                "{} ({} attacks, {} defends)",
                view.levels[level].name,
                team.get_name(),
                team.get_other().get_name()
            ),
        },
        None => "none".to_string(),
    };
    match &view.phase {
        Phase::WaitingForTeams => format!("Waiting for a player on each team; first: {next}"),
        Phase::Intermission { left_s } => {
            format!(
                "Round {} in {}: {next}",
                state.rounds().len() + 1,
                format_time(*left_s)
            )
        }
        Phase::Playing { round, .. } => format!("Round {round} now: {next}"),
        Phase::RoundOver { round, left_s } => {
            let winner: Team = state.rounds()[round - 1].winner;
            format!(
                "Round {round} won by {}; next in {}",
                winner.get_name(),
                format_time(*left_s)
            )
        }
        Phase::Over => "The match is over".to_string(),
    }
}

/// `format_free_phase` gives the line saying what free play on one level is doing now, with
/// each team's wins there.
///
/// Example: `let line = format_free_phase(&view, free_play);`
fn format_free_phase(view: &MatchView, free_play: FreePlay) -> String {
    let name: &str = &view.levels[free_play.level].name;
    let [blue, red] = free_play.wins;
    let now: String = match &view.phase {
        Phase::WaitingForTeams => "waiting for a player on each team".to_string(),
        Phase::Intermission { left_s } => format!("next round in {}", format_time(*left_s)),
        Phase::Playing { round, .. } => format!("round {round} now"),
        Phase::RoundOver { round, left_s } => {
            format!("round {round} won; next in {}", format_time(*left_s))
        }
        Phase::Over => "over".to_string(),
    };
    format!("Free play on {name}: {now}. Wins: blue {blue}, red {red}")
}

/// `get_level_markers` gives the line of the map as a strip, blue's deepest level on the left:
/// `[B]` a level blue defends, `(P)` the purple level, `*` on the level played next or now.
///
/// Example: `let strip = get_level_markers(&view);`
fn get_level_markers(view: &MatchView) -> String {
    let state: &MatchState = &view.state;
    (0..state.level_count())
        .map(|level| {
            let short: String = view.levels[level].name.chars().take(6).collect();
            let mark: &str = if get_next_level(state) == Some(level) {
                "*"
            } else {
                ""
            };
            match state.get_level_defender(level) {
                None => format!("({mark}{short})"),
                Some(_) => format!("[{mark}{short}]"),
            }
        })
        .collect::<Vec<String>>()
        .join("--")
}

/// `get_progression_lines` gives the progression page for `view`, for a player of `team` (`None`
/// before they pick one): at most `MAX_LINE_COUNT` lines.
///
/// Example: `let lines = get_progression_lines(&view, Some(Team::Red));`
pub fn get_progression_lines(view: &MatchView, team: Option<Team>) -> Vec<(String, bool)> {
    let state: &MatchState = &view.state;
    let mut lines: Vec<(String, bool)> = Vec::new();
    let mut push = |text: String, is_highlighted: bool| lines.push((text, is_highlighted));
    let you: String = match team {
        Some(team) => format!("you play {}", team.get_name()),
        None => "press 1 for blue, 2 for red".to_string(),
    };
    push(format!("WARPED MATCH - {you}"), false);
    push(format_phase(view), view.phase != Phase::WaitingForTeams);
    let time: String = view
        .match_left_s
        .map_or("starts with round 1".to_string(), format_time);
    push(
        format!(
            "Players: blue {}, red {}. Match time left: {time}",
            view.team_counts[0], view.team_counts[1]
        ),
        false,
    );
    push(String::new(), false);
    push(get_level_markers(view), false);
    push(String::new(), false);
    push(
        format!(" {:<15} {:<8} {:<8} Offence", "Level", "Place", "Defence"),
        false,
    );
    let playing: Option<usize> = match view.phase {
        Phase::Playing { level, .. } => Some(level),
        _ => None,
    };
    for (level, entry) in view.levels.iter().enumerate() {
        let ownership: Ownership = state.get_ownership(level);
        let stand_in: &str = if entry.stand_in_for.is_some() {
            " [stand-in]"
        } else {
            ""
        };
        push(
            format!(
                " {:<15} {:<8} {:<8} {}{stand_in}",
                entry.name,
                format_depth(state, level),
                format_team(ownership.defender),
                format_status(&ownership, state, level)
            ),
            playing == Some(level),
        );
    }
    push(String::new(), false);
    for team in [Team::Blue, Team::Red] {
        let wavefront: usize = state.get_wavefront(team);
        push(
            format!(
                "{} wavefront: {} (depth {} on {}'s side)",
                team.get_name(),
                view.levels[wavefront].name,
                state.get_level_depth(wavefront),
                team.get_other().get_name()
            ),
            false,
        );
    }
    push(
        format!(
            "Offensive ownership (attacker): {}. Defensive ownership (defender): {}.",
            format_team(state.get_attacker()),
            format_team(state.get_defender())
        ),
        false,
    );
    push(String::new(), false);
    let rounds: &[Round] = state.rounds();
    let hidden_count: usize = rounds.len().saturating_sub(SHOWN_ROUND_COUNT);
    push(
        match hidden_count {
            0 => format!("History ({} rounds):", rounds.len()),
            _ => format!(
                "History ({} rounds, last {SHOWN_ROUND_COUNT}):",
                rounds.len()
            ),
        },
        false,
    );
    for round in &rounds[hidden_count..] {
        push(format_round(view, round), false);
    }
    if let Some(outcome) = state.outcome() {
        push(String::new(), false);
        push(format!("Result: {}", describe_outcome(outcome)), true);
    }
    push(String::new(), false);
    push(
        "Escape quits. The level opens in its own window each round.".to_string(),
        false,
    );
    lines
}

/// `MATCH_PAGE_MAX_CHAR_COUNT` and `MATCH_PAGE_MAX_LINE_COUNT` are the in-level menu's MATCH page's
/// size (`Menu::set_match_lines`), in characters and lines.
pub const MATCH_PAGE_MAX_CHAR_COUNT: usize = 64;
pub const MATCH_PAGE_MAX_LINE_COUNT: usize = 22;

/// `get_match_lines` gives the in-level menu's MATCH page for a player of `team`: the progression
/// page's lines (`get_progression_lines`), each cut to `MATCH_PAGE_MAX_CHAR_COUNT` characters,
/// at most `MATCH_PAGE_MAX_LINE_COUNT` of them.
///
/// Example: `menu.set_match_lines(Some(get_match_lines(&view, Some(Team::Blue))));`
pub fn get_match_lines(view: &MatchView, team: Option<Team>) -> Vec<(String, bool)> {
    get_progression_lines(view, team)
        .into_iter()
        .take(MATCH_PAGE_MAX_LINE_COUNT)
        .map(|(line, is_highlighted)| {
            (
                line.chars().take(MATCH_PAGE_MAX_CHAR_COUNT).collect(),
                is_highlighted,
            )
        })
        .collect()
}

pub const PAGE_SIZE: [f32; 2] = [640.0, 455.0];
pub const HUD_RGB: [u8; 3] = [235, 240, 250];
pub const HIGHLIGHT_RGB: [u8; 3] = [255, 90, 80];
pub const DIM_RGB: [u8; 3] = [150, 158, 175];
pub const TEAM_RGB: [[u8; 3]; 2] = [[90, 150, 255], [255, 90, 80]];
const PURPLE_RGB: [u8; 3] = [190, 120, 255];
const BACKDROP_RGBA: [u8; 4] = [8, 10, 16, 255];
const PANEL_ALPHA: u8 = 215;
const MARGIN: f32 = 20.0;
const LINE: f32 = 12.0;
const GLYPH: f32 = 8.0;
const LEVEL_TOP: f32 = 64.0;
const THUMBNAIL_PAD: f32 = 5.0;
const THUMBNAIL_HEIGHT: f32 = 50.0;
const LEVEL_HEIGHT: f32 = 92.0 + THUMBNAIL_PAD + THUMBNAIL_HEIGHT;
const LEVEL_GAP: f32 = 12.0;
const CAPTURE_BAR: f32 = 6.0;
const HISTORY_TOP: f32 = 232.0 + THUMBNAIL_PAD + THUMBNAIL_HEIGHT;
const CHIP_WIDTH: f32 = 72.0;
const LEVEL_WINDOW_FOOTER: &str =
    "Escape quits. M: back to the match. Levels open in their own window.";
const MATCH_BUTTON_TEXT: &str = "[ back to the match ]";
const MATCH_BUTTON_PAD: f32 = 4.0;
const CLICK_HINT: &str = "Click a level to play it now; the wavefronts show the match's way.";

/// `get_team_rgb` gives a team's colour (the minimap's), or the purple level's for `None`.
///
/// Example: `assert_eq!(get_team_rgb(Some(Team::Blue)), [90, 150, 255]);`
fn get_team_rgb(team: Option<Team>) -> [u8; 3] {
    team.map_or(PURPLE_RGB, |team| TEAM_RGB[team.get_index()])
}

/// `get_dark_rgba` gives `rgb` darkened to a panel's fill.
///
/// Example: `let fill = get_dark_rgba([90, 150, 255]);`
fn get_dark_rgba(rgb: [u8; 3]) -> [u8; 4] {
    let [r, g, b] = rgb.map(|c| (u16::from(c) * 28 / 100) as u8);
    [r, g, b, PANEL_ALPHA]
}

/// `format_short_status` gives a level panel's offence line, at most 13 characters.
///
/// Example: `let text = format_short_status(&state.get_ownership(3), &state, 3);`
fn format_short_status(ownership: &Ownership, state: &MatchState, level: usize) -> String {
    let won_round: Option<usize> = state
        .rounds()
        .iter()
        .rev()
        .find(|round| round.level == level)
        .map(|round| round.number);
    match (ownership.captured_by, won_round) {
        (Some(team), Some(round)) => format!("{} took #{round}", team.get_name()),
        _ if level == state.get_purple_level() => "played first".to_string(),
        _ => "not taken".to_string(),
    }
}

/// `PageClick` is what a click on the progression page chooses: free play on a level (its
/// index), or back to the match.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageClick {
    Level(usize),
    Match,
}

/// `get_level_box` gives the top left corner and size, in page units, of level `level`'s panel
/// in a row of `level_count`.
///
/// Example: `let (min, size) = get_level_box(5, 2);`
fn get_level_box(level_count: usize, level: usize) -> ([f32; 2], [f32; 2]) {
    let width: f32 =
        (PAGE_SIZE[0] - 2.0 * MARGIN - LEVEL_GAP * (level_count - 1) as f32) / level_count as f32;
    (
        [MARGIN + level as f32 * (width + LEVEL_GAP), LEVEL_TOP],
        [width, LEVEL_HEIGHT],
    )
}

/// `get_match_button_box` gives the "back to the match" button's top left corner and size.
///
/// Example: `let (min, size) = get_match_button_box();`
fn get_match_button_box() -> ([f32; 2], [f32; 2]) {
    let size: [f32; 2] = [
        MATCH_BUTTON_TEXT.len() as f32 * GLYPH + 2.0 * MATCH_BUTTON_PAD,
        LINE + MATCH_BUTTON_PAD,
    ];
    let y: f32 = LEVEL_TOP + LEVEL_HEIGHT + 14.0 + 2.0 * LINE + 6.0;
    ([PAGE_SIZE[0] - MARGIN - size[0], y], size)
}

/// `is_inside` says whether `at` is inside the box at `min` of `size`.
///
/// Example: `assert!(is_inside([1.0, 1.0], [0.0, 0.0], [2.0, 2.0]));`
fn is_inside(at: [f32; 2], min: [f32; 2], size: [f32; 2]) -> bool {
    (0..2).all(|a| at[a] >= min[a] && at[a] < min[a] + size[a])
}

/// `get_click_target` gives what a click at `at` (page units, `page::get_page_point`) on
/// `view`'s progression page chooses: a level's panel plays it freely, and during free play the
/// "back to the match" button goes back; anywhere else `None`.
///
/// Example: `if let Some(PageClick::Level(level)) = get_click_target(&view, at) { ... }`
pub fn get_click_target(view: &MatchView, at: [f32; 2]) -> Option<PageClick> {
    let count: usize = view.state.level_count();
    if let Some(level) = (0..count).find(|&level| {
        let (min, size) = get_level_box(count, level);
        is_inside(at, min, size)
    }) {
        return Some(PageClick::Level(level));
    }
    let (min, size) = get_match_button_box();
    (view.state.get_free_play().is_some() && is_inside(at, min, size)).then_some(PageClick::Match)
}

/// `push_level_row` adds the five levels in a row: one panel each, tinted by its side, outlined on
/// the level played next (red while it is being played), with its defender, a bar in the colour
/// of the team that captured it, and each team's wavefront marked under it.
///
/// Example: `push_level_row(&mut shapes, &view);`
fn push_level_row(shapes: &mut Vec<Shape>, view: &MatchView) {
    let state: &MatchState = &view.state;
    let count: usize = state.level_count();
    let width: f32 = get_level_box(count, 0).1[0];
    let playing: Option<usize> = match view.phase {
        Phase::Playing { level, .. } => Some(level),
        _ => None,
    };
    push_text(
        shapes,
        [MARGIN, LEVEL_TOP - LINE - 2.0],
        "blue's side",
        TEAM_RGB[0],
    );
    push_text(
        shapes,
        [PAGE_SIZE[0] - MARGIN - 10.0 * GLYPH, LEVEL_TOP - LINE - 2.0],
        "red's side",
        TEAM_RGB[1],
    );
    for (level, entry) in view.levels.iter().enumerate().take(count) {
        let left: f32 = get_level_box(count, level).0[0];
        let defender: Option<Team> = state.get_level_defender(level);
        let ownership: Ownership = state.get_ownership(level);
        let border_rgb: Option<[u8; 3]> = if playing == Some(level) {
            Some(HIGHLIGHT_RGB)
        } else if get_next_level(state) == Some(level) {
            Some(HUD_RGB)
        } else {
            None
        };
        if level > 0 {
            shapes.push(Shape::Panel {
                min: [left - LEVEL_GAP, LEVEL_TOP + LEVEL_HEIGHT / 2.0 - 1.0],
                size: [LEVEL_GAP, 2.0],
                rgba: [DIM_RGB[0], DIM_RGB[1], DIM_RGB[2], 255],
                border_rgb: None,
            });
        }
        shapes.push(Shape::Panel {
            min: [left, LEVEL_TOP],
            size: [width, LEVEL_HEIGHT],
            rgba: get_dark_rgba(get_team_rgb(defender)),
            border_rgb,
        });
        shapes.push(Shape::Thumbnail {
            level,
            min: [left + THUMBNAIL_PAD, LEVEL_TOP + THUMBNAIL_PAD],
            size: [width - 2.0 * THUMBNAIL_PAD, THUMBNAIL_HEIGHT],
        });
        let text_left: f32 = left + 5.0;
        let mut y: f32 = LEVEL_TOP + THUMBNAIL_PAD + THUMBNAIL_HEIGHT + 6.0;
        let name: String = entry.name.chars().take(13).collect();
        push_text(shapes, [text_left, y], name, HUD_RGB);
        y += LINE;
        let place: String = match defender {
            None => "purple, once".to_string(),
            Some(team) => format!("{} {}", team.get_name(), state.get_level_depth(level)),
        };
        push_text(shapes, [text_left, y], place, get_team_rgb(defender));
        y += LINE + 4.0;
        push_text(shapes, [text_left, y], "defends", DIM_RGB);
        y += LINE;
        push_text(
            shapes,
            [text_left, y],
            format_team(ownership.defender),
            ownership
                .defender
                .map_or(DIM_RGB, |team| TEAM_RGB[team.get_index()]),
        );
        y += LINE + 4.0;
        push_text(
            shapes,
            [text_left, y],
            format_short_status(&ownership, state, level),
            ownership
                .captured_by
                .map_or(DIM_RGB, |team| TEAM_RGB[team.get_index()]),
        );
        if let Some(team) = ownership.captured_by {
            let [r, g, b] = TEAM_RGB[team.get_index()];
            shapes.push(Shape::Panel {
                min: [left, LEVEL_TOP + LEVEL_HEIGHT - CAPTURE_BAR],
                size: [width, CAPTURE_BAR],
                rgba: [r, g, b, 255],
                border_rgb: None,
            });
        }
        if entry.stand_in_for.is_some() {
            push_text(
                shapes,
                [text_left, LEVEL_TOP + LEVEL_HEIGHT + 2.0],
                "(stand-in)",
                DIM_RGB,
            );
        }
    }
    for (row, team) in [Team::Blue, Team::Red].into_iter().enumerate() {
        let level: usize = state.get_wavefront(team);
        let left: f32 = MARGIN + level as f32 * (width + LEVEL_GAP);
        let is_attacking: bool = state.get_attacker() == Some(team);
        let label: String = format!(
            "^ {} wave{}",
            team.get_name(),
            if is_attacking { " *" } else { "" }
        );
        push_text(
            shapes,
            [
                left + 5.0,
                LEVEL_TOP + LEVEL_HEIGHT + 14.0 + row as f32 * LINE,
            ],
            label,
            TEAM_RGB[team.get_index()],
        );
    }
}

/// `push_history` adds the round history: one row a round, the latest `SHOWN_ROUND_COUNT`, with
/// a chip in the winner's colour and what the round did.
///
/// Example: `push_history(&mut shapes, &view);`
fn push_history(shapes: &mut Vec<Shape>, view: &MatchView) {
    let rounds: &[Round] = view.state.rounds();
    let hidden_count: usize = rounds.len().saturating_sub(SHOWN_ROUND_COUNT);
    let heading: String = match hidden_count {
        0 => format!("HISTORY  {} rounds", rounds.len()),
        _ => format!("HISTORY  {} rounds, last {SHOWN_ROUND_COUNT}", rounds.len()),
    };
    push_text(shapes, [MARGIN, HISTORY_TOP], heading, HUD_RGB);
    for (row, round) in rounds[hidden_count..].iter().enumerate() {
        let y: f32 = HISTORY_TOP + LINE + 4.0 + row as f32 * LINE;
        push_text(
            shapes,
            [MARGIN, y],
            format!("#{:<2} {:<14}", round.number, view.levels[round.level].name),
            HUD_RGB,
        );
        let (attack, attack_rgb): (String, [u8; 3]) = match round.attacker {
            None => ("no attacker".to_string(), PURPLE_RGB),
            Some(team) => (
                format!("{} attacks", team.get_name()),
                TEAM_RGB[team.get_index()],
            ),
        };
        push_text(shapes, [MARGIN + 20.0 * GLYPH, y], attack, attack_rgb);
        let chip_left: f32 = MARGIN + 33.0 * GLYPH;
        shapes.push(Shape::Panel {
            min: [chip_left, y - 2.0],
            size: [CHIP_WIDTH, LINE - 1.0],
            rgba: get_dark_rgba(TEAM_RGB[round.winner.get_index()]),
            border_rgb: Some(TEAM_RGB[round.winner.get_index()]),
        });
        push_text(
            shapes,
            [chip_left + 6.0, y],
            format!("{} won", round.winner.get_name()),
            TEAM_RGB[round.winner.get_index()],
        );
        let effect: &str = describe_round_effect(view, round);
        push_text(shapes, [chip_left + CHIP_WIDTH + 8.0, y], effect, DIM_RGB);
    }
}

/// `get_progression_shapes` gives the window's progression page for `view`, for a player of
/// `team`: a header with the phase, the five levels in a row with their defenders, captures and
/// both wavefronts, who attacks and defends, the history and the result.
///
/// Example: `let shapes = get_progression_shapes(&view, Some(Team::Blue));`
pub fn get_progression_shapes(view: &MatchView, team: Option<Team>) -> Vec<Shape> {
    get_progression_shapes_with_footer(view, team, LEVEL_WINDOW_FOOTER)
}

/// `get_progression_shapes_with_footer` gives the progression page as `get_progression_shapes`
/// does, with `footer` as its last line (what the keys do in the program showing it).
///
/// Example: `let shapes = get_progression_shapes_with_footer(&view, None, "Escape: main menu");`
pub fn get_progression_shapes_with_footer(
    view: &MatchView,
    team: Option<Team>,
    footer: &str,
) -> Vec<Shape> {
    let state: &MatchState = &view.state;
    let mut shapes: Vec<Shape> = vec![Shape::Panel {
        min: [0.0, 0.0],
        size: PAGE_SIZE,
        rgba: BACKDROP_RGBA,
        border_rgb: None,
    }];
    push_text(&mut shapes, [MARGIN, 12.0], "WARPED MATCH", HUD_RGB);
    let (you, you_rgb): (String, [u8; 3]) = match team {
        Some(team) => (
            format!("you play {}", team.get_name()),
            TEAM_RGB[team.get_index()],
        ),
        None => ("press 1 for blue, 2 for red".to_string(), HUD_RGB),
    };
    push_text(
        &mut shapes,
        [PAGE_SIZE[0] - MARGIN - you.len() as f32 * GLYPH, 12.0],
        you.clone(),
        you_rgb,
    );
    let phase_rgb: [u8; 3] = match view.phase {
        Phase::WaitingForTeams => HUD_RGB,
        _ => HIGHLIGHT_RGB,
    };
    push_text(&mut shapes, [MARGIN, 26.0], format_phase(view), phase_rgb);
    let time: String = view
        .match_left_s
        .map_or("starts with round 1".to_string(), format_time);
    push_text(
        &mut shapes,
        [MARGIN, 38.0],
        format!(
            "Players: blue {}, red {}.  Match time left: {time}",
            view.team_counts[0], view.team_counts[1]
        ),
        DIM_RGB,
    );
    push_level_row(&mut shapes, view);
    let roles_y: f32 = LEVEL_TOP + LEVEL_HEIGHT + 14.0 + 2.0 * LINE + 8.0;
    let roles: [(&str, Option<Team>, f32); 2] = [
        (
            "Attacker (offensive ownership): ",
            state.get_attacker(),
            0.0,
        ),
        (
            "Defender (defensive ownership): ",
            state.get_defender(),
            1.0,
        ),
    ];
    for (label, role_team, row) in roles {
        let y: f32 = roles_y + row * LINE;
        push_text(&mut shapes, [MARGIN, y], label, DIM_RGB);
        push_text(
            &mut shapes,
            [MARGIN + label.len() as f32 * GLYPH, y],
            format_team(role_team),
            role_team.map_or(DIM_RGB, |team| TEAM_RGB[team.get_index()]),
        );
    }
    push_history(&mut shapes, view);
    if view.state.get_free_play().is_some() {
        let (min, size) = get_match_button_box();
        shapes.push(Shape::Panel {
            min,
            size,
            rgba: get_dark_rgba(HUD_RGB),
            border_rgb: Some(HUD_RGB),
        });
        push_text(
            &mut shapes,
            [min[0] + MATCH_BUTTON_PAD, min[1] + MATCH_BUTTON_PAD],
            MATCH_BUTTON_TEXT,
            HUD_RGB,
        );
    }
    push_text(
        &mut shapes,
        [MARGIN, PAGE_SIZE[1] - 2.0 * LINE - 4.0],
        CLICK_HINT,
        DIM_RGB,
    );
    if let Some(outcome) = state.outcome() {
        let text: String = format!("RESULT: {}", describe_outcome(outcome));
        let rgb: [u8; 3] = match outcome {
            crate::match_state::Outcome::Won(team) => TEAM_RGB[team.get_index()],
            crate::match_state::Outcome::Tie => HUD_RGB,
        };
        let y: f32 = PAGE_SIZE[1] - 3.0 * LINE - 4.0;
        shapes.push(Shape::Panel {
            min: [MARGIN, y - 4.0],
            size: [text.len() as f32 * GLYPH + 12.0, LINE + 4.0],
            rgba: get_dark_rgba(rgb),
            border_rgb: Some(rgb),
        });
        push_text(&mut shapes, [MARGIN + 6.0, y], text, rgb);
    }
    push_text(
        &mut shapes,
        [MARGIN, PAGE_SIZE[1] - LINE - 4.0],
        footer,
        DIM_RGB,
    );
    shapes
}

#[cfg(test)]
mod tests {
    //! Unit tests for `match_menu.rs`.
    //!
    //! | Behaviour | Cases | Test |
    //! | --- | --- | --- |
    //! | Start | waiting, no rounds | `the_page_before_the_first_round` |
    //! | Worked example | every row's ownership and history after it, the result | `the_page_after_the_design_docs_example` |
    //! | Round effect | purple round, attacker advances, defender holds, the round that wins the match | `the_page_after_the_design_docs_example` |
    //! | Long matches | 40 rounds fit `MAX_LINE_COUNT` | `a_long_match_fits_the_page` |
    //! | Window page | five panels, a picture box inside each, next level outlined, wavefront markers, chips, result | `the_window_page_shows_levels_wavefronts_and_history` |
    //! | Window page bounds | every shape inside `PAGE_SIZE` after a long match | `the_window_page_stays_on_the_page` |
    //! | Clicks | each level's centre and corner, the gap between levels, elsewhere, the match button without and with free play (drawn then) | `clicks_choose_every_level_or_the_match` |
    //! | MATCH page | a long match cut to the in-level menu's lines and width | `the_match_page_fits_the_in_level_menu` |
    //! | Free play | each phase on a free level, after match rounds (no match round indexed), the level outlined | `free_play_shows_its_level_and_wins` |

    use super::*;
    use crate::match_protocol::LevelEntry;

    /// `make_view` gives a view of `state` on five levels named as in DESIGN.md.
    ///
    /// Example: `let view = make_view(MatchState::New(5), Phase::Over);`
    fn make_view(state: MatchState, phase: Phase) -> MatchView {
        let names: [&str; 5] = [
            "Dust",
            "Snow Planet",
            "City Sphere",
            "Dark Forest",
            "Lunar Surface",
        ];
        MatchView {
            state,
            phase,
            levels: names
                .iter()
                .map(|name| LevelEntry {
                    name: name.to_string(),
                    folder: String::new(),
                    branch: String::new(),
                    status: String::new(),
                    program: "self".to_string(),
                    map: None,
                    server_args: Vec::new(),
                    client_args: Vec::new(),
                    stand_in_for: None,
                    takes_bot_label: true,
                    server_settings: None,
                    map_picture: None,
                })
                .collect(),
            team_counts: [1, 1],
            match_left_s: Some(600.0),
        }
    }

    /// `get_text` gives the page's lines joined by newlines.
    ///
    /// Example: `let text = get_text(&view);`
    fn get_text(view: &MatchView) -> String {
        get_progression_lines(view, Some(Team::Blue))
            .into_iter()
            .map(|(line, _)| line)
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[test]
    fn clicks_choose_every_level_or_the_match() {
        let mut state: MatchState = MatchState::New(5);
        let view: MatchView = make_view(state.clone(), Phase::WaitingForTeams);
        for level in 0..5 {
            let (min, size) = get_level_box(5, level);
            let centre: [f32; 2] = [min[0] + size[0] / 2.0, min[1] + size[1] / 2.0];
            assert_eq!(
                get_click_target(&view, centre),
                Some(PageClick::Level(level))
            );
            assert_eq!(get_click_target(&view, min), Some(PageClick::Level(level)));
        }
        let (second_min, second_size) = get_level_box(5, 1);
        let in_gap: [f32; 2] = [second_min[0] - 1.0, second_min[1] + second_size[1] / 2.0];
        assert_eq!(get_click_target(&view, in_gap), None);
        assert_eq!(get_click_target(&view, [1.0, 1.0]), None);
        let (button_min, _) = get_match_button_box();
        let on_button: [f32; 2] = [button_min[0] + 2.0, button_min[1] + 2.0];
        assert_eq!(get_click_target(&view, on_button), None);
        state.start_free_play(3);
        let freely: MatchView = make_view(state, Phase::WaitingForTeams);
        assert_eq!(get_click_target(&freely, on_button), Some(PageClick::Match));
        let shapes: Vec<Shape> = get_progression_shapes(&freely, None);
        assert!(
            shapes.iter().any(
                |shape| matches!(shape, Shape::Text { text, .. } if text == MATCH_BUTTON_TEXT)
            )
        );
    }

    #[test]
    fn the_match_page_fits_the_in_level_menu() {
        let mut state: MatchState = MatchState::New(5);
        for _ in 0..12 {
            state.record_round(Team::Red);
            if state.outcome().is_some() {
                break;
            }
            state.record_round(Team::Blue);
        }
        let lines: Vec<(String, bool)> = get_match_lines(&make_view(state, Phase::Over), None);
        assert_eq!(lines.len(), MATCH_PAGE_MAX_LINE_COUNT);
        assert!(
            lines
                .iter()
                .all(|(line, _)| line.chars().count() <= MATCH_PAGE_MAX_CHAR_COUNT)
        );
    }

    #[test]
    fn free_play_shows_its_level_and_wins() {
        let mut state: MatchState = MatchState::New(5);
        state.record_round(Team::Blue);
        state.start_free_play(0);
        state.record_free_round(Team::Red);
        state.record_free_round(Team::Red);
        for phase in [
            Phase::WaitingForTeams,
            Phase::Intermission { left_s: 3.0 },
            Phase::Playing {
                round: 3,
                level: 0,
                port: 1,
            },
            Phase::RoundOver {
                round: 2,
                left_s: 1.0,
            },
            Phase::Over,
        ] {
            let line: String = format_phase(&make_view(state.clone(), phase));
            assert!(line.starts_with("Free play on "), "{line}");
            assert!(line.ends_with("Wins: blue 0, red 2"), "{line}");
        }
        assert_eq!(get_next_level(&state), Some(0));
    }

    #[test]
    fn the_page_before_the_first_round() {
        let text: String = get_text(&make_view(MatchState::New(5), Phase::WaitingForTeams));
        assert!(text.contains("first: City Sphere (no attacker)"), "{text}");
        assert!(
            text.contains("blue wavefront: Dark Forest (depth 1 on red's side)"),
            "{text}"
        );
        assert!(
            text.contains("red wavefront: Snow Planet (depth 1 on blue's side)"),
            "{text}"
        );
        assert!(text.contains("History (0 rounds):"), "{text}");
        assert!(text.contains("(*City S)"), "{text}");
    }

    #[test]
    fn the_page_after_the_design_docs_example() {
        let mut state: MatchState = MatchState::New(5);
        for winner in [Team::Blue, Team::Blue, Team::Red, Team::Blue, Team::Blue] {
            state.record_round(winner);
        }
        assert_eq!(state.outcome(), Some(Outcome::Won(Team::Blue)));
        let view: MatchView = make_view(state, Phase::Over);
        let text: String = get_text(&view);
        for expected in [
            " Dust            blue 2   blue     not reached",
            " Snow Planet     blue 1   blue     red wavefront",
            " City Sphere     purple   -        won by blue (round 1, played once)",
            " Dark Forest     red 1    red      captured by blue (round 2)",
            " Lunar Surface   red 2    red      captured by blue (round 5)",
            " #3  Lunar Surface  blue attacks won by red  (defender holds, roles swap)",
            " #4  Snow Planet    red attacks  won by blue (defender holds, roles swap)",
            "Result: blue wins the match",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        let effect_of = |number: &str| -> &str {
            let line: &str = text
                .lines()
                .find(|line| line.starts_with(number))
                .expect("a history line");
            line.rsplit('(').next().expect("an effect")
        };
        assert_eq!(effect_of(" #1 "), "becomes the attacker)", "{text}");
        assert_eq!(effect_of(" #2 "), "attacker advances)", "{text}");
        assert_eq!(effect_of(" #5 "), "attacker wins the match)", "{text}");
        let highlighted: Vec<String> = get_progression_lines(&view, None)
            .into_iter()
            .filter(|(_, is_highlighted)| *is_highlighted)
            .map(|(line, _)| line)
            .collect();
        assert!(highlighted.contains(&"Result: blue wins the match".to_string()));
    }

    #[test]
    fn a_long_match_fits_the_page() {
        let mut state: MatchState = MatchState::New(5);
        state.record_round(Team::Blue);
        for i in 0..39 {
            state.record_round(if i % 2 == 0 { Team::Red } else { Team::Blue });
        }
        let phase: Phase = Phase::Playing {
            round: 41,
            level: state.get_next_level().unwrap(),
            port: 1,
        };
        let lines: Vec<(String, bool)> =
            get_progression_lines(&make_view(state, phase), Some(Team::Red));
        assert!(lines.len() <= MAX_LINE_COUNT, "{} lines", lines.len());
        assert!(
            lines.iter().all(|(line, _)| line.chars().count() <= 90),
            "{lines:?}"
        );
    }

    /// `get_panels` gives the page's panels of `size` (the level panels).
    ///
    /// Example: `let levels = get_panels(&shapes, size);`
    fn get_panels(shapes: &[Shape], width: f32) -> Vec<&Shape> {
        shapes
            .iter()
            .filter(|shape| matches!(shape, Shape::Panel { size, .. } if size[0] == width && size[1] == LEVEL_HEIGHT))
            .collect()
    }

    #[test]
    fn the_window_page_shows_levels_wavefronts_and_history() {
        let mut state: MatchState = MatchState::New(5);
        state.record_round(Team::Red);
        state.record_round(Team::Red);
        let view: MatchView = make_view(state, Phase::Over);
        let shapes: Vec<Shape> = get_progression_shapes(&view, Some(Team::Blue));
        let width: f32 = (PAGE_SIZE[0] - 2.0 * MARGIN - 4.0 * LEVEL_GAP) / 5.0;
        let levels: Vec<&Shape> = get_panels(&shapes, width);
        assert_eq!(levels.len(), 5);
        let outlined: Vec<usize> = (0..5)
            .filter(|&i| {
                matches!(
                    levels[i],
                    Shape::Panel {
                        border_rgb: Some(_),
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(outlined, vec![0], "Dust, red's next level, is outlined");
        let thumbnails: Vec<(usize, [f32; 2], [f32; 2])> = shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Thumbnail { level, min, size } => Some((*level, *min, *size)),
                _ => None,
            })
            .collect();
        assert_eq!(
            thumbnails
                .iter()
                .map(|(level, ..)| *level)
                .collect::<Vec<usize>>(),
            vec![0, 1, 2, 3, 4]
        );
        for ((_, min, size), panel) in thumbnails.iter().zip(&levels) {
            let Shape::Panel {
                min: panel_min,
                size: panel_size,
                ..
            } = panel
            else {
                unreachable!()
            };
            assert!(
                (0..2)
                    .all(|a| min[a] > panel_min[a]
                        && min[a] + size[a] < panel_min[a] + panel_size[a])
            );
        }
        let texts: Vec<(&str, [u8; 3])> = shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Text { text, rgb, .. } => Some((text.as_str(), *rgb)),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&("^ red wave *", TEAM_RGB[1])));
        assert!(texts.contains(&("^ blue wave", TEAM_RGB[0])));
        assert!(texts.contains(&("red took #2", TEAM_RGB[1])));
        assert_eq!(
            texts.iter().filter(|(text, _)| *text == "red won").count(),
            2
        );
        assert!(
            texts.iter().any(|(text, _)| text.starts_with("RESULT:"))
                == view.state.outcome().is_some()
        );
    }

    #[test]
    fn the_window_page_stays_on_the_page() {
        let mut state: MatchState = MatchState::New(5);
        for index in 0..40 {
            if state.get_next_level().is_none() {
                break;
            }
            let attacker: Option<Team> = state.get_attacker();
            state.record_round(match (index, attacker) {
                (0, _) => Team::Blue,
                (_, Some(team)) => team.get_other(),
                (_, None) => Team::Red,
            });
        }
        state.end_in_tie();
        let shapes: Vec<Shape> = get_progression_shapes(&make_view(state, Phase::Over), None);
        for shape in &shapes {
            let (min, max): ([f32; 2], [f32; 2]) = match shape {
                Shape::Panel { min, size, .. } | Shape::Thumbnail { min, size, .. } => {
                    (*min, [min[0] + size[0], min[1] + size[1]])
                }
                Shape::Text { at, text, .. } => {
                    (*at, [at[0] + text.len() as f32 * GLYPH, at[1] + GLYPH])
                }
            };
            assert!(
                min[0] >= 0.0 && min[1] >= 0.0 && max[0] <= PAGE_SIZE[0] && max[1] <= PAGE_SIZE[1],
                "{shape:?} leaves the page"
            );
        }
    }
}
