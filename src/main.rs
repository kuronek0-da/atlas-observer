mod cli;
mod client;
mod config;
mod game;
mod memory;
mod validation;
use std::{
    sync::mpsc::{Receiver, Sender, channel},
    thread::sleep,
    time::Duration,
};

use crate::{client::models::MatchedResponse, memory::MemoryManager};
use crate::validation::{Validator, Validity};
use crate::{cli::update_status, config::ConfigError};
use crate::{client::ClientManager, config::Config};
use crate::{ game::state::GameState};

fn main() {
    println!("=== ATLAS OBSERVER ===\n");

    let config = load_config().unwrap_or_else(|e| {
        eprintln!("{}", e);
        exit_app(1)
    });

    let client = create_client(config);
    host_or_join(&client);

    let (tx, rx) = channel();
    let m = std::thread::spawn(move || memory_thread(tx));
    let v = std::thread::spawn(move || validator_thread(rx, client));

    if m.join().is_err() {
        update_status("Something went wront in the memory thread".to_string());
    }
    if v.join().is_err() {
        update_status("Something went wront in the validator thread".to_string());
    }

    exit_app(0)
}

fn create_client(config: Config) -> ClientManager {
    let client = ClientManager::new(config).unwrap_or_else(|e| {
        eprintln!("{}", e);
        exit_app(1)
    });

    update_status("Cheking the server...".to_string());
    match client.validate_token() {
        Ok(vr) => update_status(format!("Logged as {}", vr.discord_username)),
        Err(e) => {
            eprintln!("Client Error: {}", e);
            exit_app(1)
        }
    }
    client
}

fn load_config() -> Result<Config, ConfigError> {
    let mut config = Config::load().or_else(|e| match e {
        ConfigError::ParseError(_) => Err(e),
        _ => {
            Config::new().save()?;
            Err(e)
        }
    })?;

    if config.token.is_empty() {
        config.token = cli::prompt_token();
        config.save()?;
        update_status("Token updated, please restart this application.".to_string());
        exit_app(1)
    }

    Ok(config)
}

fn host_or_join(client: &ClientManager) {
    let mut is_connected = false;

    while !is_connected {
        match cli::host_or_join_input() {
            Some(s) => {
                client.update_state(s);
                match client.send_queue_request() {
                    Ok(queue) => {
                            update_status(format!("Playing ranked against {}", queue.opponent_discord_username));
                            is_connected = true;
                    },
                    Err(e) => {
                        eprintln!("Client Error: {}", e);
                        exit_app(1);
                    }
                }
            }
            None => std::process::exit(0),
        };
    }
}

fn exit_app(code: i32) -> ! {
    println!("Press Enter to exit.");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Could not read input.");
    std::process::exit(code)
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
                    update_status("Sending match to the server...".to_string());
                    let client = client.clone();
                    std::thread::spawn(move || match client.send_result(&result) {
                        Ok(res) => match res.text() {
                            Ok(msg) => update_status(msg),
                            Err(_) => update_status(
                                "Match sent, but couldn't read response message".to_string(),
                            ),
                        },
                        Err(e) => {
                            update_status(format!("Could not send match to the server: {:?}", e));
                        }
                    });
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
