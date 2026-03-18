use crate::game::state::{GameTimers, Player};
use serde::{Deserialize, Serialize};
use std::{fmt, time::SystemTime};
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
    p1: Player,
    p2: Player,
    session_id: String,
    // Should be the same for both players unless desyncs occur
    real_timer: u32,
    timestamp: u64, // unix timestamp
}

impl fmt::Display for MatchResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let p1 = &self.p2;
        let p2 = &self.p2;
        write!(
            f,
            "{} | {:?}-{:?} ({}x{}) {:?}-{:?} | Duration: {} | Code/Session: {}",
            if self.winner() == self.sender_position {
                "Won"
            } else {
                "Lost"
            },
            p1.char,
            p2.moon,
            p1.score,
            p2.score,
            p2.char,
            p2.moon,
            self.fmt_real_timer(),
            self.session_id
        )
    }
}

impl MatchResult {
    pub fn new(
        sender_position: u8,
        players: [Player; 2],
        timers: GameTimers,
        session_id: String,
    ) -> Result<Self, StateError> {
        let [p1, p2] = players;

        let real_timer = timers.real_timer(); // Can be as long as the match lasts

        if real_timer <= 240 {
            // 1s = 24 in the counter
            return Err(StateError::MatchResultError(
                "match must be at least 10 seconds long in real time".to_string(),
            ));
        }

        if sender_position != 1 && sender_position != 2 {
            return Err(StateError::MatchResultError(format!(
                "invalid player position: {}",
                sender_position
            )));
        }

        if p1.score == p2.score {
            return Err(StateError::MatchResultError(format!(
                "invalid score: p1: {}, p2: {}",
                p1.score, p2.score
            )));
        }
        return Ok(MatchResult {
            sender_position,
            p1,
            p2,
            session_id,
            real_timer,
            timestamp: get_unix_timestamp_u64(),
        });
    }

    fn fmt_real_timer(&self) -> String {
        let secs = (self.real_timer as f32 / 24.0).round();
        let mins = (secs / 60.0).trunc();
        let remaining_secs = secs % 60.0;
        format!("{}:{:02}", mins as u32, remaining_secs as u32)
    }

    pub fn winner(&self) -> u8 {
        let p1 = &self.p1.score;
        let p2 = &self.p2.score;
        if p1 > p2 {
            return 1;
        }
        2
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
