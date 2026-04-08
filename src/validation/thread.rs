use std::sync::mpsc::{Receiver, Sender};

use log::{debug, error, info};

use crate::{
    client::{ClientManager, ClientState, http::ClientError},
    game::state::GameState,
    log,
    validation::{Validator, Validity, result::MatchResult},
};

pub fn run(game_state_rx: Receiver<GameState>, client: &ClientManager, log_tx: Sender<String>) {
    info!("Started validator thread");

    let log_ref = &log_tx;
    let mut validator = Validator::new(client.clone_state());
    let mut is_playing = false;

    while let Ok(state) = game_state_rx.recv() {
        match validator.validate(state) {
            Ok(validity) => match validity {
                Validity::Invalid(reason) => {
                    error!("Invalid game state: {}", reason);
                    log(format!("Invalid game state: {}", reason), log_ref);
                    break;
                }
                Validity::MatchFinished(result) => {
                    info!("Match result: {:?}", result);
                    log(format!("Match ended: {}", &result), log_ref);

                    log("Sending match to the server...".to_string(), log_ref);

                    let client_clone = client.clone();
                    let log_tx_client = log_tx.clone();

                    std::thread::spawn(move || {
                        send_match_result(&result, client_clone, &log_tx_client)
                    });
                }
                Validity::Valid(session_ids) => {
                    if !is_playing {
                        is_playing = true;
                        let state = ClientState::PlayingRanked(session_ids);
                        info!("Changing state to {:?}", state);
                        if let Err(e) = client.update_state(state) {
                            error!("Client Error: {}", e);
                            log(
                                "Something went wrong when trying to enter 'Playing' state."
                                    .to_string(),
                                log_ref,
                            );
                        }
                    }
                }
            },
            Err(e) => {
                error!("Validator Error: {}", e);
                log("Failed to validate game state".to_string(), log_ref);
                break;
            }
        }
    }
    info!("Validator channel closed.");
    log("Stopped validating game state".to_string(), log_ref);
}

fn send_match_result(result: &MatchResult, client: ClientManager, log_tx: &Sender<String>) {
    debug!("Sending match to the server. IDS: {}", result.session_id());
    match client.send_result(result) {
        Ok(res) => match res.text() {
            Ok(msg) => {
                info!("Match successfully sent, response: {}", msg);
                log(format!("[Match registered] {}", msg), log_tx);
            }
            Err(_) => {
                info!("Match successfully sent. No message");
                log(
                    "Match registered, but couldn't read response message".to_string(),
                    log_tx,
                )
            }
        },
        Err(e) => match e {
            ClientError::ServerError(408) => {
                error!("ClientError: {}", e);
                log(
                    "Report failed, results took too long to reach the server".to_string(),
                    log_tx,
                );
            }
            _ => {
                error!("Client Error: {}", e);
                log(
                    "Failed to send match result to the server".to_string(),
                    log_tx,
                );
            }
        },
    }
}
