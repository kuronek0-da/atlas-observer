use log::{error, info};
use std::sync::mpsc::Sender;

use crate::{
    cli::{self},
    client::{ClientManager, http::ClientError},
    config::Config,
    exit_app, log,
};

pub fn create_client(mut config: Config, log_tx: Sender<String>) -> ClientManager {
    log("Cheking the server...".to_string(), &log_tx);
    loop {
        let client = ClientManager::new(config.clone()).unwrap_or_else(|e| {
            error!("Client Error: {}", e);
            eprintln!("Client Error: {}", e);
            exit_app(1)
        });

        info!("Validating token...");

        match client.validate_token() {
            Ok(vr) => {
                info!("Authenticated as {}", vr.discord_username);
                log(format!("Logged as {}", vr.discord_username), &log_tx);
                break client;
            }
            Err(ClientError::AuthorizationError) => {
                error!("Invalid token, prompting user for new token.");
                dbg!("{}", &config.token);
                let token = cli::prompt_token();
                config.token = token;
                if let Err(e) = config.save() {
                    error!("Failed to save config: {}", e);
                    eprintln!("Failed to save config. Check logs for details.");
                }
                eprintln!("Token has been updated");
            }
            Err(e) => {
                error!("Failed to validate token: {}", e);
                eprintln!("No response from the server.");
                exit_app(1);
            }
        }
    }
}
