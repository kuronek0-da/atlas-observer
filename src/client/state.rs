use std::fmt;

use rand::RngExt;

#[derive(Debug, Clone)]
pub enum ClientState {
    /// Not in ranked
    Idle,
    /// Hosting, not in match yet
    HostingRanked(String),
    /// Joined, not in match yet
    JoinedRanked(String),
}

impl ClientState {
    pub fn hosting() -> ClientState {
        ClientState::HostingRanked(ClientState::generate_match_code())
    }

    pub fn session(&self) -> Option<&str> {
        match self {
            ClientState::HostingRanked(s) => Some(&s),
            ClientState::JoinedRanked(s) => Some(&s),
            _ => None,
        }
    }

    fn generate_match_code() -> String {
        let mut rng = rand::rng();
        (0..6)
            .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
            .collect::<String>()
            .to_uppercase()
    }
}

impl std::fmt::Display for ClientState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}
