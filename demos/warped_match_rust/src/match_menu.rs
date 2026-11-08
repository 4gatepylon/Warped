//! `match_menu.rs` is the match's progression page: the five levels in their line with who
//! defends each, whose wavefront it is and who captured it, both wavefronts, who attacks now, the
//! round history and the match's result. It keeps no window state: the window draws its lines.
//!
//! Interface: `get_progression_lines`, `MAX_LINE_COUNT`.
//!
//! Example: `let lines = get_progression_lines(&view, Some(Team::Blue));`
//!
//! Must know:
//! - A line is `(text, is_highlighted)`; the level being played and the match's result are
//!   highlighted.

use crate::match_flow::{MatchView, Phase, describe_outcome};
use crate::match_state::{MatchState, Ownership, Round, Team};

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

/// `format_round` gives one line of the round history.
///
/// Example: `let line = format_round(&view, &round);`
fn format_round(view: &MatchView, round: &Round) -> String {
    let attack: String = match round.attacker {
        None => "no attacker".to_string(),
        Some(team) => format!("{} attacks", team.get_name()),
    };
    let effect: &str = match round.attacker {
        None => "becomes the attacker",
        Some(team) if team == round.winner => "attacker advances",
        Some(_) => "defender holds, roles swap",
    };
    format!(
        " #{:<2} {:<14} {:<12} won by {:<4} ({effect})",
        round.number,
        view.levels[round.level].name,
        attack,
        round.winner.get_name()
    )
}

/// `format_phase` gives the line saying what the match is doing now.
///
/// Example: `let line = format_phase(&view);`
fn format_phase(view: &MatchView) -> String {
    let state: &MatchState = &view.state;
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

/// `get_level_markers` gives the line of the map as a strip, blue's deepest level on the left:
/// `[B]` a level blue defends, `(P)` the purple level, `*` on the level played next or now.
///
/// Example: `let strip = get_level_markers(&view);`
fn get_level_markers(view: &MatchView) -> String {
    let state: &MatchState = &view.state;
    (0..state.level_count())
        .map(|level| {
            let short: String = view.levels[level].name.chars().take(6).collect();
            let mark: &str = if state.get_next_level() == Some(level) {
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

#[cfg(test)]
mod tests {
    //! Unit tests for `match_menu.rs`.
    //!
    //! | Behaviour | Cases | Test |
    //! | --- | --- | --- |
    //! | Start | waiting, no rounds | `the_page_before_the_first_round` |
    //! | Worked example | every row's ownership and history after it, the result | `the_page_after_the_design_docs_example` |
    //! | Long matches | 40 rounds fit `MAX_LINE_COUNT` | `a_long_match_fits_the_page` |

    use super::*;
    use crate::match_flow::LevelEntry;
    use crate::match_state::Outcome;

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
}
