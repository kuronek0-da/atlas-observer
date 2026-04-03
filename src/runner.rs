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
    info!("UI Started.");
    'outer: loop {
        log_tx.send("Waiting for command.".to_string()).ok();

        let client_state = match cmd_rx.recv().expect("Sender dropped.") {
            AppCommand::Host(state) => state,
            AppCommand::Join(state) => state,
            AppCommand::Exit(state) => {
                if let Err(e) = client.update_state(state) {
                    error!("Client Error: {}", e);
                    log("Exit failed, trying to force close...".to_string(), &log_tx);
                    exit_app(1)
                }
                continue;
            }
            _ => continue,
        };

        let waiting_msg = match &client_state {
            ClientState::HostingRanked(code) => format!("Hosting with code '{}'", code),
            ClientState::JoinedRanked(code) => format!("Trying to join '{}'...", code),
            _ => continue,
        };

        info!("Changing state to '{:?}'", client_state);
        if let Err(e) = client.update_state(client_state) {
            error!("Client Error: {}", e);
            log(
                "Failed to host/join. Check logs for more details.".to_string(),
                &log_tx,
            );
            continue;
        }

        let is_cancelled = Arc::new(AtomicBool::new(false));
        let is_cancelled_clone = is_cancelled.clone();
        let client_clone = client.clone();
        let log_tx_clone = log_tx.clone();

        log(waiting_msg, &log_tx);

        let queue_thread = std::thread::spawn(move || {
            info!("Sending queue request to the server");
            let req = client_clone.send_queue_request();
            if is_cancelled_clone.load(Ordering::Relaxed) {
                return;
            }
            match req {
                Ok(res) => {
                    info!("Player connected: {}", res.opponent_discord_username);
                    log(
                        format!("Player connected: {}", res.opponent_discord_username),
                        &log_tx_clone,
                    );
                    return;
                }
                Err(ClientError::ServerError(408)) => {
                    info!(
                        "Request expired for code/session '{:?}'",
                        *client_clone.client_state()
                    );
                    log("Request expired".to_string(), &log_tx_clone)
                }
                Err(ClientError::NotFoundError) => {
                    info!(
                        "Match not found for code/session '{:?}'",
                        *client_clone.client_state()
                    );
                    log("Match not found".to_string(), &log_tx_clone)
                }
                Err(e) => {
                    error!(
                        "Failed to send queue request for code '{:?}'",
                        *client_clone.client_state()
                    );
                    error!("Client Error: {}", e);
                    log(
                        "Something went wrong, failed to send queue request".to_string(),
                        &log_tx_clone,
                    );
                }
            }
            is_cancelled_clone.store(true, Ordering::Relaxed);
        });

        loop {
            match cmd_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(AppCommand::Stop(state)) => {
                    info!("Sending cancel queue request");
                    log("Sending cancel request...".to_string(), &log_tx);
                    match client.send_cancel_queue() {
                        Ok(msg) => log(msg, &log_tx),
                        Err(e) => {
                            error!("Client Error: {}", e);
                            log("Failed to send cancel queue request.".to_string(), &log_tx);
                            break;
                        }
                    }
                    is_cancelled.store(true, Ordering::Relaxed);
                    if let Err(e) = client.update_state(state) {
                        error!("Client Error: {}", e);
                        log(
                            "Internal error when exiting host/join state".to_string(),
                            &log_tx,
                        );
                    }
                    break;
                }
                Ok(AppCommand::Exit(state)) => {
                    break cancel_and_exit(state, &log_tx, &client, &is_cancelled);
                }
                _ => {}
            }
            if queue_thread.is_finished() {
                if is_cancelled.load(Ordering::Relaxed) {
                    if let Err(e) = client.update_state(ClientState::Idle) {
                        error!("Client Error: {}", e);
                        log(
                            "Internal error when trying to update state".to_string(),
                            &log_tx,
                        );
                    }
                    break;
                }
                break 'outer;
            }
        }
    }

    let log_tx_memory = log_tx.clone();
    let log_tx_validator = log_tx.clone();

    let (tx, rx) = channel();
    let m = std::thread::spawn(move || memory::run(tx, &log_tx_memory));
    let v = std::thread::spawn(move || validation::run(rx, client, &log_tx_validator));

    if let Err(e) = m.join() {
        error!("Memory Thread Error: {:?}", e);
        log(
            "Something went wrong in the memory thread".to_string(),
            &log_tx,
        );
    }

    if let Err(e) = v.join() {
        error!("Validator Thread Error: {:?}", e);
        log(
            "Something went wrong in the validator thread".to_string(),
            &log_tx,
        );
    }
}

fn cancel_and_exit(
    state: ClientState,
    log_tx: &Sender<String>,
    client: &ClientManager,
    is_cancelled: &Arc<AtomicBool>,
) {
    log(
        "Sending cancel request before exiting...".to_string(),
        log_tx,
    );
    info!("Trying to send cancel queue request");
    match client.send_cancel_queue() {
        Ok(msg) => log(msg, log_tx),
        Err(e) => {
            log("Failed to send cancel queue request.".to_string(), log_tx);
            error!("Client Error: {}", e);
        }
    }
    is_cancelled.store(true, Ordering::Relaxed);
    if let Err(e) = client.update_state(state) {
        error!("Client Error: {}", e);
        log("Internal error when trying to exit.".to_string(), log_tx);
    }
}
