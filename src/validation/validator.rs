use std::sync::{Arc, Mutex};

use crate::{
    client::state::ClientState,
    game::state::GameState,
    memory::addresses::{ClientMode, GameMode},
    validation::result::{MatchResult, StateError},
};

pub struct Validator {
    matchstate: MatchState,
    session_id: String,
}

impl Validator {
    pub fn new(client_state: Arc<Mutex<ClientState>>) -> Result<Self, StateError> {
        let session_id = match client_state.lock().unwrap().session() {
            Some(s) => s.clone(),
            None => return Err(StateError::SessionNotFound),
        };
        Ok(Validator {
            matchstate: MatchState::default(),
            session_id,
        })
    }

    fn validate_client_mode(&self, client_mode: &ClientMode) -> Result<(), String> {
        if !matches!(client_mode, ClientMode::Host | ClientMode::Client) {
            return Err(format!("invalid client mode: {:?}", client_mode));
        }
        Ok(())
    }

    pub fn validate(&mut self, state: GameState) -> Result<Validity, StateError> {
        if let Err(msg) = self.validate_client_mode(state.client_mode()) {
            return Ok(Validity::Invalid(msg));
        }

        self.update_matchstate(state.game_mode());

        if let GameState::InGame { .. } = &state {
            if matches!(self.matchstate, MatchState::MatchFinished) {
                return self.handle_match_finished(state);
            }
        }

        match &self.matchstate {
            MatchState::Invalid(reason) => Ok(Validity::Invalid(reason.clone())),
            _ => Ok(Validity::Valid),
        }
    }

    pub fn get_session(&self) -> String {
        self.session_id.clone()
    }

    fn handle_match_finished(&mut self, state: GameState) -> Result<Validity, StateError> {
        if let GameState::InGame {
            local_player,
            client_mode,
            players,
            timers,
            ..
        } = state
        {
            let session_ids = self.get_session();
            let result = MatchResult::new(
                session_ids,
                client_mode,
                local_player as u8,
                players,
                timers,
            )?;

            return Ok(Validity::MatchFinished(result));
        }
        Err(StateError::MatchResultError("not a valid state".into())) // Should be unreachable
    }

    fn update_matchstate(&mut self, game_mode: &GameMode) {
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
