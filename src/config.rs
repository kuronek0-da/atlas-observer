use std::io::ErrorKind;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum ConfigError {
    #[error("could not find config file")]
    FileNotFound,
    #[error("could not parse config file: {0}")]
    ParseError(String),
    #[error("could not write config file: {0}")]
    WriteError(String),
    #[error("could not read config file: {0}")]
    ReadError(String),
}

const SERVER_URL: Option<&str> = option_env!("SERVER_URL");

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
    pub fn load() -> Result<Self, ConfigError> {
        Self::from_file("config.toml")
    }

    pub fn load_test() -> Result<Self, ConfigError> {
        Self::from_file("test_config.toml")
    }

    pub fn new() -> Self {
        Config {
            server_url: SERVER_URL.unwrap_or("http://localhost:8080").to_string(),
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
            server_url: SERVER_URL.unwrap_or("http://localhost:8080").to_string(),
            token: conf.token
        })
    }
}
