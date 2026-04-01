use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

use crate::{
    client::{ClientManager, ClientState, http::ClientError},
    log, memory,
    ui::AppCommand,
    validation,
};

pub fn run(client: ClientManager, log_tx: Sender<String>, cmd_rx: Receiver<AppCommand>) {
    'outer: loop {
        log_tx.send("Waiting for command.".to_string()).ok();

        let client_state = match cmd_rx.recv().expect("Sender dropped.") {
            AppCommand::Host(state) => state,
            AppCommand::Join(state) => state,
            _ => continue,
        };

        let waiting_msg = match &client_state {
            ClientState::HostingRanked(code) => format!("Hosting with code '{}'", code),
            ClientState::JoinedRanked(code) => format!("Trying to join '{}'...", code),
            _ => continue,
        };

        if let Err(e) = client.update_state(client_state) {
            log(format!("Client Error: {}", e), &log_tx);
            continue;
        }

        let is_cancelled = Arc::new(AtomicBool::new(false));
        let is_cancelled_clone = is_cancelled.clone();
        let client_clone = client.clone();
        let log_tx_clone = log_tx.clone();

        log(waiting_msg, &log_tx);

        let queue_thread = std::thread::spawn(move || {
            let req = client_clone.send_queue_request();
            if is_cancelled_clone.load(Ordering::Relaxed) {
                return;
            }
            match req {
                Ok(res) => {
                    log(
                        format!("Player connected: {}", res.opponent_discord_username),
                        &log_tx_clone,
                    );
                    return;
                }
                Err(ClientError::ServerError(408)) => {
                    log("Request expired".to_string(), &log_tx_clone)
                }
                Err(ClientError::NotFoundError) => {
                    log("Match not found".to_string(), &log_tx_clone)
                }
                Err(e) => log(format!("Client Error: {}", e), &log_tx_clone),
            }
            is_cancelled_clone.store(true, Ordering::Relaxed);
        });

        loop {
            match cmd_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(AppCommand::Stop(state)) => {
                    log("Sending cancel request...".to_string(), &log_tx);
                    match client.send_cancel_queue() {
                        Ok(msg) => log(msg, &log_tx),
                        Err(e) => log(format!("Client Error: {}", e), &log_tx),
                    }
                    is_cancelled.store(true, Ordering::Relaxed);
                    if let Err(e) = client.update_state(state) {
                        log(format!("Client Error: {}", e), &log_tx);
                    }
                    break;
                }
                _ => {}
            }
            if queue_thread.is_finished() {
                if is_cancelled.load(Ordering::Relaxed) {
                    if let Err(e) = client.update_state(ClientState::Idle) {
                        log(format!("Client Error: {}", e), &log_tx);
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

    if m.join().is_err() {
        log(
            "Something went wrong in the memory thread".to_string(),
            &log_tx,
        );
    }

    if v.join().is_err() {
        log(
            "Something went wrong in the validator thread".to_string(),
            &log_tx,
        );
    }
}
