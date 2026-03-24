use std::sync::{Arc, Mutex};

use crate::{
    client::state::ClientState,
    game::state::GameState,
    memory::addresses::{ClientMode, GameMode, LocalPlayer},
    validation::result::{MatchResult, StateError},
};

pub struct Validator {
    client_state: Arc<Mutex<ClientState>>,
    matchstate: MatchState,
}

impl Validator {
    pub fn new(client_state: Arc<Mutex<ClientState>>) -> Self {
        Validator {
            client_state,
            matchstate: MatchState::default(),
        }
    }

    pub fn validate(&mut self, state: GameState) -> Result<Validity, StateError> {
        match state {
            // During in-game and retry menu
            GameState::InGame {
                local_player,
                client_mode,
                game_mode,
                timers,
                players,
            } => {
                if !matches!(client_mode, ClientMode::Host | ClientMode::Client) {
                    return Ok(Validity::Invalid(format!(
                        "invalid client mode: {:?}",
                        client_mode
                    )));
                }
                
                self.update_matchstate(&game_mode);
                match &self.matchstate {
                    MatchState::MatchFinished => {
                        let session_id = match self.client_state.lock() {
                            Ok(state) => match state.session() {
                                Some(session) => Ok(String::from(session))?,
                                None => Err(StateError::MatchResultError(
                                    "could not get session id / code".to_string(),
                                ))?,
                            },
                            Err(_) => Err(StateError::MatchResultError(
                                "could not get client state".to_string(),
                            ))?,
                        };

                        let result = MatchResult::new(session_id, client_mode, local_player as u8, players, timers)?;
                        return Ok(Validity::MatchFinished(result));
                    }
                    MatchState::Invalid(reason) => Ok(Validity::Invalid(reason.clone())),
                    _ => Ok(Validity::Valid),
                }
            }
            // during char select or transition states
            GameState::NotInGame {
                game_mode,
                client_mode,
                host_position
            } => {
                if !matches!(client_mode, ClientMode::Host | ClientMode::Client) {
                    return Ok(Validity::Invalid(format!(
                        "invalid client mode: {:?}",
                        client_mode
                    )));
                }

                self.update_matchstate(&game_mode);
                match &self.matchstate {
                    MatchState::Invalid(reason) => Ok(Validity::Invalid(reason.clone())),
                    _ => Ok(Validity::Valid),
                }
            }
        }
    }

    pub fn update_matchstate(&mut self, game_mode: &GameMode) {
        if !self.matchstate.is_valid_before(game_mode) {
            self.matchstate = MatchState::invalid_mode(game_mode);
            return;
        }

        match game_mode {
            GameMode::CharSelect => self.matchstate = MatchState::WaitingInCharSelect,
            GameMode::InGame => self.matchstate = MatchState::InGame,
            GameMode::Retry => {
                if self.matchstate == MatchState::InGame {
                    self.matchstate = MatchState::MatchFinished;
                    return;
                }
                self.matchstate = MatchState::RetryMenu;
            }
            GameMode::ReplayMenu => self.matchstate = MatchState::invalid_mode(game_mode),
            _ => (),
        }
    }
}

#[derive(Debug)]
pub enum Validity {
    Valid,
    Invalid(String),
    MatchFinished(MatchResult),
}

#[derive(Debug, Eq, PartialEq, Default)]
pub enum MatchState {
    #[default]
    /// Before char select and after retry menu
    Idle,
    WaitingInCharSelect,
    InGame,
    RetryMenu,
    /// Will only happen once every match, then go back to retry
    MatchFinished,
    Invalid(String),
}

impl MatchState {
    pub fn invalid_mode(game_mode: &GameMode) -> MatchState {
        MatchState::Invalid(format!(
            "invalid match state detected before {:?}",
            game_mode
        ))
    }

    fn is_valid_before(&self, game_mode: &GameMode) -> bool {
        match game_mode {
            GameMode::CharSelect => matches!(
                self,
                MatchState::Idle
                    | MatchState::RetryMenu
                    | MatchState::InGame
                    | MatchState::WaitingInCharSelect
            ),
            GameMode::InGame => matches!(
                self,
                MatchState::RetryMenu | MatchState::InGame | MatchState::WaitingInCharSelect
            ),
            GameMode::Retry => matches!(
                self,
                MatchState::InGame | MatchState::RetryMenu | MatchState::MatchFinished
            ),
            _ => true,
        }
    }
}
