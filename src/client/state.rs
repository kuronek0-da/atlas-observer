use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ClientState {
    /// Not in ranked
    Idle,
    /// Queuing ranked
    WaitingForOpponent,
    /// Currently playing ranked
    PlayingRanked(String),
    Exit,
}

impl ClientState {
    // pub fn hosting() -> ClientState {
    //     ClientState::HostingRanked(ClientState::generate_match_code())
    // }

    pub fn session(&self) -> Option<&String> {
        match self {
            ClientState::PlayingRanked(id) => Some(&id),
            _ => None,
        }
    }

    // fn generate_match_code() -> String {
    //     let mut rng = rand::rng();
    //     (0..6)
    //         .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
    //         .collect::<String>()
    //         .to_uppercase()
    // }
}

impl std::fmt::Display for ClientState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}
