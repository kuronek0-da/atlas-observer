use std::fmt;

use thiserror::Error;

use crate::{game::state::{ GameTimers, Players }, memory::addresses::LocalPlayer};


#[derive(Debug, Error)]
pub enum StateError {
    #[error("could not create a valid result: '{0}'")]
    MatchResultError(String)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Winner {
    Player1,
    Player2
}

#[derive(Debug)]
pub struct MatchResult {
    local_player: LocalPlayer,
    winner: Winner,
    players: Players,
    // Should be the same for both players unless desyncs occur
    timers: GameTimers
}

impl MatchResult {
    pub fn new(local_player: LocalPlayer, players: Players, timers: GameTimers) -> Result<Self, StateError> {
        let p1 = &players.p1;
        let p2 = &players.p2;
        
        // Max score = 3
        if p1.score >= 3 || p2.score >= 3 {
            return Err(StateError::MatchResultError("max score is 3.".to_string()));
        }
        if p1.score > p2.score {
            return Ok(MatchResult { local_player, winner: Winner::Player1, players, timers });
        }
        return Ok(MatchResult { local_player, winner: Winner::Player2, players, timers });
    }
}

impl fmt::Display for MatchResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let p1 = &self.players.p1;
        let p2 = &self.players.p2;
        let won = match self.local_player {
            LocalPlayer::P1 => self.winner == Winner::Player1,
            _ => self.winner == Winner::Player2,
        };
        write!(f, "Result: {} | {:?}-{:?} ({}x{}) {:?}-{:?}",
            if won { "WIN" } else { "LOSE" },
            p1.moon, p1.char, p1.score,
            p2.score, p2.moon, p2.char
        )
    }
}