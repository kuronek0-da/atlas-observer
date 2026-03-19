use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ValidationResponse {
    pub discord_username: String,
}
