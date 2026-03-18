mod client;
mod config;
mod game;
mod memory;
mod validation;
use std::{
    io::Error,
    sync::mpsc::{Receiver, Sender, channel},
    thread::sleep,
    time::Duration,
};

use crate::client::{ClientManager, ClientState};
use crate::game::state::GameState;
use crate::memory::MemoryManager;
use crate::validation::{Validator, Validity};

fn main() -> Result<(), Error> {
    println!("=== ATLAS OBSERVER ===\n");

    let state = match host_or_join_input() {
        Some(s) => s,
        None => std::process::exit(1),
    };

    let client = match ClientManager::new(state) {
        Ok(cli) => cli,
        Err(e) => {
            println!("Could not start client: {}", e);
            exit_app();
            std::process::exit(1);
        }
    };

    let (tx, rx) = channel();

    let m = std::thread::spawn(move || memory_thread(tx));
    let v = std::thread::spawn(move || validator_thread(rx, client));

    if m.join().is_err() {
        update_status("Something went wront in the memory thread".to_string());
    }
    if v.join().is_err() {
        update_status("Something went wront in the validator thread".to_string());
    }

    exit_app();
    Ok(())
}

fn exit_app() {
    println!("Press Enter to exit.");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Could not read input.");
}

fn memory_thread(tx: Sender<GameState>) {
    let mut memory = MemoryManager::new();

    'outer: loop {
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
                    break 'outer;
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
                    update_status(format!("Match ended: {}", &result));
                    match client.send_result(&result) {
                        Ok(res) => match res.text() {
                            Ok(msg) => update_status(msg),
                            Err(_) => update_status(
                                "Match sent, but couldn't read response message".to_string(),
                            ),
                        },
                        Err(e) => {
                            update_status(format!("{:?}", e));
                            break;
                        }
                    }

                    if client.send_result(&result).is_err() {
                        update_status("Could not send match to the server.".to_string());
                        update_status("Exiting ranked mode...".to_string());
                        break;
                    }
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
        std::io::stdin()
            .read_line(&mut input)
            .expect("Could not read input.");
        match input.trim() {
            "host" => {
                let host_state = ClientState::hosting();
                if let Some(session) = host_state.session() {
                    match cli_clipboard::set_contents(session.to_owned()) {
                        Ok(_) => update_status("Code copied to clipboard".to_string()),
                        Err(_) => update_status("Could not set code to clipboard".to_string()),
                    }
                    println!("Your code: {}", session);
                }
                break Some(host_state);
            }
            cmd if cmd.starts_with("join ") => {
                let session = cmd[5..].trim().to_string().to_uppercase();
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
