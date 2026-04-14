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
use thiserror::Error;

use crate::{
    game::state::GameState,
    log,
    memory::{MemoryManager, process::MemoryError},
};

#[derive(Error, Debug)]
enum ThreadError {
    #[error("Memory polling for ids timed out")]
    IdsPollingTimeoutError,
    #[error("Memory Error: {0}")]
    MemoryError(#[from] MemoryError),
}

enum PollResult<T> {
    Ready(T),
    Canceled,
    Err(ThreadError),
}

pub fn run(
    game_state_tx: Sender<GameState>,
    log_tx: Sender<String>,
    session_tx: Sender<Vec<String>>,
    is_queue_canceled: Arc<AtomicBool>,
    are_players_paired: Arc<AtomicBool>,
) {
    let mut memory = MemoryManager::new();

    // Ensure clean state
    match wait_for_clean_start(&mut memory, &is_queue_canceled, &log_tx) {
        PollResult::Canceled => return,
        PollResult::Err(e) => return error!("Error while waiting for clean start: {}", e),
        _ => {}
    }

    // Attach to processess
    match attach_to_process(&mut memory, &is_queue_canceled, &log_tx) {
        PollResult::Canceled => return,
        PollResult::Err(e) => return error!("Error while attaching to process: {}", e),
        _ => {}
    }

    log("Attached to MBAA and CCCaster".into(), &log_tx);
    // Get Session IDs
    let session_ids = match acquire_session_ids(&mut memory, &is_queue_canceled, &log_tx) {
        PollResult::Ready(ids) => ids,
        PollResult::Canceled => return,
        PollResult::Err(e) => {
            error!("Error while acquiring session ids: {}", e);
            return;
        }
    };

    // Send IDs and wait for pairing
    if session_tx.send(session_ids).is_err() {
        error!("Error sending session ids: validator thread dropped");
        log(
            "Internal error, check logs for details".to_string(),
            &log_tx,
        );
        return;
    }
    // Doesn't return Err()
    match wait_for_pairing(&is_queue_canceled, &are_players_paired) {
        PollResult::Canceled => return,
        _ => {}
    }

    // Main polling loop
    run_main_polling_loop(memory, game_state_tx, log_tx);
}

fn wait_for_clean_start(
    memory: &mut MemoryManager,
    is_canceled: &Arc<AtomicBool>,
    log_tx: &Sender<String>,
) -> PollResult<()> {
    if memory.is_melty_running() {
        log(
            "MBAA session detected. Restart MBAA.exe".to_string(),
            log_tx,
        );
        sleep(Duration::from_secs(2));
    } else {
        log("Waiting for MBAA and CCCaster".into(), log_tx);
    }

    while memory.is_melty_running() {
        if check_cancel(is_canceled) {
            return PollResult::Canceled;
        }
        sleep(Duration::from_secs(2));
    }
    PollResult::Ready(())
}

fn attach_to_process(
    memory: &mut MemoryManager,
    is_canceled: &Arc<AtomicBool>,
    log_tx: &Sender<String>,
) -> PollResult<()> {
    let mut has_logged_err = false;

    loop {
        if check_cancel(is_canceled) {
            return PollResult::Canceled;
        }

        match memory.attach() {
            Ok(_) => return PollResult::Ready(()),
            Err(MemoryError::InvalidCCCaster) => {
                log("Invalid version".to_string(), log_tx);
                is_canceled.store(true, Ordering::Relaxed);
                return PollResult::Err(ThreadError::from(MemoryError::InvalidCCCaster));
            }
            Err(MemoryError::MultipleProcessesError(ref process)) => {
                if !has_logged_err {
                    let err = format!(
                        "Error: {}",
                        MemoryError::MultipleProcessesError(process.clone())
                    );
                    error!("{}", &err);
                    log(err.clone(), log_tx);
                    has_logged_err = true;
                }
            }
            Err(_) => {}
        }
        sleep(Duration::from_secs(2));
    }
}

fn check_cancel(is_canceled: &Arc<AtomicBool>) -> bool {
    if is_canceled.load(Ordering::Relaxed) {
        info!("Memory thread shutdown signaled");
        return true;
    }
    false
}

fn acquire_session_ids(
    memory: &mut MemoryManager,
    is_canceled: &Arc<AtomicBool>,
    log_tx: &Sender<String>,
) -> PollResult<Vec<String>> {
    // Try to read ids every 1s for 15s
    let start = Instant::now();
    loop {
        if check_cancel(is_canceled) {
            return PollResult::Canceled;
        }

        match memory.poll_session_ids() {
            Ok(ids) if !ids.is_empty() => break PollResult::Ready(ids),
            Ok(_) => {
                if start.elapsed() >= Duration::from_secs(15) {
                    log(
                        "Could not find session, matching players is not possible".to_string(),
                        log_tx,
                    );
                    return PollResult::Err(ThreadError::IdsPollingTimeoutError);
                }
                std::thread::sleep(Duration::from_secs(1))
            }
            Err(e) => {
                log(
                    "Something went wrong, failed to read session ids".to_string(),
                    log_tx,
                );
                return PollResult::Err(ThreadError::from(e));
            }
        }
    }
}

/// Checks every 1 second if players are paired and if queue is canceled
/// # Returns a PollResult:
/// * Ready(()) if is_canceled is false and are_paired becomes true
/// * Canceled if is_canceled becomes true
fn wait_for_pairing(is_canceled: &Arc<AtomicBool>, are_paired: &Arc<AtomicBool>) -> PollResult<()> {
    while !are_paired.load(Ordering::Relaxed) {
        if check_cancel(is_canceled) {
            return PollResult::Canceled;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    PollResult::Ready(())
}

fn run_main_polling_loop(
    mut memory: MemoryManager,
    game_state_tx: Sender<GameState>,
    log_tx: Sender<String>,
) {
    info!("Memory thread: started reading MBAA");
    // Starts reading MBAA and tries to send to validator thread
    let mut was_in_game = true;
    loop {
        match memory.poll_game_state() {
            Ok(state) => {
                report_gamestate(&state, &mut was_in_game, &log_tx);
                if game_state_tx.send(state).is_err() {
                    error!("Memory thread: failed to send state to validator thread.");
                    log("Internal error, stopped reading MBAA".to_string(), &log_tx);
                    break;
                }

                sleep(Duration::from_millis(500));
            }
            Err(e) => {
                sleep(Duration::from_secs(1));
                if !memory.is_melty_running() {
                    info!("Game closed");
                    log(format!("Game closed"), &log_tx);
                } else {
                    error!("Memory Error while polling game state: {}", e);
                    log(
                        "Something went wrong while reading MBAA/CCCaster".to_string(),
                        &log_tx,
                    );
                    log(
                        "Make sure you're using the right CCCaster version".to_string(),
                        &log_tx,
                    );
                }
                memory.detach();
                break;
            }
        }
    }

    info!("Shutting down memory thread.");
}

#[cfg(test)]
mod tests {
}

/// Starts memory reading poll and reporting to the ui
//pub fn run(
//    game_state_tx: Sender<GameState>,
//    log_tx: &Sender<String>,
//    session_tx: Sender<Vec<String>>,
//    is_queue_canceled: Arc<AtomicBool>,
//    are_players_paired: Arc<AtomicBool>,
//) {
//    info!("Memory thread started");
//    let mut memory = MemoryManager::new();
//
//    if memory.is_melty_running() {
//        log(format!("MBAA session detected. Restart MBAA.exe."), log_tx);
//
//        while memory.is_melty_running() {
//            if is_queue_canceled.load(Ordering::Relaxed) {
//                return;
//            }
//            sleep(Duration::from_secs(2));
//        }
//    }
//
//    log("Waiting for MBAA.exe and CCCaster...".to_string(), log_tx);
//
//    // logs MultipleProcessesError once
//    let mut has_logged_mpe = false;
//    while let Err(e) = memory.attach() {
//        if is_queue_canceled.load(Ordering::Relaxed) {
//            info!("Shutting down memory thread");
//            return;
//        }
//
//        match e {
//            MemoryError::MultipleProcessesError(_) if !has_logged_mpe => {
//                if let MemoryError::MultipleProcessesError(ref process) = e {
//                    let close_msg = format!("Close the other '{}' processes.", process);
//                    error!("Memory Error: {}", e);
//                    log(close_msg, log_tx);
//
//                    has_logged_mpe = true;
//                } else {
//                    has_logged_mpe = false;
//                }
//            }
//            MemoryError::InvalidCCCaster => {
//                error!("Memory Error: {}", e);
//                log(
//                    "Invalid CCCaster version or player is not on netplay.".to_string(),
//                    log_tx,
//                );
//
//                is_queue_canceled.store(true, Ordering::Relaxed);
//                info!("Shutting down memory thread");
//                return;
//            }
//            _ => {}
//        }
//
//        sleep(Duration::from_secs(2));
//    }
//
//    info!("Attached to MBAA and CCCaster");
//    log(format!("Attached to MBAA and CCCaster"), log_tx);
//
//    info!("Trying to connect read session ids...");
//    log(format!("Trying to pair with opponent..."), log_tx);
//
//    // Try to read ids every 1s for 15s
//    let start = Instant::now();
//    let session_ids = loop {
//        if is_queue_canceled.load(Ordering::Relaxed) {
//            info!("Shutting down memory thread.");
//            return;
//        }
//
//        match memory.poll_session_ids() {
//            Ok(ids) if !ids.is_empty() => break ids,
//            Ok(_) => {
//                if start.elapsed() >= Duration::from_secs(15) {
//                    error!("Memory polling for session ids timed out.");
//                    log(
//                        "Could not find session, matching players is not possible".to_string(),
//                        log_tx,
//                    );
//                    info!("Shutting down memory thread.");
//                    return;
//                }
//                std::thread::sleep(Duration::from_secs(1))
//            }
//            Err(e) => {
//                error!("Memory Error: {}", e);
//                log(format!("Something went wrong: {}", e), log_tx);
//                return;
//            }
//        }
//    };
//
//    // Try to send read ids to validator thread
//    if session_tx.send(session_ids).is_err() {
//        error!("Failed to send session ids.");
//        log(
//            "Could not read player session, unable to proceed player pairing.".to_string(),
//            log_tx,
//        );
//        info!("Shutting down memory thread.");
//        log("Stopped reading MBAA".to_string(), log_tx);
//        return;
//    }
//
//    while !are_players_paired.load(Ordering::Relaxed) {
//        if is_queue_canceled.load(Ordering::Relaxed) {
//            info!("Shutting down memory thread.");
//            log("Stopped reading MBAA".to_string(), log_tx);
//            return;
//        }
//        std::thread::sleep(Duration::from_secs(1));
//    }
//
//    info!("Memory thread: started reading MBAA");
//    // Starts reading MBAA and tries to send to validator thread
//    let mut was_in_game = true;
//    loop {
//        match memory.poll_game_state() {
//            Ok(state) => {
//                report_gamestate(&state, &mut was_in_game, log_tx);
//                if game_state_tx.send(state).is_err() {
//                    error!("Memory thread: failed to send state to validator thread.");
//                    log("Internal error, stopped reading MBAA".to_string(), log_tx);
//                    break;
//                }
//
//                sleep(Duration::from_millis(16));
//            }
//            Err(e) => {
//                sleep(Duration::from_secs(1));
//                if !memory.is_melty_running() {
//                    info!("Game closed");
//                    log(format!("Game closed"), log_tx);
//                } else {
//                    error!("Memory Error: {}", e);
//                    log(
//                        "Something went wrong while reading MBAA/CCCaster".to_string(),
//                        log_tx,
//                    );
//                    log(
//                        "Make sure you're using the right CCCaster version".to_string(),
//                        log_tx,
//                    );
//                }
//                memory.detach();
//                break;
//            }
//        }
//    }
//
//    info!("Shutting down memory thread.");
//}

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
            log("Playing a match...".to_string(), log_tx);
        }
        _ => {}
    }
}
