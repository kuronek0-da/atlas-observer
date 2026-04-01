use std::sync::mpsc::{Sender, channel};

use crate::{cli::update_status, config::Config, ui::{AppCommand, AppUI}};

mod cli;
mod client;
mod config;
mod game;
mod memory;
mod runner;
mod ui;
mod validation;
mod setup;

fn main() {
    loop {
        let (log_tx, log_rx) = channel::<String>();
        let (cmd_tx, cmd_rx) = channel::<AppCommand>();

        log(
            "Ranked mode is only supported on cccaster v3.1.008.".to_string(),
            &log_tx,
        );

        let config = Config::load();
        let client = setup::create_client(config, &log_tx);
        let client_state_clone = client.clone_state();

        let mut app = AppUI::new(log_rx, cmd_tx, client_state_clone);

        std::thread::spawn(move || runner::run(client, log_tx, cmd_rx));
        if let Err(e) = ratatui::run(|terminal| app.run(terminal)) {
            update_status(format!("UI Error: {}", e));
        }

        if app.exit {
            break;
        }
    }
}

pub fn exit_app(code: i32) -> ! {
    println!("Press Enter to exit.");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Could not read input.");
    std::process::exit(code)
}

pub fn log(msg: String, log_tx: &Sender<String>) {
    _ = log_tx.send(msg).ok();
}
