use std::sync::mpsc::Sender;

use crate::{cli::{self, update_status}, client::{ClientManager, http::ClientError}, config::Config, exit_app, log};

pub fn create_client(config: Config, log_tx: &Sender<String>) -> ClientManager {
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
