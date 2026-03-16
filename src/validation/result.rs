use crate::{
    game::state::{GameTimers, Players},
};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("could not create a valid result: '{0}'")]
    MatchResultError(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Winner {
    Player1,
    Player2,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchResult {
    sender_position: u8,
    players: Players,
    session_id: String,
    // Should be the same for both players unless desyncs occur
    timers: MatchTimers,
    timestamp: u64, // unix timestamp
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchTimers {
    pub round_timer: u32,
    pub real_timer: u32,
}

impl MatchResult {
    pub fn new(
        sender_position: u8,
        players: Players,
        timers: GameTimers,
        session_id: String,
    ) -> Result<Self, StateError> {
        let p1 = &players.p1;
        let p2 = &players.p2;

        let round_timer = timers.round_timer();
        let real_timer = timers.real_timer(); // Can be as long as the match lasts

        if real_timer <= 240 {
            // 1s = 24 in the counter
            return Err(StateError::MatchResultError(
                "match must be at least 10 seconds long in real time".to_string(),
            ));
        }

        let timers: MatchTimers = MatchTimers {
            round_timer,
            real_timer,
        };

        if sender_position != 1 && sender_position != 2 {
            return Err(StateError::MatchResultError(format!(
                "invalid player position: {}",
                sender_position
            )));
        }

        // Max score = 3
        if p1.score + p2.score != 3 {
            return Err(StateError::MatchResultError(
                "invalid match result".to_string(),
            ));
        }
        return Ok(MatchResult {
            sender_position,
            players,
            session_id,
            timers,
            timestamp: get_unix_timestamp_u64(),
        });
    }
}

fn get_unix_timestamp_u64() -> u64 {
    let now = SystemTime::now();
    let duration_since_epoch = now
        .duration_since(std::time::UNIX_EPOCH)
        .expect("SystemTime set before UNIX EPOCH"); // Handle potential error if time is before 1970

    // duration_since returns a Duration, which can be converted to seconds as u64
    duration_since_epoch.as_secs()
}

