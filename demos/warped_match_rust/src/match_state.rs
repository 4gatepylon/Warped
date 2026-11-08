//! `match_state.rs` keeps one match's state by the rules of `game_design/DESIGN.md` (Match,
//! Wavefront): the map's line of levels, every round played and who won it, each team's
//! wavefront, which team attacks now, and the match's outcome. It only applies the rules; the
//! match server (`bin/warped_match_server.rs`) feeds it each round's winner.
//!
//! Interface: `Team`, `Ownership`, `Round`, `Outcome`, `MatchState` (`New`, `get_next_level`,
//! `get_attacker`, `get_defender`, `get_wavefront`, `get_ownership`, `record_round`,
//! `end_in_tie`, `rounds`, `outcome`, `level_count`).
//!
//! Example: `let mut state = MatchState::New(5); state.record_round(Team::Blue);`
//!
//! Must know:
//! - Levels are indices on the map's line: 0 is blue's deepest level, `level_count / 2` the purple
//!   level, and `level_count - 1` red's deepest. The default map is Dust, Snow Planet, City
//!   Sphere, Dark Forest, Lunar Surface.
//! - A wavefront is stored as a depth on the other team's side (1 = its shallowest level).

use serde::{Deserialize, Serialize};

/// `Team` is one of the two teams.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Team {
    Blue,
    Red,
}

impl Team {
    /// `get_other` gives the opposing team.
    ///
    /// Example: `assert_eq!(Team::Blue.get_other(), Team::Red);`
    pub fn get_other(self) -> Team {
        match self {
            Team::Blue => Team::Red,
            Team::Red => Team::Blue,
        }
    }

    /// `get_index` gives the team's index into `net::TEAM_NAMES` (blue 0, red 1).
    ///
    /// Example: `let index = Team::Red.get_index();`
    pub fn get_index(self) -> usize {
        match self {
            Team::Blue => 0,
            Team::Red => 1,
        }
    }

    /// `get_name` gives the team's name in lower case, as levels report it.
    ///
    /// Example: `assert_eq!(Team::Blue.get_name(), "blue");`
    pub fn get_name(self) -> &'static str {
        match self {
            Team::Blue => "blue",
            Team::Red => "red",
        }
    }
}

/// `Ownership` is who a level belongs to now: the team that defends it (`None` for the purple
/// level), the team whose wavefront it is (it attacks there next), and the team that has
/// captured it (won it and moved its wavefront past it).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Ownership {
    pub defender: Option<Team>,
    pub wavefront_of: Option<Team>,
    pub captured_by: Option<Team>,
}

/// `Round` is one played round: its number (from 1), its level, the attacking team (`None` on the
/// purple level) and the winner.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Round {
    pub number: usize,
    pub level: usize,
    pub attacker: Option<Team>,
    pub winner: Team,
}

/// `Outcome` is how a finished match ended: a team won, or time ran out (a tie).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Outcome {
    Won(Team),
    Tie,
}

/// `MatchState` is one match: the number of levels on the map, each team's wavefront depth, the
/// attacker, every round played and the outcome once there is one.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct MatchState {
    level_count: usize,
    wavefront_depths: [usize; 2],
    attacker: Option<Team>,
    rounds: Vec<Round>,
    outcome: Option<Outcome>,
}

impl MatchState {
    /// `New` starts a match on a map of `level_count` levels (odd, at least 3), before its first
    /// round on the purple level.
    ///
    /// Example: `let state = MatchState::New(5);`
    #[allow(non_snake_case)]
    pub fn New(level_count: usize) -> MatchState {
        assert!(
            level_count >= 3 && level_count % 2 == 1,
            "MatchState::New: a map has an odd number of levels, at least 3, got {level_count}"
        );
        MatchState {
            level_count,
            wavefront_depths: [1, 1],
            attacker: None,
            rounds: Vec::new(),
            outcome: None,
        }
    }

    /// `level_count` gives the number of levels on the map.
    ///
    /// Example: `let count = state.level_count();`
    pub fn level_count(&self) -> usize {
        self.level_count
    }

    /// `rounds` gives every round played, in order.
    ///
    /// Example: `let played = state.rounds().len();`
    pub fn rounds(&self) -> &[Round] {
        &self.rounds
    }

