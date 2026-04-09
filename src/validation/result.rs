use crate::{
    game::state::{GameTimers, Player},
    memory::addresses::ClientMode,
};
use serde::{Deserialize, Serialize};
use std::{fmt, time::SystemTime};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("could not create a valid result: '{0}'")]
    MatchResultError(String),
    #[error("match code/session not found.")]
    SessionNotFound,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SenderRole {
    Host,
    Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchResult {
    host_position: Option<u8>,
    sender_role: SenderRole,
    p1: Player,
    p2: Player,
    session_id: String,
    // Should be the same for both players unless desyncs occur
    real_timer: u32,
    timestamp: u64, // unix timestamp
}

impl fmt::Display for MatchResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let p1 = &self.p1;
        let p2 = &self.p2;
        write!(
            f,
            "{:?}-{:?} ({}x{}) {:?}-{:?} | Duration: {}",
            p1.moon,
            p1.character,
            p1.score,
            p2.score,
            p2.moon,
            p2.character,
            self.fmt_real_timer(),
        )
    }
}

impl MatchResult {
    pub fn new(
        session_id: String,
        client_mode: ClientMode,
        local_player: u8,
        players: [Player; 2],
        timers: GameTimers,
    ) -> Result<Self, StateError> {
        let [p1, p2] = players;

        let real_timer = timers.real_timer(); // Can be as long as the match lasts

        let mut sender_role = SenderRole::Host;
        let host_position = match client_mode {
            ClientMode::Host => Some(local_player),
            ClientMode::Client => {
                sender_role = SenderRole::Client;
                None
            }
            _ => Err(StateError::MatchResultError(
                "role state mut be host or client".to_string(),
            ))?,
        };

        if real_timer <= 240 {
            // 1s = 24 in the counter
            return Err(StateError::MatchResultError(
                "match must be at least 10 seconds long in real time".to_string(),
            ));
        }

        if p1.score == p2.score {
            return Err(StateError::MatchResultError(format!(
                "invalid score: p1: {}, p2: {}",
                p1.score, p2.score
            )));
        }
        return Ok(MatchResult {
            host_position,
            sender_role,
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

    pub fn session_id(&self) -> &String {
        &self.session_id
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
