use std::sync::mpsc::{Receiver, Sender};

use crate::{client::{ClientManager, ClientState}, game::state::GameState, log, validation::{Validator, Validity}};

pub fn run(game_state_rx: Receiver<GameState>, client: ClientManager, log_tx: &Sender<String>) {
    let mut validator = Validator::new(client.clone_state());
    let mut is_playing = false;

    for state in game_state_rx {
        match validator.validate(state) {
            Ok(validity) => match validity {
                Validity::Invalid(reason) => {
                    log(format!("Invalid game state: {}", reason), log_tx);
                    break;
                }
                Validity::MatchFinished(result) => {
                    log(format!("Match ended: {}", &result), log_tx);
                    log("Sending match to the server...".to_string(), log_tx);
                    let client = client.clone();
                    let log_tx_client = log_tx.clone();
                    std::thread::spawn(move || match client.send_result(&result) {
                        Ok(res) => match res.text() {
                            Ok(msg) => log(msg, &log_tx_client),
                            Err(_) => log(
                                "Match sent, but couldn't read response message".to_string(),
                                &log_tx_client,
                            ),
                        },
                        Err(e) => {
                            log(
                                format!("Could not send match to the server: {}", e),
                                &log_tx_client,
                            );
                        }
                    });
                }
                Validity::Valid(session) => {
                    if !is_playing {
                        is_playing = true;
                        if let Err(e) = client.update_state(ClientState::PlayingRanked(session)) {
                            log(format!("Client Errr: {}", e), log_tx);
                        }
                    }
                }
            },
            Err(e) => {
                log(format!("Validator error: {:?}", e), log_tx);
                break;
            }
        }
    }
}