    /// `outcome` gives how the match ended, or `None` while it goes on.
    ///
    /// Example: `if state.outcome().is_some() { ... }`
    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    /// `get_purple_level` gives the purple level's index, the middle of the line.
    ///
    /// Example: `assert_eq!(MatchState::New(5).get_purple_level(), 2);`
    pub fn get_purple_level(&self) -> usize {
        self.level_count / 2
    }

    /// `get_max_depth` gives the depth of each team's deepest level.
    ///
    /// Example: `assert_eq!(MatchState::New(5).get_max_depth(), 2);`
    pub fn get_max_depth(&self) -> usize {
        self.level_count / 2
    }

    /// `get_level_at` gives the index of the level at `depth` on `defender`'s side.
    ///
    /// Example: `assert_eq!(MatchState::New(5).get_level_at(Team::Red, 1), 3);`
    pub fn get_level_at(&self, defender: Team, depth: usize) -> usize {
        assert!(
            (1..=self.get_max_depth()).contains(&depth),
            "get_level_at: depth {depth} is not on a team's side"
        );
        match defender {
            Team::Blue => self.get_purple_level() - depth,
            Team::Red => self.get_purple_level() + depth,
        }
    }

    /// `get_level_defender` gives the team a level belongs to, or `None` for the purple level.
    ///
    /// Example: `assert_eq!(MatchState::New(5).get_level_defender(0), Some(Team::Blue));`
    pub fn get_level_defender(&self, level: usize) -> Option<Team> {
        assert!(
            level < self.level_count,
            "get_level_defender: no level {level}"
        );
        let purple: usize = self.get_purple_level();
        match level.cmp(&purple) {
            std::cmp::Ordering::Less => Some(Team::Blue),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(Team::Red),
        }
    }

    /// `get_level_depth` gives a level's depth: 0 for purple, then 1, 2, … outward.
    ///
    /// Example: `assert_eq!(MatchState::New(5).get_level_depth(4), 2);`
    pub fn get_level_depth(&self, level: usize) -> usize {
        level.abs_diff(self.get_purple_level())
    }

    /// `get_wavefront` gives the level that `team` attacks next when it is the attacker.
    ///
    /// Example: `assert_eq!(MatchState::New(5).get_wavefront(Team::Blue), 3);`
    pub fn get_wavefront(&self, team: Team) -> usize {
        self.get_level_at(team.get_other(), self.wavefront_depths[team.get_index()])
    }

    /// `get_attacker` gives the team whose wavefront is played next, or `None` before the purple
    /// level's round.
    ///
    /// Example: `let attacker = state.get_attacker();`
    pub fn get_attacker(&self) -> Option<Team> {
        self.attacker
    }

    /// `get_defender` gives the team that owns the next level, or `None` before the purple level's
    /// round.
    ///
    /// Example: `let defender = state.get_defender();`
    pub fn get_defender(&self) -> Option<Team> {
        self.attacker.map(Team::get_other)
    }

    /// `get_next_level` gives the level of the next round: the purple level first, then the
    /// attacker's wavefront, or `None` once the match is over.
    ///
    /// Example: `if let Some(level) = state.get_next_level() { ... }`
    pub fn get_next_level(&self) -> Option<usize> {
        if self.outcome.is_some() {
            return None;
        }
        Some(match self.attacker {
            None => self.get_purple_level(),
            Some(attacker) => self.get_wavefront(attacker),
        })
    }

    /// `get_ownership` gives who defends `level`, whose wavefront it is and who has captured it.
    ///
    /// Example: `let ownership = state.get_ownership(3);`
    pub fn get_ownership(&self, level: usize) -> Ownership {
        let defender: Option<Team> = self.get_level_defender(level);
        let attacker: Option<Team> = defender.map(Team::get_other);
        let depth: usize = self.get_level_depth(level);
        let attacker_depth: Option<usize> =
            attacker.map(|team| self.wavefront_depths[team.get_index()]);
        let is_won_by_attacker: bool = self.outcome == attacker.map(Outcome::Won);
        let is_captured: bool = attacker_depth.is_some_and(|wavefront_depth| {
            depth < wavefront_depth || (depth == wavefront_depth && is_won_by_attacker)
        });
        let is_wavefront: bool = attacker_depth == Some(depth) && !is_captured;
        Ownership {
            defender,
            wavefront_of: attacker.filter(|_| is_wavefront && self.attacker.is_some()),
            captured_by: match level == self.get_purple_level() {
                true => self.rounds.first().map(|round| round.winner),
                false => attacker.filter(|_| is_captured),
            },
        }
    }

