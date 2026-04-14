mod cli;
mod client;
mod config;
mod game;
mod memory;
mod runner;
mod setup;
mod ui;
mod validation;

use std::{
    fs::OpenOptions,
    sync::mpsc::{Sender, channel},
};

use log::{LevelFilter, error, info};
use simplelog::WriteLogger;

use crate::{
    client::ClientState, config::Config, ui::{AppCommand, AppUI}
};

fn main() {
    logger_init();
    let now = chrono::Local::now();
    info!(
        "=== Atlas Observer v{} started at {} ===",
        env!("CARGO_PKG_VERSION"),
        now
    );

    let (log_tx, log_rx) = channel::<String>();
    let (cmd_tx, cmd_rx) = channel::<AppCommand>();
    let (state_tx, state_rx) = channel::<ClientState>();

    log(
        "[Warning] Ranked mode is only supported on cccaster v3.1.008.".to_string(),
        &log_tx,
    );

    let config = Config::load();
    let client = setup::create_client(config, log_tx.clone());

    let mut app = AppUI::new(log_rx, cmd_tx, state_rx);

    std::thread::spawn(move || runner::run(client, log_tx, cmd_rx, state_tx));

    loop {
        if let Err(e) = ratatui::run(|terminal| app.run(terminal)) {
            error!("UI Error: {}", e);
            eprintln!("UI Error: {}", e);
        }

        if app.exit {
            break;
        }
    }
    info!("Atlas closed.\n");
}

fn logger_init() {
    let _ = WriteLogger::init(
        LevelFilter::Info,
        simplelog::Config::default(),
        OpenOptions::new()
            .create(true)
            .append(true)
            .open("atlas-observer.log")
            .unwrap(),
    );
}

pub fn exit_app(code: i32) -> ! {
    println!("Press Enter to exit.");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Could not read input.");
    info!("Atlas closed.\n");
    std::process::exit(code)
}

pub fn log(msg: String, log_tx: &Sender<String>) {
    _ = log_tx.send(msg).ok();
}
