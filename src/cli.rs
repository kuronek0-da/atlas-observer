use crate::client::ClientState;

pub fn host_or_join_input() -> Option<ClientState> {
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
                    update_status(format!("Your code: {}", session));
                    match cli_clipboard::set_contents(session.to_owned()) {
                        Ok(_) => update_status("Code copied to clipboard".to_string()),
                        Err(_) => update_status("Could not set code to clipboard".to_string()),
                    }
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

pub fn prompt_token() -> String {
    println!("Token not found or is invalid. Please insert a valid token below or update your config file:");
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