    /// `record_round` records that `winner` won the next round's level and applies the wavefront
    /// rules: the purple level's winner becomes the attacker; an attacker that wins moves its
    /// wavefront one level deeper, or wins the match on the defender's deepest level; a defender
    /// that wins becomes the attacker, at its own wavefront.
    ///
    /// Example: `state.record_round(Team::Blue);`
    pub fn record_round(&mut self, winner: Team) -> Round {
        let level: usize = self
            .get_next_level()
            .expect("record_round: the match is over, so no round can be won");
        let round: Round = Round {
            number: self.rounds.len() + 1,
            level,
            attacker: self.attacker,
            winner,
        };
        self.rounds.push(round);
        match self.attacker {
            None => self.attacker = Some(winner),
            Some(attacker) if attacker == winner => {
                let depth: &mut usize = &mut self.wavefront_depths[attacker.get_index()];
                if *depth == self.level_count / 2 {
                    self.outcome = Some(Outcome::Won(attacker));
                } else {
                    *depth += 1;
                }
            }
            Some(_) => self.attacker = Some(winner),
        }
        round
    }

    /// `end_in_tie` ends a match that no team has won, as when its time runs out.
    ///
    /// Example: `state.end_in_tie();`
    pub fn end_in_tie(&mut self) {
        assert!(
            self.outcome.is_none(),
            "end_in_tie: the match is already over"
        );
        self.outcome = Some(Outcome::Tie);
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `match_state.rs`.
    //!
    //! | Behaviour | Cases | Test |
    //! | --- | --- | --- |
    //! | Worked example of DESIGN.md | its five rounds, levels, attackers, next rounds | `the_design_docs_example_plays_out` |
    //! | Start | purple first, wavefronts at depth 1, nobody attacks | `a_match_starts_on_the_purple_level` |
    //! | Attacker sweeps | blue or red wins every round: 3 rounds | `a_sweep_takes_three_rounds` |
    //! | Defender wins | roles swap, wavefront kept where it left off | `a_defender_win_swaps_roles_and_keeps_wavefronts` |
    //! | Every sequence | all win sequences up to 14 rounds, against a reference model and invariants | `every_win_sequence_follows_the_rules` |
    //! | Ownership | defended, wavefront, captured, after a match win | `ownership_follows_the_wavefronts` |
    //! | Tie, bad use | tie, rounds after the end, bad level counts | `a_tie_ends_the_match`, `no_round_after_the_end`, `a_map_needs_an_odd_number_of_levels` |
    //! | Bigger map | 7 levels, sweep of 4 rounds | `a_seven_level_map_needs_deeper_wins` |

    use super::*;

    const DUST: usize = 0;
    const SNOW_PLANET: usize = 1;
    const CITY_SPHERE: usize = 2;
    const DARK_FOREST: usize = 3;
    const LUNAR_SURFACE: usize = 4;
    const MAX_SEQUENCE_LENGTH: usize = 14;

    #[test]
    fn the_design_docs_example_plays_out() {
        let mut state: MatchState = MatchState::New(5);
        let rows: [(usize, Option<Team>, Team, Option<usize>); 5] = [
            (CITY_SPHERE, None, Team::Blue, Some(DARK_FOREST)),
            (
                DARK_FOREST,
                Some(Team::Blue),
                Team::Blue,
                Some(LUNAR_SURFACE),
            ),
            (
                LUNAR_SURFACE,
                Some(Team::Blue),
                Team::Red,
                Some(SNOW_PLANET),
            ),
            (
                SNOW_PLANET,
                Some(Team::Red),
                Team::Blue,
                Some(LUNAR_SURFACE),
            ),
            (LUNAR_SURFACE, Some(Team::Blue), Team::Blue, None),
        ];
        for (i, (level, attacker, winner, next)) in rows.into_iter().enumerate() {
            assert_eq!(state.get_next_level(), Some(level), "round {}", i + 1);
            assert_eq!(state.get_attacker(), attacker, "round {}", i + 1);
            let round: Round = state.record_round(winner);
            assert_eq!(
                round,
                Round {
                    number: i + 1,
                    level,
                    attacker,
                    winner
                }
            );
            assert_eq!(state.get_next_level(), next, "after round {}", i + 1);
        }
        assert_eq!(state.outcome(), Some(Outcome::Won(Team::Blue)));
        assert_eq!(state.get_attacker(), Some(Team::Blue));
    }

    #[test]
    fn a_match_starts_on_the_purple_level() {
        let state: MatchState = MatchState::New(5);
        assert_eq!(state.get_next_level(), Some(CITY_SPHERE));
        assert_eq!((state.get_attacker(), state.get_defender()), (None, None));
        assert_eq!(state.get_wavefront(Team::Blue), DARK_FOREST);
        assert_eq!(state.get_wavefront(Team::Red), SNOW_PLANET);
        assert!(state.rounds().is_empty() && state.outcome().is_none());
    }

    #[test]
    fn a_sweep_takes_three_rounds() {
        for (team, levels) in [
            (Team::Blue, [CITY_SPHERE, DARK_FOREST, LUNAR_SURFACE]),
            (Team::Red, [CITY_SPHERE, SNOW_PLANET, DUST]),
        ] {
            let mut state: MatchState = MatchState::New(5);
            for level in levels {
                assert_eq!(state.record_round(team).level, level);
            }
            assert_eq!(state.outcome(), Some(Outcome::Won(team)));
            assert_eq!(state.get_next_level(), None);
        }
    }

    #[test]
    fn a_defender_win_swaps_roles_and_keeps_wavefronts() {
        let mut state: MatchState = MatchState::New(5);
        state.record_round(Team::Red);
        state.record_round(Team::Red);
        assert_eq!(state.get_next_level(), Some(DUST));
        state.record_round(Team::Blue);
        assert_eq!(state.get_attacker(), Some(Team::Blue));
        assert_eq!(state.get_next_level(), Some(DARK_FOREST));
        state.record_round(Team::Red);
        assert_eq!(state.get_attacker(), Some(Team::Red));
        assert_eq!(state.get_next_level(), Some(DUST));
        state.record_round(Team::Red);
        assert_eq!(state.outcome(), Some(Outcome::Won(Team::Red)));
    }

    /// `play_reference` plays `winners` by a direct reading of DESIGN.md's rules, kept apart from
    /// `MatchState`: the levels of each round and the match winner, if any.
    ///
    /// Example: `let (levels, winner) = play_reference(&[Team::Blue]);`
    fn play_reference(winners: &[Team]) -> (Vec<usize>, Option<Team>) {
        let names: [&str; 5] = ["dust", "snow", "city", "forest", "lunar"];
        let blue_targets: [&str; 2] = ["forest", "lunar"];
        let red_targets: [&str; 2] = ["snow", "dust"];
        let (mut blue_next, mut red_next) = (0, 0);
        let mut attacker: Option<Team> = None;
        let mut levels: Vec<usize> = Vec::new();
        for &winner in winners {
            let level: &str = match attacker {
                None => "city",
                Some(Team::Blue) => blue_targets[blue_next],
                Some(Team::Red) => red_targets[red_next],
            };
            levels.push(names.iter().position(|&name| name == level).unwrap());
            match attacker {
                Some(Team::Blue) if winner == Team::Blue => {
                    if blue_next == 1 {
                        return (levels, Some(Team::Blue));
                    }
                    blue_next += 1;
                }
                Some(Team::Red) if winner == Team::Red => {
                    if red_next == 1 {
                        return (levels, Some(Team::Red));
                    }
                    red_next += 1;
                }
                _ => attacker = Some(winner),
            }
        }
        (levels, None)
    }

    #[test]
    fn every_win_sequence_follows_the_rules() {
        let mut checked_count: usize = 0;
        for length in 1..=MAX_SEQUENCE_LENGTH {
            for bits in 0..(1u32 << length) {
                let winners: Vec<Team> = (0..length)
                    .map(|i| {
                        if bits >> i & 1 == 0 {
                            Team::Blue
                        } else {
                            Team::Red
                        }
                    })
                    .collect();
                let (expected_levels, expected_winner) = play_reference(&winners);
                if expected_levels.len() < winners.len() {
                    continue;
                }
                let mut state: MatchState = MatchState::New(5);
                let mut previous_depths: [usize; 2] = state.wavefront_depths;
                for &winner in &winners {
                    let attacker: Option<Team> = state.get_attacker();
                    let round: Round = state.record_round(winner);
                    assert_eq!(round.attacker, attacker);
                    assert_eq!(
                        state.get_level_defender(round.level),
                        attacker.map(Team::get_other)
                    );
                    for team in [Team::Blue, Team::Red] {
                        let (before, after) = (
                            previous_depths[team.get_index()],
                            state.wavefront_depths[team.get_index()],
                        );
                        let expected: usize = before
                            + usize::from(
                                attacker == Some(team)
                                    && winner == team
                                    && state.outcome().is_none(),
                            );
                        assert_eq!(
                            after, expected,
                            "{winners:?}: a wavefront only moves when its team wins as attacker"
                        );
                    }
                    previous_depths = state.wavefront_depths;
                }
                let levels: Vec<usize> = state.rounds().iter().map(|round| round.level).collect();
                assert_eq!(levels, expected_levels, "{winners:?}");
                assert_eq!(
                    state.outcome(),
                    expected_winner.map(Outcome::Won),
                    "{winners:?}"
                );
                assert_eq!(
                    levels.iter().filter(|&&level| level == CITY_SPHERE).count(),
                    1,
                    "{winners:?}"
                );
                assert_eq!(state.get_next_level().is_none(), expected_winner.is_some());
                checked_count += 1;
            }
        }
        assert!(
            checked_count > 800,
            "only {checked_count} sequences checked"
        );
    }

    #[test]
    fn ownership_follows_the_wavefronts() {
        let mut state: MatchState = MatchState::New(5);
        let defended_only = |team: Team| Ownership {
            defender: Some(team),
            wavefront_of: None,
            captured_by: None,
        };
        assert_eq!(state.get_ownership(DUST), defended_only(Team::Blue));
        assert_eq!(state.get_ownership(LUNAR_SURFACE), defended_only(Team::Red));
        assert_eq!(state.get_ownership(CITY_SPHERE).captured_by, None);
        state.record_round(Team::Blue);
        assert_eq!(
            state.get_ownership(CITY_SPHERE).captured_by,
            Some(Team::Blue)
        );
        assert_eq!(
            state.get_ownership(DARK_FOREST).wavefront_of,
            Some(Team::Blue)
        );
        assert_eq!(
            state.get_ownership(SNOW_PLANET).wavefront_of,
            Some(Team::Red)
        );
        state.record_round(Team::Blue);
        assert_eq!(
            state.get_ownership(DARK_FOREST),
            Ownership {
                defender: Some(Team::Red),
                wavefront_of: None,
                captured_by: Some(Team::Blue)
            }
        );
        assert_eq!(
            state.get_ownership(LUNAR_SURFACE).wavefront_of,
            Some(Team::Blue)
        );
        state.record_round(Team::Red);
        assert_eq!(
            state.get_ownership(DARK_FOREST).captured_by,
            Some(Team::Blue)
        );
        state.record_round(Team::Red);
        assert_eq!(
            state.get_ownership(SNOW_PLANET).captured_by,
            Some(Team::Red)
        );
        assert_eq!(state.get_ownership(DUST).wavefront_of, Some(Team::Red));
        state.record_round(Team::Red);
        assert_eq!(
            state.get_ownership(DUST),
            Ownership {
                defender: Some(Team::Blue),
                wavefront_of: None,
                captured_by: Some(Team::Red)
            }
        );
        assert_eq!(
            state.get_ownership(LUNAR_SURFACE).wavefront_of,
            Some(Team::Blue)
        );
    }

    #[test]
    fn a_tie_ends_the_match() {
        let mut state: MatchState = MatchState::New(5);
        state.record_round(Team::Red);
        state.end_in_tie();
        assert_eq!(
            (state.outcome(), state.get_next_level()),
            (Some(Outcome::Tie), None)
        );
    }

    #[test]
    #[should_panic(expected = "the match is over")]
    fn no_round_after_the_end() {
        let mut state: MatchState = MatchState::New(5);
        for _ in 0..4 {
            state.record_round(Team::Blue);
        }
    }

    #[test]
    #[should_panic(expected = "odd number of levels")]
    fn a_map_needs_an_odd_number_of_levels() {
        MatchState::New(4);
    }

    #[test]
    fn a_seven_level_map_needs_deeper_wins() {
        let mut state: MatchState = MatchState::New(7);
        let levels: Vec<usize> = (0..4)
            .map(|_| state.record_round(Team::Red).level)
            .collect();
        assert_eq!(levels, vec![3, 2, 1, 0]);
        assert_eq!(state.outcome(), Some(Outcome::Won(Team::Red)));
    }
}
