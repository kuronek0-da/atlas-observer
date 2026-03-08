mod game;
mod memory;
mod validation;

use std::{
    io::{self, Write, stdin},
    sync::{
        Arc, atomic::{AtomicBool, Ordering}, mpsc::{Receiver, Sender, channel}
    },
    thread::sleep,
    time::Duration,
};
use crate::game::state::GameState;
use crate::memory::addresses::GameMode;
use crate::memory::manager::MemoryManager;
use crate::validation::validator::{Validator, Validity};

fn main() {
    println!("=== ATLAS OBSERVER ===\n");
    let ranked_active = Arc::new(AtomicBool::new(false));

    let ranked_active_memory = Arc::clone(&ranked_active);
    let (tx, rx) = channel();

    std::thread::spawn(move || memory_thread(tx, ranked_active_memory));
    let handle = std::thread::spawn(move || validator_thread(rx));
    
    input_thread(ranked_active);
}

fn input_thread(ranked_active: Arc<AtomicBool>) {
    println!("[RANKED MODE COMMANDS]");
    println!("- 's' -> start\n- 'e' -> end");
    loop {
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();
        println!("");

        // Ranked not started
        if !ranked_active.load(Ordering::Relaxed) {
            match input.to_lowercase().trim() {
                "s" => {
                    println!("\rRanking mode started.");
                    ranked_active.store(true, Ordering::Relaxed);
                },
                _ => {}
            }
        } else {
            match input.to_lowercase().trim() {
                "e" => {
                    print!("\rEnding ranked mode...\n");
                    ranked_active.store(false, Ordering::Relaxed);
                    sleep(Duration::from_millis(500));
                    println!("\rRanking mode ended.");
                },
                _ => {}
            }
        }
        sleep(Duration::from_millis(500));
    }
}

fn memory_thread(tx: Sender<GameState>, ranked_active: Arc<AtomicBool>) {
    let mut memory = MemoryManager::new();

    'outer: loop {
        // Wait for ranked mode to be enabled
        while !ranked_active.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(500));
        }

        println!("\rMBAA session detected. Restart MBAA.exe.");
        // io::stdout().flush().unwrap();
        while memory.is_running() {
            if !ranked_active.load(Ordering::Relaxed) {
                continue 'outer;
            }
            sleep(Duration::from_secs(2));
        }

        // print!("\r{}", " ".repeat(100));
        println!("\rWaiting for MBAA.exe...");
        while let Err(_) = memory.attach() {
            if !ranked_active.load(Ordering::Relaxed) {
                continue 'outer;
            }
            sleep(Duration::from_secs(2));
        }

        print!("\r{}", " ".repeat(100));
        println!("\rAttached to MBAA.exe");
        loop {

            match memory.poll() {
                Ok(state) => {
                    if !ranked_active.load(Ordering::Relaxed) {
                        break;
                    }
                    if tx.send(state).is_err() {
                        eprintln!("Receiver dropped, shutting down memory thread.");
                        return;
                    }
                },
                Err(e) => {
                    if !memory.is_running() {
                        println!("Game closed.");
                        ranked_active.store(false, Ordering::Relaxed);
                    } else {
                        eprintln!("Lost connection: {:?}", e);
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
                    println!("\rInvalid game state: {}", reason);
                    break;
                },
                Validity::MatchFinished(result) => {
                    println!("\rMatch finished: {:?}", result);
                },
                _ => {}
            },
            Err(e) => {
                eprintln!("\rValidator error: {:?}", e);
                break;
            }
        }
    }
}