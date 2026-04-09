use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

use log::{error, info};

use crate::{
    client::{ClientManager, ClientState, http::ClientError},
    exit_app, log, memory,
    ui::AppCommand,
    validation,
};

pub fn run(client: ClientManager, log_tx: Sender<String>, cmd_rx: Receiver<AppCommand>) {
    let log_ref = &log_tx;

    let is_queue_canceled = Arc::new(AtomicBool::new(false));

    'outer: loop {
        log("Waiting for command...".to_string(), log_ref);
        // Shouldn't receive any stop commands at this point
        if !update_state_from_command(&client, &cmd_rx, log_ref) {
            break;
        }

        if *client.client_state() != ClientState::WaitingForOpponent {
            return;
        }

        let are_players_paired = Arc::new(AtomicBool::new(false));

        let log_tx_memory = log_tx.clone();
        let (game_state_tx, game_state_rx) = channel();
        let (session_ids_tx, session_ids_rx) = channel();
        let is_queue_canceled_mt = Arc::clone(&is_queue_canceled);
        let are_players_paired_mt = Arc::clone(&are_players_paired);

        std::thread::spawn(move || {
            memory::run(
                game_state_tx,
                &log_tx_memory,
                session_ids_tx,
                is_queue_canceled_mt,
                are_players_paired_mt,
            )
        });

        // Checks if a stop or exit have been issued before trying to get session ids
        let session_ids: Vec<String> = loop {
            if cancel_and_exit_from_command(&cmd_rx, log_ref, &client, &is_queue_canceled, false) {
                continue 'outer;
            }

            match session_ids_rx.try_recv() {
                Ok(ids) => break ids,
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
            }
        };

        info!("Trying to send pairing request");
        let client_clone = client.clone();
        let client_state_clone = client.clone_state();
        let log_tx_queue = log_ref.clone();
        let is_queue_canceled_qt = Arc::clone(&is_queue_canceled);

        let queue_thread =
            std::thread::spawn(move || match client_clone.send_queue_request(session_ids) {
                Ok(res) => {
                    info!("Opponent: {} | Session ID: {}", res.opponent_discord_username, res.session_id);
                    log(
                        format!("Playing ranked against {}", res.opponent_discord_username),
                        &log_tx_queue,
                    );

                    match client_state_clone.lock() {
                        Ok(mut s) => {
                            *s = ClientState::PlayingRanked(res.session_id);
                        }
                        Err(e) => {
                            error!("Could not load client state on queue thread: {}", e);
                            log(
                                "Failed to start ranked, check logs for details.".to_string(),
                                &log_tx_queue,
                            );
                            is_queue_canceled_qt.store(true, Ordering::Relaxed);
                        }
                    }
                }
                Err(e) => {
                    error!("Client Error: {}", e);
                    match e {
                        ClientError::ServerError(408) => {
                            log("Ranked queue timed out".to_string(), &log_tx_queue)
                        }
                        _ if is_queue_canceled_qt.load(Ordering::Relaxed) => {}
                        _ => {
                            log(
                                "Could not send ranked request to the server.".to_string(),
                                &log_tx_queue,
                            );
                            is_queue_canceled_qt.store(true, Ordering::Relaxed);
                        }
                    }
                    return;
                }
            });

        loop {
            // Checks if Stop or Exit commands have been issued every 500 milis
            if cancel_and_exit_from_command(&cmd_rx, log_ref, &client, &is_queue_canceled, true) {
                continue 'outer;
            }

            if queue_thread.is_finished() {
                // if queue not cancelled, then players are paired
                if is_queue_canceled.load(Ordering::Relaxed) {
                    if let Err(e) = client.update_state(ClientState::Idle) {
                        error!("Client Error: {}", e);
                        log(
                            "Internal error. Check logs for details.".to_string(),
                            log_ref,
                        );
                        return;
                    }
                    continue 'outer;
                }
                are_players_paired.store(true, Ordering::Relaxed);
                break;
            }

            std::thread::sleep(Duration::from_millis(500));
        }

        validation::run(game_state_rx, &client, log_tx.clone());
    }
    info!("Shutting down runner thread.");
    if let Err(e) = client.update_state(ClientState::Exit) {
        error!("Client Error: {}", e);
        log("Internal error, check logs for details.".to_string(), &log_tx);
    }
}

/// Returns true if a command was received
fn cancel_and_exit_from_command(
    cmd_rx: &Receiver<AppCommand>,
    log_tx: &Sender<String>,
    client: &ClientManager,
    is_queue_canceled: &Arc<AtomicBool>,
    should_make_request: bool,
) -> bool {
    // Checks if Stop or Exit commands have been issued every 500 milis
    match cmd_rx.try_recv() {
        // Cancel queue request if true
        Ok(cmd) if matches!(cmd, AppCommand::Stop | AppCommand::Exit) => {
            if should_make_request {
                info!("Sending cancel queue request...");
                log("Sending cancel queue request...".to_string(), log_tx);

                match client.send_cancel_queue() {
                    Ok(msg) => {
                        info!("Queue canceled");
                        log(msg, log_tx);
                    }
                    Err(e) => {
                        error!("Client Error: {}", e);
                        log(
                            "Failed to send cancel request to the server.".to_string(),
                            log_tx,
                        );
                    }
                }
            }

            let s = if matches!(cmd, AppCommand::Stop) {
                ClientState::Idle
            } else {
                ClientState::Exit
            };

            if let Err(e) = client.update_state(s) {
                error!("Client Error: {}", e);
                log(
                    "Internal error trying to execute command. Check logs for details".to_string(),
                    log_tx,
                );
            }

            is_queue_canceled.store(true, Ordering::Relaxed);
            return true;
        }
        _ => false,
    }
}

/// Returns true if operation succeeded
fn update_state_from_command(
    client: &ClientManager,
    cmd_rx: &Receiver<AppCommand>,
    log_tx: &Sender<String>,
) -> bool {
    let cmd = match cmd_rx.recv() {
        Ok(v) => v,
        Err(_) => {
            error!("Runner thread: command sender dropped.");
            log(
                "Internal error, please restart Atlas. Check logs for details.".to_string(),
                log_tx,
            );
            return false;
        }
    };

    match cmd {
        AppCommand::Start => {
            if let Err(e) = client.update_state(ClientState::WaitingForOpponent) {
                error!("Client Error: {}", e);
                log(
                    "Failed to start ranked, check logs for details.".to_string(),
                    log_tx,
                );
            }
        }
        AppCommand::Stop => {
            if let Err(e) = client.update_state(ClientState::Idle) {
                error!("Client Error: {}", e);
                log(
                    "Failed to start ranked, check logs for details.".to_string(),
                    log_tx,
                );
            }
        }
        AppCommand::Exit => {
            if let Err(e) = client.update_state(ClientState::Exit) {
                error!("Client Error: {}", e);
                log("Exit failed, trying to force close...".to_string(), log_tx);
                exit_app(1)
            }
        }
    }
    true
}
