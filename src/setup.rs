use std::sync::mpsc::Sender;
use log::{error, info};

use crate::{
    cli::{self, update_status},
    client::{ClientManager, http::ClientError},
    config::Config,
    exit_app, log,
};

pub fn create_client(config: Config, log_tx: &Sender<String>) -> ClientManager {
    let mut config_clone = config.clone();
    let client = ClientManager::new(config).unwrap_or_else(|e| {
        error!("Client Error: {}", e);
        eprintln!("Client Error: {}", e);
        exit_app(1)
    });

    info!("Validating token...");
    log("Cheking the server...".to_string(), log_tx);
    match client.validate_token() {
        Ok(vr) => {
            info!("Authenticated as {}", vr.discord_username);
            log(format!("Logged as {}", vr.discord_username), log_tx);
        },
        Err(ClientError::AuthorizationError) => {
            error!("Invalid token, prompting user for new token.");
            let token = cli::prompt_token();
            config_clone.token = token;
            if let Err(e) = config_clone.save() {
                error!("Failed to save config: {}", e);
                eprintln!("Failed to save config. Check logs for details.");
            }
            eprintln!("Token updated, restart Atlas");
            exit_app(1)
        }
        Err(e) => {
            error!("Failed to validate token: {}", e);
            eprintln!("Failed to validate token.");
            exit_app(1)
        }
    }
    client
}
