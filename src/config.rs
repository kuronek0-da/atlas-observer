use std::io::ErrorKind;

use log::{error, info};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{cli, exit_app};

#[derive(Debug, Error, Clone)]
pub enum ConfigError {
    #[error("could not find config file")]
    FileNotFound,
    #[error("could not parse configution: {0}")]
    ParseError(String),
    #[error("could not write config file: {0}")]
    WriteError(String),
    #[error("could not read config file: {0}")]
    ReadError(String),
}

const SERVER_URL: &str = match option_env!("SERVER_URL") {
    Some(s) => s,
    None => "https://atlas-index-server-production.up.railway.app",
};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(skip_serializing)]
    pub server_url: String,
    pub token: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ConfigFile {
    pub token: String,
}

impl Config {
    pub fn load() -> Self {
        let mut config = Config::from_file("config.toml").unwrap_or_else(|e| match e {
            ConfigError::FileNotFound => {
                error!("Config Error: {}", e);
                eprintln!("Configuration not found.");
                eprintln!("Trying to creating a new config file...");
                match Config::new().save() {
                    Ok(_) => eprintln!("File created successfully, restart Atlas."),
                    Err(e) => {
                        error!("Config Error: {}", e);
                        eprintln!("Action failed, try giving this app writing permission.");
                    }
                }
                exit_app(1)
            }
            ConfigError::ParseError(ref field) => {
                error!("Config Error: {}", e);
                eprintln!("'{}' field in config.toml has incorrect formatting or is invalid.", field);
                exit_app(1)
            }
            _ => {
                error!("Config Error: {}", e);
                eprintln!("Failed to load config, try giving this app reading/writting permission");
                exit_app(1)
            }
        });

        if config.token.is_empty() {
            error!("Empty token detected, prompting for new token");
            config.token = cli::prompt_token();
            if let Err(e) = config.save() {
                error!("Config Error: {}", e);
                eprintln!("Failed to save updated config.");
                exit_app(1)
            }
            info!("Token updated");
            eprintln!("Token updated, please restart this application.");
            exit_app(1)
        }

        config
    }

    #[allow(dead_code)]
    pub fn load_test() -> Result<Self, ConfigError> {
        Self::from_file("test_config.toml")
    }

    pub fn new() -> Self {
        Config {
            server_url: SERVER_URL.to_string(),
            token: String::new(),
        }
    }

    /// Writes the current config to the toml file
    pub fn save(&self) -> Result<(), ConfigError> {
        let content = toml::to_string(&self).map_err(|e| {
            ConfigError::WriteError(format!("unable to get struct as string, {}", e))
        })?;
        std::fs::write("config.toml", content)
            .map_err(|_| ConfigError::WriteError("config.toml".to_string()))
    }

    fn from_file(path: &str) -> Result<Self, ConfigError> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => Ok(c),
            Err(e) => match e.kind() {
                ErrorKind::NotFound => Err(ConfigError::FileNotFound),
                error_kind => Err(ConfigError::ReadError(format!("{}", error_kind))),
            },
        }?;
        let conf: ConfigFile =
            toml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))?;
        Ok(Config {
            server_url: SERVER_URL.to_string(),
            token: conf.token,
        })
    }
}
