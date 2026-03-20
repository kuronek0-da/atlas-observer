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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub server_url: String,
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
            server_url: String::new(),
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
        let conf: Config =
            toml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))?;
        if conf.server_url.is_empty() {
            return Err(ConfigError::ParseError("server_url is empty.".to_string()));
        }
        Ok(conf)
    }
}
