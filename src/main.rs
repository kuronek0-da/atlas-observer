mod cli;
mod client;
mod config;
mod game;
mod memory;
mod ui;
mod validation;
use std::{
    sync::mpsc::{Receiver, Sender, channel},
    thread::sleep,
    time::Duration,
};

use crate::{
    cli::update_status,
    client::ClientState,
    config::ConfigError,
    ui::{AppCommand, AppUI},
};
use crate::{client::ClientManager, config::Config};
use crate::{client::http::ClientError, game::state::GameState};
use crate::{client::models::MatchedResponse, memory::MemoryManager};
use crate::{
    memory::addresses::{ClientMode, GameMode, LocalPlayer},
    validation::{Validator, Validity},
};

fn main() {
    println!("=== ATLAS OBSERVER ===\n");

    println!(
        "[Warning]\nRanked mode is only supported on cccaster v3.1.008.\nOther builds may produce incorrect results.\n"
    );

    loop {
        let (log_tx, log_rx) = channel::<String>();
        let (cmd_tx, cmd_rx) = channel::<AppCommand>();

        let config = load_config(&log_tx);
        let client = create_client(config, &log_tx);

        let mut app = AppUI::new(log_rx, cmd_tx);

        std::thread::spawn(move || runner(client, log_tx, cmd_rx));
        if let Err(e) = ratatui::run(|terminal| app.run(terminal)) {
            update_status(format!("UI Error: {}", e));
        }

        if app.exit {
            break;
        }
    }
}

fn runner(client: ClientManager, log_tx: Sender<String>, cmd_rx: Receiver<AppCommand>) {
    loop {
        log_tx.send("Waiting for command.".to_string()).ok();

        let client_state;
        match cmd_rx.recv().expect("Sender dropped.") {
            AppCommand::Host(state) => client_state = state,
            AppCommand::Join(state) => client_state = state,
            _ => continue,
        }

        let waiting_msg = match &client_state {
            ClientState::HostingRanked(code) => format!("Hosting with code '{}'", code),
            ClientState::JoinedRanked(code) => format!("Trying to join '{}'...", code),
            _ => continue,
        };

        if let Err(e) = client.update_state(client_state) {
            log(format!("Client Error: {}", e), &log_tx);
            continue;
        }

        // End of loop

        log(waiting_msg, &log_tx);
        // create a thread if cmd was join or host 
        match client.send_queue_request() {
            Ok(res) => {
                log(
                    format!("Player connected: {}", res.opponent_discord_username),
                    &log_tx,
                );
            }
            Err(ClientError::ServerError(408)) => log("Request expired".to_string(), &log_tx),
            Err(ClientError::NotFoundError) => log("Match not found".to_string(), &log_tx),
            Err(e) => log(format!("Client Error: {}", e), &log_tx),
        }
        // loop to see if message was returned, continue if not
    }

    let log_tx_memory = log_tx.clone();
    let log_tx_validator = log_tx.clone();

    let (tx, rx) = channel();
    let m = std::thread::spawn(move || memory_thread(tx, &log_tx_memory));
    let v = std::thread::spawn(move || validator_thread(rx, client, &log_tx_validator));

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

fn create_client(config: Config, log_tx: &Sender<String>) -> ClientManager {
    let client = ClientManager::new(config).unwrap_or_else(|e| {
        log(format!("{}", e), log_tx);
        exit_app(1)
    });

    log("Cheking the server...".to_string(), log_tx);
    match client.validate_token() {
        Ok(vr) => log(format!("Logged as {}", vr.discord_username), log_tx),
        Err(e) => {
            log(format!("Client Error: {}", e), log_tx);
            exit_app(1)
        }
    }
    client
}

fn load_config(log_tx: &Sender<String>) -> Config {
    let mut config = Config::load().unwrap_or_else(|e| match e {
        ConfigError::FileNotFound => {
            log(format!("Config Error: {}", e), log_tx);
            log(
                "Trying to creating a new config file...".to_string(),
                log_tx,
            );
            match Config::new().save() {
                Ok(_) => log(
                    "File created successfully, restart Atlas.".to_string(),
                    log_tx,
                ),
                Err(e) => {
                    log(format!("Config Error: {}", e), log_tx);
                    log("Try running this app as admin.".to_string(), log_tx);
                }
            }
            exit_app(1)
        }
        _ => {
            log(format!("Config Error: {}", e), log_tx);
            exit_app(1)
        }
    });

    if config.token.is_empty() {
        config.token = cli::prompt_token();
        if let Err(e) = config.save() {
            log(format!("Config Error: {}", e), log_tx);
            exit_app(1)
        }
        log(
            "Token updated, please restart this application.".to_string(),
            log_tx,
        );
        exit_app(1)
    }

    config
}

pub fn exit_app(code: i32) -> ! {
    println!("Press Enter to exit.");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Could not read input.");
    std::process::exit(code)
}

fn memory_thread(tx: Sender<GameState>, log_tx: &Sender<String>) {
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
                    if tx.send(state).is_err() {
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
        GameState::InGame {
            local_player,
            players,
            client_mode,
            ..
        } if !*was_in_game => {
            *was_in_game = true;
            log("Match running...".to_string(), log_tx);
        }
        _ => {}
    }
}

fn validator_thread(rx: Receiver<GameState>, client: ClientManager, log_tx: &Sender<String>) {
    let mut validator = Validator::new(client.clone_state());

    for state in rx {
        match validator.validate(state) {
            Ok(validity) => match validity {
                Validity::Invalid(reason) => {
                    log(format!("Invalid game state: {}", reason), log_tx);
                    break;
                }
                Validity::MatchFinished(result) => {
                    log(format!("Match ended: {}", &result), log_tx);
                    log("Sending match to the server...".to_string(), log_tx);
                    let client = client.clone();
                    let log_tx_client = log_tx.clone();
                    std::thread::spawn(move || match client.send_result(&result) {
                        Ok(res) => match res.text() {
                            Ok(msg) => log(msg, &log_tx_client),
                            Err(_) => log(
                                "Match sent, but couldn't read response message".to_string(),
                                &log_tx_client,
                            ),
                        },
                        Err(e) => {
                            log(
                                format!("Could not send match to the server: {}", e),
                                &log_tx_client,
                            );
                        }
                    });
                }
                _ => {}
            },
            Err(e) => {
                log(format!("Validator error: {:?}", e), log_tx);
                break;
            }
        }
    }
}

pub fn log(msg: String, log_tx: &Sender<String>) {
    _ = log_tx.send(msg).ok();
}
