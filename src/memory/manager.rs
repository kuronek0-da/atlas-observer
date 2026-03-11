use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use windows::{
    Win32::Foundation::{HANDLE, CloseHandle},
    Win32::System::{
        Threading::{PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    }
};

use super::addresses::*;
use super::reader::*;
use crate::game::{character::{Character, Moon}, 
    state::{self, GameState, GameTimers, Players}};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Process '{0}' was not found.")]
    ProcessNotFound(String),
    #[error("could not open MBAA")]
    OpenProcessFailed,
    #[error("error trying to read memory: {0}")]
    ReadFailed(String),
    #[error("failed to parse {0}: unexpected value {1}")]
    ParseFailed(&'static str, u32)
}

pub struct MemoryManager {
    sys: System,
    mb_process: Option<HANDLE>,
    caster_process: Option<HANDLE>,
    caster_base: Option<usize>
}

impl MemoryManager {
    const MBAA: &str = "MBAA.exe";
    const CASTER: &str = "cccaster.v3.1.exe";

    pub fn new() -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything())
        );
        MemoryManager {
            sys,
            mb_process: None,
            caster_process: None,
            caster_base: None
        }
    }

    // Attaches to MBAA.exe
    pub fn attach(&mut self) -> Result<(), MemoryError> {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        let mb_pid = self.sys.processes_by_exact_name(Self::MBAA.as_ref()).next()
            .ok_or(MemoryError::ProcessNotFound(Self::MBAA.to_string()))?;
        let caster_pid = self.sys.processes_by_exact_name(Self::CASTER.as_ref()).next()
            .ok_or(MemoryError::ProcessNotFound(Self::CASTER.to_string()))?;

        self.mb_process = Some(open_process(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            mb_pid.pid().as_u32(),
        )?);
        self.caster_process = Some(open_process(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            caster_pid.pid().as_u32()
        )?);
        if let Some(process) = self.caster_process  {
            self.caster_base = Some(get_module_base(process, Self::CASTER)?);
        } else {
            return Err(MemoryError::ProcessNotFound(Self::CASTER.to_string()))
        };

        Ok(())
    }

    pub fn detach(&mut self) {
        if let Some(handle) = self.mb_process.take() {
            unsafe { let _ = CloseHandle(handle); }
        }
    }

    pub fn is_running(&mut self) -> bool {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.sys.processes_by_exact_name(Self::MBAA.as_ref()).next().is_some()
    }

    pub fn poll(&self) -> Result<GameState, MemoryError> {
        let mb_process: HANDLE = self.mb_process.ok_or(MemoryError::ProcessNotFound(Self::MBAA.to_string()))?;
        let caster_process = self.caster_process.ok_or(MemoryError::ProcessNotFound(Self::CASTER.to_string()))?;

        self.read_game_state(mb_process, caster_process)
    }

    fn read_game_state(&self, mb_process: HANDLE, caster_process: HANDLE) -> Result<GameState, MemoryError> {
        let game_mode = self.read_mode(mb_process)?;
        match game_mode {
            GameMode::InGame => {
                // CCCaster
                let local_player = self.read_local_player(caster_process)?;
                let client_mode = self.read_client_mode(caster_process)?;
                // MBAA
                let timers = self.read_timers(mb_process)?;
                let players = self.read_players(mb_process)?;

                return Ok(GameState::InGame { local_player, client_mode, game_mode, timers, players })
            },
            _ => Ok(GameState::NotInGame { game_mode, client_mode: self.read_client_mode(caster_process)? })
        }
    }

    fn read_local_player(&self, caster_process: HANDLE) -> Result<LocalPlayer, MemoryError> {
        let base = self.caster_base.ok_or(MemoryError::ReadFailed("caster base addr not found.".to_string()))?;
        let local_player_addr = base + LOCAL_PLAYER_OFFSET;
        let local_player_value = read_u8!(caster_process, local_player_addr)?;

        match local_player_value {
            0 => Ok(LocalPlayer::Unknown),
            1 => Ok(LocalPlayer::P1),
            2 => Ok(LocalPlayer::P2),
            e => Err(MemoryError::ParseFailed("local_player", e as u32))
        }
    }

    fn read_client_mode(&self, caster_process: HANDLE) -> Result<ClientMode, MemoryError> {
        let base = self.caster_base.ok_or(MemoryError::ReadFailed("caster base addr not found.".to_string()))?;
        let client_mode_addr = base + CLIENT_MODE_OFFSET;
        let client_value = read_u8!(caster_process, client_mode_addr)?;
        
        ClientMode::try_from(client_value).map_err(|_| MemoryError::ParseFailed("client mode", client_value as u32))
    }

    fn read_mode(&self, process: HANDLE) -> Result<GameMode, MemoryError> {
        let mode_value = read_u32!(process, GAME_MODE_ADDR)?;
        match GameMode::try_from(mode_value) {
            Ok(mode) => Ok(mode),
            _ => Ok(GameMode::Unknown)
        }
    }

    fn read_timers(&self, process: HANDLE) -> Result<GameTimers, MemoryError> {
        Ok(GameTimers::new(
        read_u32!(process, WORLD_TIMER_ADDR)?,
        read_u32!(process, ROUND_TIMER_ADDR)?,
        read_u32!(process, REAL_TIMER_ADDR)?
        ))
    }

    fn read_players(&self, process: HANDLE) -> Result<Players, MemoryError> {
        let p1 = self.get_player(
            read_u32!(process, P1_CHARACTER_ADDR)?,
            read_u32!(process, P1_MOON_SELECTOR_ADDR)?,
            read_u32!(process, P1_WINS_ADDR)?
        )?;
        let p2 = self.get_player(
            read_u32!(process, P2_CHARACTER_ADDR)?,
            read_u32!(process, P2_MOON_SELECTOR_ADDR)?,
            read_u32!(process, P2_WINS_ADDR)?
        )?;

        Ok(Players::new(p1, p2))
    }

    fn get_player(&self, char_u32: u32, moon_u32: u32, score: u32) -> Result<state::Player, MemoryError> {
        let char = Character::try_from(char_u32).map_err(|_| MemoryError::ParseFailed("character", char_u32))?;
        let moon = Moon::try_from(moon_u32).map_err(|_| MemoryError::ParseFailed("moon", moon_u32))?;
        Ok(state::Player { char, moon, score })
    }
}

impl Drop for MemoryManager {
    fn drop(&mut self) {
        self.detach();
    }
}