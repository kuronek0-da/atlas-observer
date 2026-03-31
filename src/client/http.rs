use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    client::{
        models::{MatchedResponse, QueueRequest, ValidationResponse},
        state::ClientState,
    },
    config::{Config, ConfigError},
    validation::result::MatchResult,
};
use reqwest::{
    StatusCode,
    blocking::{Client, Response},
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("failed to execute request or could not reach the server")]
    RequestError,
    #[error("something went wrong, server status response: '{0}'")]
    ServerError(u16),
    #[error("token expired or is invalid")]
    AuthorizationError,
    #[error("could not read client state")]
    StateError,
    #[error("invalid state detected: '{0}'")]
    InvalidStateError(ClientState),
    #[error("could not parse '{0}'")]
    ParseError(String),
    #[error("content not found")]
    NotFoundError,
}

/// Handles the requests to the server
#[derive(Debug, Clone)]
pub struct ClientManager {
    token: String,
    server_url: String,
    state: Arc<Mutex<ClientState>>,
    client: Client,
}


impl ClientManager {
    // 5 minutes
    const REQUEST_TIMEOUT_SECS: Duration = Duration::from_secs(300);
    pub fn new(config: Config) -> Result<Self, ConfigError> {
        Ok(ClientManager {
            token: config.token,
            server_url: config.server_url,
            state: Arc::new(Mutex::new(ClientState::Idle)),
            client: Client::builder()
                .build()
                .unwrap(),
        })
    }

    pub fn new_test(config: Config) -> Result<Self, ConfigError> {
        Ok(ClientManager {
            token: config.token,
            server_url: config.server_url,
            state: Arc::new(Mutex::new(ClientState::Idle)),
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

    fn send_post<T: Serialize>(&self, path: String, body: &T) -> Result<Response, ClientError> {
        let url = format!("{}/{}", self.server_url, path);
        self.client
            .post(url)
            .timeout(ClientManager::REQUEST_TIMEOUT_SECS)
            .headers(self.construct_headers())
            .json(body)
            .send()
            .map_err(|_| {
                ClientError::RequestError
            })
    }

    pub fn validate_token(&self) -> Result<ValidationResponse, ClientError> {
        let res = self.send_post("auth/validate".to_string(), &"".to_string())?;
        if res.status().is_success() {
            return res
                .json()
                .map_err(|_| ClientError::ParseError("validation response".to_string()));
        }
        if res.status().as_u16() == 401 {
            return Err(ClientError::AuthorizationError);
        }
        Err(ClientError::ServerError(res.status().as_u16()))
    }

    pub fn send_result(&self, result: &MatchResult) -> Result<Response, ClientError> {
        let state = self.state
            .lock()
            .map_err(|_| ClientError::StateError)?
            .to_owned();

        let res = match state {
            ClientState::PlayingRanked(_) => self.send_post("api/match".to_string(), &result)?,
            _ => Err(ClientError::InvalidStateError(state))?
        };

        if res.status().is_success() {
            Ok(res)
        } else {
            Err(ClientError::ServerError(res.status().as_u16()))
        }
    }

    pub fn send_queue_request(&self) -> Result<MatchedResponse, ClientError> {
        let state = self.clone_state();
        let state = state
            .lock()
            .map_err(|_| ClientError::StateError)?
            .to_owned();
        let res = match state {
            ClientState::HostingRanked(session_id) => {
                let body = QueueRequest { session_id };
                self.send_post("api/queue".to_string(), &body)?
            }
            ClientState::JoinedRanked(session_id) => {
                let body = String::new();
                self.send_post(format!("api/queue/{}", session_id), &body)?
            }
            s => Err(ClientError::InvalidStateError(s))?,
        };

        match res.status() {
            StatusCode::REQUEST_TIMEOUT => Err(ClientError::ServerError(res.status().as_u16())),
            StatusCode::NOT_FOUND => Err(ClientError::NotFoundError),
            _ => {
                if res.status().is_success() {
                    return Ok(res
                        .json()
                        .map_err(|_| ClientError::ParseError("QueueResponse".to_string()))?);
                }
                Err(ClientError::ServerError(res.status().as_u16()))
            }
        }
    }

    pub fn send_cancel_queue(&self) -> Result<String, ClientError> {
        let res = self.client.delete(format!("{}/api/queue", self.server_url))
            .headers(self.construct_headers())
            .send()
            .map_err(|_| ClientError::RequestError)?;
        if res.status().is_success() {
            if let Ok(msg) = res.text() {
                return Ok(msg);
            }
            return Ok(String::from("Queue successfully canceled"))
        }
        Err(ClientError::ServerError(res.status().as_u16()))
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

    use crate::{game::{
        game_char::{GameChar, Moon},
        state::{GameTimers, Player},
    }, memory::addresses::ClientMode};

    use super::*;

    fn mock_match_result(session_id: String, client_mode: ClientMode) -> MatchResult {
        let local_player = match client_mode {
            ClientMode::Host => 1,
            _ => 2,
        };
        let timers = GameTimers::new(10000, 1200, 4000);

        let p1 = Player {
            character: GameChar::Akiha,
            moon: Moon::Half,
            score: 2,
        };
        let p2 = Player {
            character: GameChar::Seifuku,
            moon: Moon::Crescent,
            score: 1,
        };
        let players = [p1, p2];
        MatchResult::new(
            session_id,
            client_mode,
            local_player,
            players,
            timers
        )
        .expect("Could not mock MatchResult")
    }

    #[test]
    fn test_send_result() {
        let config = Config::load_test().unwrap();
        let client1 = ClientManager::new_test(config.clone()).expect("Failed to load config.");
        client1.update_state(ClientState::hosting());

        let session_id = client1
            .state
            .lock()
            .expect("Could not get client session id")
            .session()
            .expect("Session id not set for client")
            .to_string();
        let client2 = ClientManager::new_test(config).expect("Failed to load config.");
        client2.update_state(ClientState::JoinedRanked(session_id.clone()));

        let result1 = mock_match_result(session_id.clone(), ClientMode::Host);
        println!("{}", result1);
        let result2 = mock_match_result(session_id, ClientMode::Client);
        println!("{}", result2);

        let (tx1, rx1) = std::sync::mpsc::channel();
        let (tx2, rx2) = std::sync::mpsc::channel();

        println!("Sending first request to: {}/api/match", client1.server_url);
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
