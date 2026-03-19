mod cli;
mod client;
mod config;
mod game;
mod memory;
mod validation;
use core::panic;
use std::{
    error::Error,
    sync::mpsc::{Receiver, Sender, channel},
    thread::sleep,
    time::Duration,
};

use crate::game::state::GameState;
use crate::memory::MemoryManager;
use crate::validation::{Validator, Validity};
use crate::{cli::update_status, config::ConfigError};
use crate::{
    client::{ClientManager, ClientState},
    config::Config,
};

fn main() {
    println!("=== ATLAS OBSERVER ===\n");

    match Config::load().as_mut() {
        Ok(conf) => {
            if conf.token.is_empty() {
                conf.token = cli::prompt_token();
                if let Err(e) = conf.save() {
                    eprintln!("{}", e);
                    exit_app();
                }
            }
        }
        Err(e) => match e {
            ConfigError::ParseError(msg) => {
                eprintln!("Config Error: {}", e);
                exit_app();
            }
            _ => {
                eprintln!("Config Error: {}\nTrying to create a config file...", e);
                let new_conf = Config::new();
                match new_conf.save() {
                    Ok(_) => {
                        println!("Config file created successfully. Please set your config there.")
                    }
                    Err(e) => eprintln!("Config Error: {}", e),
                }
                exit_app();
            }
        },
    }

    let client = match ClientManager::new(ClientState::Idle) {
        Ok(cli) => cli,
        Err(e) => {
            println!("Could not start client: {}", e);
            exit_app();
            panic!("Unreachable line");
        }
    };

    update_status("Cheking the server...".to_string());
    match client.validate_token() {
        Ok(vr) => update_status(format!("Logged as {}", vr.discord_username)),
        Err(e) => {
            eprint!("Client Error: {}", e);
            exit_app();
        }
    }

    match cli::host_or_join_input() {
        Some(s) => client.update_state(s),
        None => std::process::exit(1),
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
}

fn exit_app() {
    println!("Press Enter to exit.");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Could not read input.");
    std::process::exit(1);
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
                        update_status(format!("Game closed. Waiting for next session..."));
                    } else {
                        update_status(format!("Lost connection: {:?}", e));
                    }
                    memory.detach();
                    break; // back to outer loop, not exit
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
                            update_status(format!("Could not send match to the server: {:?}", e));
                            break;
                        }
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
