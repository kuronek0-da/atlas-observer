mod cli;
mod client;
mod config;
mod game;
mod memory;
mod ui;
mod validation;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread::sleep,
    time::Duration,
};

use crate::memory::MemoryManager;
use crate::validation::{Validator, Validity};
use crate::{
    cli::update_status,
    client::ClientState,
    config::ConfigError,
    ui::{AppCommand, AppUI},
};
use crate::{client::ClientManager, config::Config};
use crate::{client::http::ClientError, game::state::GameState};

fn main() {
    loop {
        let (log_tx, log_rx) = channel::<String>();
        let (cmd_tx, cmd_rx) = channel::<AppCommand>();

        log(
            "Ranked mode is only supported on cccaster v3.1.008.".to_string(),
            &log_tx,
        );

        let mut config = load_config();
        let client = create_client(config, &log_tx);
        let client_state_clone = client.clone_state();

        let mut app = AppUI::new(log_rx, cmd_tx, client_state_clone);

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
    let mut config_clone = config.clone();
    let client = ClientManager::new(config).unwrap_or_else(|e| {
        eprintln!("{}", e);
        exit_app(1)
    });

    log("Cheking the server...".to_string(), log_tx);
    match client.validate_token() {
        Ok(vr) => log(format!("Logged as {}", vr.discord_username), log_tx),
        Err(ClientError::AuthorizationError) => {
            let token = cli::prompt_token();
            config_clone.token = token;
            if let Err(e) = config_clone.save() {
                update_status(format!("Config Error: {}", e));
            }
            update_status("Token updated. Restart Atlas.".to_string());
            exit_app(1)
        }
        Err(e) => {
            eprintln!("Client Error: {}", e);
            exit_app(1)
        }
    }
    client
}

fn load_config() -> Config {
    let mut config = Config::load().unwrap_or_else(|e| match e {
        ConfigError::FileNotFound => {
            eprintln!("Config Error: {}", e);
            eprintln!("Trying to creating a new config file...");
            match Config::new().save() {
                Ok(_) => eprintln!("File created successfully, restart Atlas."),
                Err(e) => {
                    eprintln!("Config Error: {}", e);
                    eprintln!("Try running this app as admin.");
                }
            }
            exit_app(1)
        }
        _ => {
            eprintln!("Config Error: {}", e);
            exit_app(1)
        }
    });

    if config.token.is_empty() {
        config.token = cli::prompt_token();
        if let Err(e) = config.save() {
            eprintln!("Config Error: {}", e);
            exit_app(1)
        }
        eprintln!("Token updated, please restart this application.");
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
        GameState::InGame { .. } if !*was_in_game => {
            *was_in_game = true;
            log("Match running...".to_string(), log_tx);
        }
        _ => {}
    }
}

fn validator_thread(rx: Receiver<GameState>, client: ClientManager, log_tx: &Sender<String>) {
    let mut validator = Validator::new(client.clone_state());
    let mut is_playing = false;

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
                Validity::Valid(session) => {
                    if !is_playing {
                        is_playing = true;
                        if let Err(e) = client.update_state(ClientState::PlayingRanked(session)) {
                            log(format!("Client Errr: {}", e), log_tx);
                        }
                    }
                }
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
