use crate::{game::state::GameState, memory::addresses::{ClientMode, GameMode}, validation::result::{MatchResult, StateError}};

pub struct Validator {
    session_id: String,
    matchstate: MatchState,
}

impl Validator {
    pub fn new(session_id: String) -> Self {
        Validator {
            session_id,
            matchstate: MatchState::default(),
        }
    }

    pub fn validate(&mut self, state: GameState) -> Result<Validity, StateError> {
        match state {
            GameState::InGame { local_player, client_mode, game_mode, timers, players } => {
                if !matches!(client_mode, ClientMode::Host | ClientMode::Client) {
                    return Ok(Validity::Invalid("not in netplay".to_string()));
                }

                self.update_matchstate(&game_mode);
                match &self.matchstate {
                    MatchState::MatchFinished => {
                        let result = MatchResult::new(players, timers, self.session_id.clone())?;
                        return Ok(Validity::MatchFinished(result));
                    },
                    MatchState::Invalid(reason) => Ok(Validity::Invalid(reason.clone())),
                    _ => Ok(Validity::Valid)
                }
            },
            GameState::NotInGame { game_mode, client_mode } => {
                if !matches!(client_mode, ClientMode::Host | ClientMode::Client) {
                    return Ok(Validity::Invalid("not in netplay".to_string()));
                }
                
                self.update_matchstate(&game_mode);
                match &self.matchstate {
                    MatchState::Invalid(reason) => Ok(Validity::Invalid(reason.clone())),
                    _ => Ok(Validity::Valid)
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
            },
            GameMode::ReplayMenu => self.matchstate = MatchState::invalid_mode(game_mode),
            _ => ()
        }
    }
}

#[derive(Debug)]
pub enum Validity {
    Valid,
    Invalid(String),
    MatchFinished(MatchResult)
}

#[derive(Debug, Eq, PartialEq, Default)]
pub enum MatchState {
    #[default]
    Idle, // Before char select and after retry menu
    WaitingInCharSelect,
    InGame,
    RetryMenu,
    MatchFinished, // Will only happen once every match, then go back to retry
    Invalid(String),
}

impl MatchState {
    pub fn invalid_mode(game_mode: &GameMode) -> MatchState {
        MatchState::Invalid(format!("invalid match state detected before {:?}", game_mode))
    }

    fn is_valid_before(&self, game_mode: &GameMode) -> bool {
        match game_mode {
            GameMode::CharSelect => matches!(self,
                MatchState::Idle | MatchState::RetryMenu |
                MatchState::InGame | MatchState::WaitingInCharSelect
            ),
            GameMode::InGame => matches!(self,
                MatchState::RetryMenu | MatchState::InGame | MatchState::WaitingInCharSelect
            ),
            GameMode::Retry => matches!(self,
                MatchState::InGame | MatchState::RetryMenu | MatchState::MatchFinished
            ),
            _ => true
        }
    }

}