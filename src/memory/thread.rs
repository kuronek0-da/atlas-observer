use std::{sync::mpsc::Sender, thread::sleep, time::Duration};

use crate::{game::state::GameState, log, memory::MemoryManager};

/// Starts memory reading poll and reporting to the ui
pub fn run(game_state_tx: Sender<GameState>, log_tx: &Sender<String>) {
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
        while let Err(_) = memory.attach() {
            sleep(Duration::from_secs(2));
        }

        let mut was_in_game = true;
        log(format!("Attached to MBAA.exe"), log_tx);
        loop {
            match memory.poll() {
                Ok(state) => {
                    report_gamestate(&state, &mut was_in_game, log_tx);
                    if game_state_tx.send(state).is_err() {
                        log(
                            format!("Receiver dropped, shutting down memory thread."),
                            log_tx,
                        );
                        return;
                    }
                }
                Err(e) => {
                    if !memory.is_running() {
                        log(format!("Game closed. Waiting for next session..."), log_tx);
                    } else {
                        log(format!("Lost connection: {:?}", e), log_tx);
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
            *was_in_game = false;
            log("Waiting for the match to start...".to_string(), log_tx);
        }
        GameState::InGame { .. } if !*was_in_game => {
            *was_in_game = true;
            log("Match running...".to_string(), log_tx);
        }
        _ => {}
    }
}
