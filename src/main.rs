mod game;
mod memory;
mod validation;
use std::array::repeat;
use std::io::{self, Write};
use std::{
    io::stdin,
    sync::{
        Arc, atomic::{AtomicBool, Ordering}, mpsc::{Receiver, Sender, channel}
    },
    thread::sleep,
    time::Duration,
};
use crate::{game::state::GameState, memory::addresses::GameMode};
use crate::memory::manager::MemoryManager;
use crate::validation::validator::{Validator, Validity};

fn main() {
    println!("=== ATLAS OBSERVER ===\n");
    let mut ranked_active = false;

    let mut will_continue = true;

    input_thread(ranked_active);

    while will_continue {
        let (tx, rx) = channel();

        let m = std::thread::spawn(move || memory_thread(tx));
        let v = std::thread::spawn(move || validator_thread(rx));
        
        m.join();
        v.join();

        print!("Continue ranked? (y/n, default: y) > ");
        std::io::stdout().flush().unwrap();
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();

        match input.to_lowercase().trim() {
            "n" => {
                will_continue = false;
            },
            _ => {}
        }
    }
}

// TODO: control ranked mode
fn input_thread(mut ranked_active: bool) {
    println!("[RANKED MODE COMMANDS]");
    println!("- 's' -> start\n");
    // Ranked not started
    if !ranked_active {
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();

        match input.to_lowercase().trim() {
            "s" => {
                println!("\rRanking mode started.");
                ranked_active = true;
            },
            _ => {}
        }
    }
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
                },
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

fn validator_thread(rx: Receiver<GameState>) {
    let mut validator = Validator::new();

    for state in rx {
        match validator.validate(state) {
            Ok(validity) => match validity {
                Validity::Invalid(reason) => {
                    update_status(format!("Invalid game state: {}", reason));
                    break;
                },
                Validity::MatchFinished(result) => {
                    update_status(format!("Finished = {:?}", result));
                },
                _ => {}
            },
            Err(e) => {
                update_status(format!("Validator error: {:?}", e));
                break;
            }
        }
    }
}

fn update_status(msg: String) {
    // print!("{}", " ".repeat(100));
    println!("\r[Status: {msg}]");
    // std::io::stdout().flush().unwrap();
}