use serde::{Deserialize, Serialize};

use crate::game::{GameChar, Moon};
use crate::memory::addresses::{ClientMode, GameMode, LocalPlayer};

#[derive(Debug)]
pub enum GameState {
    InGame {
        local_player: LocalPlayer,
        client_mode: ClientMode,
        game_mode: GameMode,
        timers: GameTimers,
        players: [Player; 2],
    },
    NotInGame {
        game_mode: GameMode,
        client_mode: ClientMode,
        host_position: u8,
    },
}

impl GameState {
    pub fn client_mode(&self) -> &ClientMode {
        match self {
            Self::InGame { client_mode, .. } => client_mode,
            Self::NotInGame { client_mode, .. } => client_mode,
        }
    }

    pub fn game_mode(&self) -> &GameMode {
        match self {
            Self::InGame { game_mode, .. } => game_mode,
            Self::NotInGame { game_mode, .. } => game_mode,
        }
    }
}

// Currently clashing with MatchTimer, i might delete or use it for validation later
#[derive(Debug)]
pub struct GameTimers {
    world_timer: u32,
    round_timer: u32,
    real_timer: u32,
}

impl GameTimers {
    pub fn new(world_timer: u32, round_timer: u32, real_timer: u32) -> Self {
        GameTimers {
            world_timer,
            round_timer,
            real_timer,
        }
    }
    pub fn world_timer(&self) -> u32 {
        self.world_timer
    }
    pub fn round_timer(&self) -> u32 {
        self.round_timer
    }
    pub fn real_timer(&self) -> u32 {
        self.real_timer
    }
}

// Remove this struct later, make p1 and p2 a field in gamestate
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct Players {
//     pub p1: Player,
//     pub p2: Player,
// }
//
// impl Players {
//     pub fn new(p1: Player, p2: Player) -> Self {
//         Players { p1, p2 }
//     }
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub character: GameChar,
    pub score: u32,
    pub moon: Moon,
}
