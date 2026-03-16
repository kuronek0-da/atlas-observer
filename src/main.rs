mod client;
mod config;
mod game;
mod memory;
mod validation;
use std::{
    sync::{
        mpsc::{Receiver, Sender, channel},
    },
    thread::sleep,
    time::Duration,
};

use crate::client::ClientManager;
use crate::client::state::ClientState;
use crate::memory::manager::MemoryManager;
use crate::validation::validator::{Validator, Validity};
use crate::{game::state::GameState};

fn main() {
    println!("=== ATLAS OBSERVER ===\n");

    let state = match host_or_join_input() {
        Some(s) => s,
        None => std::process::exit(1)
    };

    let client = match ClientManager::new(state) {
        Ok(cli) => cli,
        Err(e) => panic!("Could not start client: {}", e)
    };

    let (tx, rx) = channel();

    let m = std::thread::spawn(move || memory_thread(tx));
    let v = std::thread::spawn(move || validator_thread(rx, client));

    m.join();
    v.join();
}

fn memory_thread(tx: Sender<GameState>) {
    let mut memory = MemoryManager::new();

    loop {
        if memory.is_running() {
            update_status(format!("MBAA session detected. Restart MBAA.exe."));

            while memory.is_running() {
                sleep(Duration::from_secs(2));
            }
        }

        // print!("\r{}", " ".repeat(100));
        update_status(format!("Waiting for MBAA.exe..."));
        while let Err(_) = memory.attach() {
            sleep(Duration::from_secs(2));
        }

        update_status(format!("Attached to MBAA.exe"));
        loop {
            match memory.poll() {
                Ok(state) => {
                    if tx.send(state).is_err() {
                        update_status(format!("Receiver dropped, shutting down memory thread."));
                        return;
                    }
                }
                Err(e) => {
                    if !memory.is_running() {
                        update_status(format!("Game closed. Ended ranked mode"));
                    } else {
                        update_status(format!("Lost connection: {:?}, Ended ranked mode", e));
                    }

                    memory.detach();
                    break;
                }
            }
            sleep(Duration::from_millis(16));
        }
    }
}

fn validator_thread(rx: Receiver<GameState>, client: ClientManager) {
    let mut validator = Validator::new(client.clone_state());

    for state in rx {
        match validator.validate(state) {
            Ok(validity) => match validity {
                Validity::Invalid(reason) => {
                    update_status(format!("Invalid game state: {}", reason));
                    break;
                }
                Validity::MatchFinished(result) => {
                    if client.send_result(&result).is_err() {
                        update_status("Could not send match to the server.".to_string());
                        update_status("Exiting ranked mode...".to_string());
                        break;
                    }
                    update_status(format!("Finished = {:?}", result));
                }
                _ => {}
            },
            Err(e) => {
                update_status(format!("Validator error: {:?}", e));
                break;
            }
        }
    }
}

fn host_or_join_input() -> Option<ClientState> {
    println!("Commands: 'host' to generate a code, 'join <code>' to join, 'stop' to cancel");
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)
            .expect("Could not read input.");
        match input.trim() {
            "host" => {
                let host_state = ClientState::hosting();
                if let Some(session) = host_state.session() {
                    println!("Your code: {}", session);
                }
                break Some(host_state);
            }
            cmd if cmd.starts_with("join ") => {
                let session = cmd[5..].trim().to_string();
                println!("Joined ranked match with code: {}", session);
                break Some(ClientState::JoinedRanked(session.clone()));
            }
            "stop" => {
                break None;
            }
            _ => println!("Unknown command."),
        }
    }
}

fn update_status(msg: String) {
    // print!("{}", " ".repeat(100));
    println!("\r[Status: {msg}]");
    // std::io::stdout().flush().unwrap();
}
