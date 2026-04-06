use std::{sync::mpsc::Sender, thread::sleep, time::Duration};

use log::{error, info};

use crate::{
    game::state::GameState,
    log,
    memory::{MemoryManager, process::MemoryError},
};

/// Starts memory reading poll and reporting to the ui
pub fn run(game_state_tx: Sender<GameState>, log_tx: &Sender<String>) {
    info!("Memory thread started");
    let mut memory = MemoryManager::new();

    loop {
        if memory.is_running() {
            log(format!("MBAA session detected. Restart MBAA.exe."), log_tx);

            while memory.is_running() {
                sleep(Duration::from_secs(2));
            }
        }

        // print!("\r{}", " ".repeat(100));
        log(format!("Waiting for MBAA.exe..."), log_tx);
        let mut has_multi_proc = false;
        while let Err(e) = memory.attach() {
            if !has_multi_proc {
                if let MemoryError::MultipleProcessesError(ref process) = e {
                    let close_msg = format!("Close the other '{}' processes.", process);
                    error!("Memory Error: {}", e);
                    log(close_msg, log_tx);
                }
                has_multi_proc = true;
            }
            sleep(Duration::from_secs(2));
        }

        let mut was_in_game = true;
        info!("Attached to MBAA.exe");
        log(format!("Attached to MBAA.exe"), log_tx);
        loop {
            match memory.poll_game_state() {
                Ok(state) => {
                    report_gamestate(&state, &mut was_in_game, log_tx);
                    if game_state_tx.send(state).is_err() {
                        log("Stoped reading MBAA".to_string(), log_tx);
                        info!("Receiver dropped, shutting down memory thread.");
                        return;
                    }
                }
                Err(e) => {
                    if !memory.is_running() {
                        info!("Game closed");
                        log(format!("Game closed. Waiting for next session..."), log_tx);
                    } else {
                        error!("Memory Error: {}", e);
                        log("Something went wrong while reading MBAA/CCCaster".to_string(), log_tx);
                        log("Make sure you're using the right version".to_string(), log_tx);
                    }
                    memory.detach();
                    break; // back to outer loop, not exit
                }
            }
            sleep(Duration::from_millis(16));
        }
    }
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
