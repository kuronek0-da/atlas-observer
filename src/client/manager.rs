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

    use crate::{
        game::{
            character::{GameChar, Moon},
            state::{GameTimers, Player, Players},
        },
        validation::result::MatchTimers,
    };

    use super::*;

    fn mock_match_result(session_id: String) -> MatchResult {
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
            1u8,
            Players { p1, p2 },
            GameTimers::new(12000, 20, 5000),
            session_id,
        )
        .unwrap()
    }

    #[test]
    fn test_send_result() {
        let client = ClientManager::new_test(ClientState::JoinedRanked("ABCDEFG".to_string()))
            .expect("Failed to load config.");
        let result1 = mock_match_result("ABCDEFG".to_string());
        let result2 = mock_match_result("ABCDEFG".to_string());

        println!("Sending req to: {}", client.result_path());
        let req1 = client.send_result(&result1);
        match req1 {
            Ok(res) => assert!(res.status().is_success()),
            Err(e) => panic!("{}", e),
        }
        let req2 = client.send_result(&result2);
        match req2 {
            Ok(res) => {
                assert!(res.status().is_success());
                println!("{}", res.text().unwrap());
            }
            Err(e) => panic!("{}", e),
        }
    }
}
