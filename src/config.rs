use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not find config file")]
    FileNotFound,
    #[error("could not parse config file: {0}")]
    ParseError(String),
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server_url: String,
    pub player_id: u32,
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        Self::from_file("config.toml")
    }

    pub fn load_test() -> Result<Self, ConfigError> {
        Self::from_file("test_config.toml")
    }

    fn from_file(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|_| ConfigError::FileNotFound)?;
        toml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))
    }
}

