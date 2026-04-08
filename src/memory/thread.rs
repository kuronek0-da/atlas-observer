use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread::sleep,
    time::{Duration, Instant},
};

use log::{error, info};

use crate::{
    game::state::GameState,
    log,
    memory::{MemoryManager, process::MemoryError},
};

/// Starts memory reading poll and reporting to the ui
pub fn run(
    game_state_tx: Sender<GameState>,
    log_tx: &Sender<String>,
    session_tx: Sender<Vec<String>>,
    is_queue_canceled: Arc<AtomicBool>,
    are_players_paired: Arc<AtomicBool>,
) {
    info!("Memory thread started");
    let mut memory = MemoryManager::new();

    if memory.is_melty_running() {
        log(format!("MBAA session detected. Restart MBAA.exe."), log_tx);

        while memory.is_melty_running() {
            if is_queue_canceled.load(Ordering::Relaxed) {
                return;
            }
            sleep(Duration::from_secs(2));
        }
    }

    log("Waiting for MBAA.exe and CCCaster...".to_string(), log_tx);

    // logs MultipleProcessesError once
    let mut has_logged_mpe = false;
    while let Err(e) = memory.attach() {
        if is_queue_canceled.load(Ordering::Relaxed) {
            info!("Shutting down memory thread");
            return;
        }

        if !has_logged_mpe {
            if let MemoryError::MultipleProcessesError(ref process) = e {
                let close_msg = format!("Close the other '{}' processes.", process);
                error!("Memory Error: {}", e);
                log(close_msg, log_tx);

                has_logged_mpe = true;
            } else {
                has_logged_mpe = false;
            }
        }
        sleep(Duration::from_secs(2));
    }

    info!("Attached to MBAA and CCCaster");
    log(format!("Attached to MBAA and CCCaster"), log_tx);

    info!("Trying to connect read session ids...");
    log(format!("Trying to pair with opponent..."), log_tx);

    // Try to read ids every 1s for 15s
    let start = Instant::now();
    let session_ids = loop {
        if is_queue_canceled.load(Ordering::Relaxed) {
            info!("Shutting down memory thread.");
            return;
        }

        match memory.poll_session_ids() {
            Ok(ids) if !ids.is_empty() => break ids,
            Ok(_) => {
                if start.elapsed() >= Duration::from_secs(15) {
                    error!("Memory polling for session ids timed out.");
                    log(
                        "Could not find session, matching players is not possible".to_string(),
                        log_tx,
                    );
                    info!("Shutting down memory thread.");
                    return;
                }
                std::thread::sleep(Duration::from_secs(1))
            }
            Err(e) => {
                error!("Memory Error: {}", e);
                log(format!("Something went wrong: {}", e), log_tx);
                return;
            }
        }
    };

    // Try to send read ids to validator thread
    if session_tx.send(session_ids).is_err() {
        error!("Failed to send session ids.");
        log(
            "Could not read player session, unable to proceed player pairing.".to_string(),
            log_tx,
        );
        info!("Shutting down memory thread.");
        return;
    }

    while !are_players_paired.load(Ordering::Relaxed) {
        if is_queue_canceled.load(Ordering::Relaxed) {
            info!("Shutting down memory thread.");
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // Starts reading MBAA and tries to send to validator thread
    let mut was_in_game = true;
    loop {
        match memory.poll_game_state() {
            Ok(state) => {
                report_gamestate(&state, &mut was_in_game, log_tx);
                if game_state_tx.send(state).is_err() {
                    error!("Memory thread: failed to send state to validator thread.");
                    log("Internal error, stopped reading MBAA".to_string(), log_tx);
                    break;
                }

                sleep(Duration::from_millis(16));
            }
            Err(e) => {
                if !memory.is_melty_running() {
                    info!("Game closed");
                    log(format!("Game closed"), log_tx);
                } else {
                    error!("Memory Error: {}", e);
                    log(
                        "Something went wrong while reading MBAA/CCCaster".to_string(),
                        log_tx,
                    );
                    log(
                        "Make sure you're using the right version".to_string(),
                        log_tx,
                    );
                }
                memory.detach();
                break;
            }
        }
    }

    info!("Shutting down memory thread.");
}

fn report_gamestate(state: &GameState, was_in_game: &mut bool, log_tx: &Sender<String>) {
    match state {
        GameState::NotInGame { .. } if *was_in_game => {
            info!("Not in game: {:?}", state);
            *was_in_game = false;
            log("Waiting for the match to start...".to_string(), log_tx);
        }
        GameState::InGame { .. } if !*was_in_game => {
            info!("In game: {:?}", state);
            *was_in_game = true;
            log("Match running...".to_string(), log_tx);
        }
        _ => {}
    }
}
