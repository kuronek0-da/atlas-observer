/*
Took from Constants.hpp in CCCaster
Only those i might find relevant
#define CC_GAME_MODE_ADDR           ( ( uint32_t * ) 0x54EEE8 ) // Current game mode, constants below

#define CC_GAME_MODE_CHARA_SELECT   ( 20 )
#define CC_GAME_MODE_IN_GAME        ( 1 )
#define CC_GAME_MODE_RETRY          ( 5 )
#define CC_GAME_MODE_REPLAY         ( 26 )
#define CC_GAME_MODE_LOADING        ( 8 )

#define CC_P1_CHARACTER_ADDR        ( ( uint32_t * ) 0x74D8FC )
#define CC_P1_MOON_SELECTOR_ADDR    ( ( uint32_t * ) 0x74D900 )
#define CC_P2_CHARACTER_ADDR        ( ( uint32_t * ) 0x74D920 )
#define CC_P2_MOON_SELECTOR_ADDR    ( ( uint32_t * ) 0x74D924 )

#define CC_P1_GAME_POINT_FLAG_ADDR  ( ( uint32_t * ) 0x559548 ) // P1 game point flag
#define CC_P2_GAME_POINT_FLAG_ADDR  ( ( uint32_t * ) 0x55954C ) // P2 game point flag
#define CC_P1_WINS_ADDR             ( ( uint32_t * ) 0x559550 ) // P1 number of wins
#define CC_P2_WINS_ADDR             ( ( uint32_t * ) 0x559580 ) // P2 number of wins

#define CC_WORLD_TIMER_ADDR         ( ( uint32_t * ) 0x55D1D4 ) // Frame step timer, always counting up
#define CC_ROUND_TIMER_ADDR         ( ( uint32_t * ) 0x562A3C ) // Counts down from 4752, may stop
#define CC_REAL_TIMER_ADDR          ( ( uint32_t * ) 0x562A40 ) // Counts up from 0 after round start
*/

/* === READ FROM MBAA === */

pub const WORLD_TIMER_ADDR: usize = 0x55D1D4;
pub const ROUND_TIMER_ADDR: usize = 0x562A3C;
pub const REAL_TIMER_ADDR: usize = 0x562A40;

use num_enum::TryFromPrimitive;

pub const GAME_MODE_ADDR: usize = 0x54EEE8;

#[derive(Debug, Clone, TryFromPrimitive, PartialEq, Eq)]
#[repr(u32)]
pub enum GameMode {
    Unknown = 0,
    InGame = 1,
    Retry = 5,
    Loading = 8,
    CharSelect = 20,
    ReplayMenu = 26,
}

// ADD LATER: Rounds to win, Timer initial value (defaults to 90 secs)

pub const P1_CHARACTER_ADDR: usize = 0x74D8FC;
pub const P1_MOON_SELECTOR_ADDR: usize = 0x74D900;
pub const P1_GAME_POINT_FLAG_ADDR: usize = 0x559548;
pub const P1_WINS_ADDR: usize = 0x559550;

pub const P2_CHARACTER_ADDR: usize = 0x74D920;
pub const P2_MOON_SELECTOR_ADDR: usize = 0x74D924;
pub const P2_WINS_ADDR: usize = 0x559580;
pub const P2_GAME_POINT_FLAG_ADDR: usize = 0x55954C;

/* === READ FROM CCCASTER === */

pub const CLIENT_MODE_OFFSET: usize = 0x38208C;

/// ClientMode enum from caster, set to 1 on default
#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum ClientMode {
    Unknown,
    Host,
    Client,
    SpectateNetplay,
    SpectateBroadcast,
    Broadcast,
    Offline,
}

pub const LOCAL_PLAYER_OFFSET: usize = 0x3822F0;

#[derive(Debug, PartialEq, Eq)]
pub enum LocalPlayer {
    Unknown,
    P1,
    P2,
}
