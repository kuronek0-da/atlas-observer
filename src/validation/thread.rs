use std::sync::mpsc::{Receiver, Sender};

use log::{error, info};

use crate::{
    client::{ClientManager, http::ClientError},
    game::state::GameState,
    log,
    validation::{Validator, Validity, result::MatchResult},
};

pub fn run(game_state_rx: Receiver<GameState>, client: &ClientManager, log_tx: Sender<String>) {
    info!("Started validator thread");

    let mut validator = match Validator::new(client.clone_state()) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to start validator: {}", e);
            log("Internal error, no session id found".into(), &log_tx);
            return;
        }
    };

    while let Ok(state) = game_state_rx.recv() {
        let validity = match validator.validate(state) {
            Ok(v) => v,
            Err(e) => {
                handle_fatal_error("Failed to validate game state", e, &log_tx);
                break;
            }
        };

        match validity {
            Validity::Valid => {}
            Validity::MatchFinished(result) => {
                handle_match_end(result, client, &log_tx);
            }
            Validity::Invalid(reason) => {
                handle_fatal_error("Invalid game state", reason, &log_tx);
                break;
            }
        }
    }

    info!("Validator thread closed.");
    log("Stopped validating game state".into(), &log_tx);
}

fn handle_match_end(result: MatchResult, client: &ClientManager, log_tx: &Sender<String>) {
    info!("Match result: {:?}", result);
    log("Sending match to the server...".into(), log_tx);

    let client_clone = client.clone();
    let log_tx_clone = log_tx.clone();

    std::thread::spawn(move || send_match_result(&result, client_clone, &log_tx_clone));
}

fn handle_fatal_error<E: std::fmt::Display>(context: &str, err: E, log_tx: &Sender<String>) {
    let msg = format!("{}: {}", context, err);
    error!("{}", msg);
    log(context.to_owned(), log_tx);
}

fn send_match_result(result: &MatchResult, client: ClientManager, log_tx: &Sender<String>) {
    info!("Sending match to the server. ID: {}", result.session_id());

    let attempt = attempt_send(&client, result);

    match attempt {
        Ok(msg) => {
            info!("Match successfully sent, response: {}", msg);
            log(format!("[Match] {}", &result), log_tx);
            log(format!("[Registered Result] {}", msg), log_tx);
        }
        Err(ClientError::ServerError(408)) => {
            error!("Timeout reporting match");
            log("Report failed: request timeout".into(), log_tx);
        }
        Err(e) => {
            error!("Client Error: {}", e);
            log("Failed to send match result to the server".into(), log_tx);
        }
    }
}

fn attempt_send(client: &ClientManager, result: &MatchResult) -> Result<String, ClientError> {
    let res = client.send_result(result)?;
    res.text()
        .map_err(|_| ClientError::ParseError("result response".into()))
}
