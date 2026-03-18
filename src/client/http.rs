use std::sync::{Arc, Mutex};

use crate::{
    client::state::ClientState,
    config::{Config, ConfigError},
    validation::result::MatchResult,
};
use reqwest::{
    blocking::{Client, Response},
    header::{HeaderMap, HeaderValue},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("request error: '{0}'")]
    RequestError(String),
    #[error("something went wrong, server status response: '{0}'")]
    ServerError(u16),
}

/// Handles the requests to the server
pub struct ClientManager {
    player_id: u32,
    server_url: String,
    state: Arc<Mutex<ClientState>>,
    client: Client,
}

impl ClientManager {
    pub fn new(state: ClientState) -> Result<Self, ConfigError> {
        Self::from_config(state, Config::load()?)
    }

    pub fn new_test(state: ClientState) -> Result<Self, ConfigError> {
        Self::from_config(state, Config::load_test()?)
    }

    fn from_config(state: ClientState, config: Config) -> Result<Self, ConfigError> {
        Ok(ClientManager {
            player_id: config.player_id,
            server_url: config.server_url,
            state: Arc::new(Mutex::new(state)),
            client: Client::new(),
        })
    }

    pub fn clone_state(&self) -> Arc<Mutex<ClientState>> {
        Arc::clone(&self.state)
    }

    fn construct_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        // headers.insert("ngrok-skip-browser-warning", HeaderValue::from_static("1"));
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers
    }

    pub fn send_result(&self, result: &MatchResult) -> Result<Response, ClientError> {
        let res = self
            .client
            .post(self.result_path())
            .headers(self.construct_headers())
            .json(&result)
            .send()
            .map_err(|e| ClientError::RequestError(e.to_string()))?;

        if res.status().is_success() {
            Ok(res)
        } else {
            Err(ClientError::ServerError(res.status().as_u16()))
        }
    }

    pub fn result_path(&self) -> String {
        // TODO: get player from token instead of param
        format!("{}/api/match?playerId={}", self.server_url, self.player_id)
    }
}

#[cfg(test)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use crate::game::{
        game_char::{GameChar, Moon},
        state::{GameTimers, Player},
    };

    use super::*;

    fn mock_match_result(session_id: String, sender_pos: u8) -> MatchResult {
        let p1 = Player {
            char: GameChar::Akiha,
            moon: Moon::Half,
            score: 2,
        };
        let p2 = Player {
            char: GameChar::Seifuku,
            moon: Moon::Crescent,
            score: 1,
        };
        MatchResult::new(
            sender_pos,
            [p1, p2],
            GameTimers::new(0, 0, 4000),
            session_id,
        )
        .expect("Could not mock MatchResult")
    }

    #[test]
    fn test_send_result() {
        let client1 =
            ClientManager::new_test(ClientState::hosting()).expect("Failed to load config.");
        let session_id = client1
            .state
            .lock()
            .expect("Could not get client session id")
            .session()
            .expect("Session id not set for client")
            .to_string();

        let client2 = ClientManager::new_test(ClientState::JoinedRanked(session_id.clone()))
            .expect("Failed to load config.");

        let result1 = mock_match_result(session_id.clone(), 1);
        let result2 = mock_match_result(session_id, 2);

        println!("Sending req to: {}", client1.result_path());
        std::thread::spawn(move || {
            let req1 = client1.send_result(&result1);
            match req1 {
                Ok(res) => assert!(res.status().is_success()),
                Err(e) => panic!("{}", e),
            }
        });

        sleep(Duration::from_millis(100));

        let req2 = client2.send_result(&result2);
        match req2 {
            Ok(res) => {
                assert!(res.status().is_success());
                println!("{}", res.text().unwrap());
            }
            Err(e) => panic!("{}", e),
        }
    }
}
