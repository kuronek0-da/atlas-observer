use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ValidationResponse {
    pub discord_username: String,
}

#[derive(Debug, Serialize)]
pub struct QueueRequest {
    pub session_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct MatchedResponse {
    pub session_id: String,
    pub opponent_discord_username: String,
}
