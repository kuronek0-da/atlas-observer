use std::{
    fmt::format,
    sync::{Arc, Mutex},
};

use crate::{
    client::state::ClientState,
    config::{Config, ConfigError},
    validation::result::MatchResult,
};
use reqwest::{
    blocking::{Client, Response},
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("request error: '{0}'")]
    RequestError(String),
    #[error("something went wrong, server status response: '{0}'")]
    ServerError(u16),
    #[error("token expired or is invalid")]
    AuthorizationError,
    #[error("could not read client state")]
    StateError,
}

/// Handles the requests to the server
pub struct ClientManager {
    token: String,
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
            token: config.token,
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
        let auth = format!("Bearer {}", self.token);
        // headers.insert("ngrok-skip-browser-warning", HeaderValue::from_static("1"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth).expect("Invalid token"),
        );
        headers
    }

    pub fn validate_token(&self) -> Result<(), ClientError> {
        let res = self
            .client
            .post(self.validation_path())
            .headers(self.construct_headers())
            .send()
            .map_err(|e| ClientError::RequestError(e.to_string()))?;

        if res.status().is_success() {
            return Ok(());
        }
        if res.status().as_u16() == 401 {
            return Err(ClientError::AuthorizationError);
        }
        Err(ClientError::ServerError(res.status().as_u16()))
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

    fn result_path(&self) -> String {
        // TODO: get player from token instead of param
        format!("{}/api/match", self.server_url)
    }

    fn validation_path(&self) -> String {
        format!("{}/auth/validate", self.server_url)
    }

    pub fn update_state(&self, state: ClientState) -> Result<(), ClientError> {
        let mut data = self.state.lock().map_err(|_| ClientError::StateError)?;
        *data = state;
        Ok(())
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

        let (tx1, rx1) = std::sync::mpsc::channel();
        let (tx2, rx2) = std::sync::mpsc::channel();

        println!("Sending first request to: {}", client1.result_path());
        let f = std::thread::spawn(move || {
            let req1 = client1.send_result(&result1);
            println!("First request got a response");
            tx1.send(req1).unwrap();
        });

        sleep(Duration::from_millis(1000));

        println!("Sending second request...");
        let s = std::thread::spawn(move || {
            let req2 = client2.send_result(&result2);
            println!("Second request got a response");
            tx2.send(req2).unwrap();
        });

        f.join().expect("First thread failed");
        s.join().expect("Second thread failed");

        match rx1.recv().unwrap() {
            Ok(res) => {
                println!("First request status: {}", res.status().as_u16());
                assert_eq!(res.status().as_u16(), 201);
            }
            Err(e) => panic!("{}", e),
        }

        match rx2.recv().unwrap() {
            Ok(res) => {
                println!("Second request status: {}", res.status().as_u16());
                assert_eq!(res.status().as_u16(), 201);
            }
            Err(e) => panic!("{}", e),
        }
    }
}
