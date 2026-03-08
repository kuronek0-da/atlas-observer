use sysinfo::{Process, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use windows::{
    Win32::Foundation::{CloseHandle, HANDLE},
    Win32::System::Diagnostics::Debug::ReadProcessMemory,
    Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
};

use super::addresses as addr;
use addr::*;
use crate::game::{character::{Character, Moon}, 
    state::{self, GameState, GameTimers, Players}};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("MBAA was not found.")]
    ProcessNotFound,
    #[error("could not open MBAA")]
    OpenProcessFailed,
    #[error("error trying to read memory: {0}")]
    ReadFailed(String),
    #[error("failed to parse {0}: unexpected value {1}")]
    ParseFailed(&'static str, u32)
}

pub struct MemoryManager {
    sys: System,
    process: Option<HANDLE>,
}

macro_rules! read_u32 {
    ($process:expr, $addr:expr) => {
        read_u32($process, $addr)
            .map_err(|_| MemoryError::ReadFailed(stringify!($addr).to_string()))
    };
}
fn read_u32(process: HANDLE, address: usize) -> Result<u32, ()> {
    unsafe {
        let mut buffer: u32 = 0;
        let mut bytes_read: usize = 0;

        let result = ReadProcessMemory(
            process,
            address as *const _,
            &mut buffer as *mut _ as *mut _,
            std::mem::size_of::<u32>(),
            Some(&mut bytes_read),
        );

        if result.is_ok() && bytes_read == size_of::<u32>() {
            Ok(buffer)
        } else {
            Err(())
        }
    }
}

impl MemoryManager {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything())
        );
        MemoryManager {
            sys,
            process: None
        }
    }

    // Attaches to MBAA.exe
    pub fn attach(&mut self) -> Result<(), MemoryError> {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        if let Some(p) = self.sys.processes_by_exact_name("MBAA.exe".as_ref()).next() {
            let pid = p.pid().as_u32();

            unsafe {
                self.process = OpenProcess(
                    PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                    false,
                    pid,
                ).ok();

                if self.process == None {
                    return Err(MemoryError::OpenProcessFailed)
                }
                Ok(())
            }
        } else {
            Err(MemoryError::ProcessNotFound)
        }
    }

    pub fn detach(&mut self) {
        if let Some(handle) = self.process.take() {
            unsafe { let _ = CloseHandle(handle); }
        }
    }

    pub fn is_running(&mut self) -> bool {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.sys.processes_by_exact_name("MBAA.exe".as_ref()).next().is_some()
    }

    pub fn poll(&self) -> Result<GameState, MemoryError> {
        let process: HANDLE = self.process.ok_or(MemoryError::ProcessNotFound)?;
        let mode = self.read_mode(process)?;
        match mode {
            GameMode::InGame | GameMode::Retry => self.read_ingame_state(process, mode),
            _ => Ok(GameState::NotInGame { mode })
        }       
    }

    fn read_mode(&self, process: HANDLE) -> Result<GameMode, MemoryError> {
        let mode_value = read_u32!(process, GAME_MODE_ADDR)?;
        match GameMode::try_from(mode_value) {
            Ok(mode) => Ok(mode),
            _ => Ok(GameMode::Unknown)
        }
    }

    fn read_ingame_state(&self, process: HANDLE, mode: GameMode) -> Result<GameState, MemoryError> {
        let timers = self.read_timers(process)?;
        let players = self.read_players(process)?;
        Ok(GameState::InGame { mode, timers, players })
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