use std::fmt;
use thiserror::Error;
use std::time::SystemTime;
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
    winner: Winner,
    players: Players,
    session_id: String,
    // Should be the same for both players unless desyncs occur
    timers: MatchTimers,
    timestamp: u64 // unix timestamp
}

#[derive(Debug)]
pub struct MatchTimers {
    pub round_timer: u32,
    pub real_timer: u32
}

impl MatchResult {
    pub fn new(players: Players, timers: GameTimers, session_id: String) -> Result<Self, StateError> {
        let p1 = &players.p1;
        let p2 = &players.p2;
        let timers: MatchTimers = MatchTimers {
            round_timer: timers.round_timer(),
            real_timer: timers.real_timer()
        };
        let winner;
        
        // Max score = 3
        if p1.score + p2.score != 3 {
            return Err(StateError::MatchResultError("invalid match result".to_string()));
        }
        if p1.score > p2.score {
            winner = Winner::Player1;
        } else {
            winner = Winner::Player2;
        }
        return Ok(MatchResult { winner, players, session_id, timers, timestamp: get_unix_timestamp_u64() });
    }
}

fn get_unix_timestamp_u64() -> u64 {
    let now = SystemTime::now();
    let duration_since_epoch = now.duration_since(std::time::UNIX_EPOCH)
        .expect("SystemTime set before UNIX EPOCH"); // Handle potential error if time is before 1970

    // duration_since returns a Duration, which can be converted to seconds as u64
    duration_since_epoch.as_secs()
}