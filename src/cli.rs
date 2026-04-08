#![allow(dead_code)]
#![allow(unused)]
use std::io::{Write, stdout};

use crate::{
    client::{ClientManager, ClientState, http::ClientError},
    exit_app,
};

// pub fn host_or_join(client: &ClientManager) {
//     println!("This system uses codes/keywords to pair players.");
//     println!(
//         "Commands:\n - 'host <code>' or 'host' to generate a random code\n - 'join <code>' to join\n - 'stop' to cancel"
//     );
//     loop {
//         match host_or_join_input() {
//             Some(s) => {
//                 if let Err(e) = client.update_state(s) {
//                     update_status(format!("Client Error: {}", e));
//                     return;
//                 }
//                 match client.send_queue_request() {
//                     Ok(queue) => {
//                         update_status(format!(
//                             "Playing ranked against {}",
//                             queue.opponent_discord_username
//                         ));
//                         break;
//                     }
//                     Err(ClientError::ServerError(409)) => {
//                         update_status("Session code already in use, try again.".to_string());
//                     }
//                     Err(ClientError::ServerError(404)) => {
//                         update_status("Host not found.".to_string());
//                     }
//                     Err(e) => {
//                         eprintln!("Client Error: {}", e);
//                         exit_app(1);
//                     }
//                 }
//             }
//             None => std::process::exit(0),
//         }
//     }
// }

fn host_or_join_input() -> Option<ClientState> {
    loop {
        print!("> ");
        stdout().flush().expect("Could not flush stdout");
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .expect("Could not read input.");
        match input.trim() {
            "start" => { todo!() },
            "stop" => {
                break None;
            }
            _ => println!("Unknown command."),
        }
    }
}

pub fn prompt_token() -> String {
    println!(
        "Token not found or is invalid. Please insert a valid token below or update your config file:"
    );
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Could not read input");
    input.trim().to_string()
}

pub fn update_status(msg: String) {
    // print!("{}", " ".repeat(100));
    println!("\r[Status: {msg}]");
    // std::io::stdout().flush().unwrap();
}
