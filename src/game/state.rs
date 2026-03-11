use super::character::{Character, Moon};
use crate::memory::addresses::{ClientMode, GameMode, LocalPlayer};

#[derive(Debug)]
pub enum GameState {
    InGame { local_player: LocalPlayer, client_mode: ClientMode, game_mode: GameMode, timers: GameTimers, players: Players },
    NotInGame { game_mode: GameMode, client_mode: ClientMode }
}

#[derive(Debug)]
pub struct GameTimers {
    world_timer: u32,
    round_timer: u32,
    real_timer: u32
}

impl GameTimers {
    pub fn new(world_timer: u32, round_timer: u32, real_timer: u32) -> Self {
        GameTimers { world_timer, round_timer, real_timer }
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

#[derive(Debug, Clone)]
pub struct Players {
    pub p1: Player,
    pub p2: Player,
}

impl Players {
    pub fn new(p1: Player, p2: Player) -> Self {
        Players { p1, p2 }
    }
}

#[derive(Debug, Clone)]
pub struct Player {
    pub char: Character,
    pub score: u32,
    pub moon: Moon,
}