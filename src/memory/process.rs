use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use windows::{
    Win32::Foundation::{CloseHandle, HANDLE},
    Win32::System::Threading::{PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
};

use super::addresses::*;
use super::reader::*;
use crate::game::{
    GameChar, Moon,
    state::{self, GameState, GameTimers, Player},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Module '{0}' not found in process")]
    ModuleNotFound(String),
    #[error("VirtualQueryEx failed at address {0}")]
    QueryFailed(String),
    #[error("Process '{0}' was not found.")]
    ProcessNotFound(String),
    #[error("Could not read valid values from CCCaster.")]
    InvalidCCCaster,
    #[error("Invalid values from MBAA, player not on netplay")]
    InvalidMBAA,
    #[error("Multiple '{0}' processes detected")]
    MultipleProcessesError(String),
    #[error("could not open process with pid: {0}")]
    OpenProcessFailed(u32),
    #[error("error trying to read memory: {0}")]
    ReadFailed(String),
    #[error("failed to parse {0}: unexpected value {1}")]
    ParseFailed(&'static str, u32),
}

pub const MBAA: &str = "MBAA.exe";
pub const CASTER: &str = "cccaster.v3.1.exe";

pub struct MemoryManager {
    sys: System,
    mb_process: Option<HANDLE>,
    caster_process: Option<HANDLE>,
    caster_base: Option<usize>,
}

impl MemoryManager {
    pub fn new() -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
        );
        MemoryManager {
            sys,
            mb_process: None,
            caster_process: None,
            caster_base: None,
        }
    }

    /// Attaches to MBAA.exe
    pub fn attach(&mut self) -> Result<(), MemoryError> {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        if let Err(e) = self.find_valid_mbaa() {
            self.detach();
            return Err(e);
        }

        if let Err(e) = self.find_valid_caster() {
            self.detach();
            return Err(e);
        }
        Ok(())
    }

    fn find_single_pid(&self, name: &str) -> Result<Pid, MemoryError> {
        let mut iter = self.sys.processes_by_exact_name(name.as_ref());
        let p = iter
            .next()
            .ok_or(MemoryError::ProcessNotFound(name.to_string()))?;
        if iter.next().is_some() {
            return Err(MemoryError::MultipleProcessesError(name.to_string()));
        }
        Ok(p.pid())
    }

    fn find_valid_mbaa(&mut self) -> Result<(), MemoryError> {
        let mb_pid = self.find_single_pid(MBAA)?;
        let mb_process = open_process(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            mb_pid.as_u32(),
        )?;

        self.mb_process = Some(open_process(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            mb_pid.as_u32(),
        )?);
        Ok(())
    }

    /// Mostly for wine users
    fn find_valid_caster(&mut self) -> Result<(), MemoryError> {
        let iter = self.sys.processes_by_exact_name(CASTER.as_ref());
        let mut any_found = false;
        for process in iter {
            let Ok(caster_process) = open_process(
                PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                false,
                process.pid().as_u32(),
            ) else {
                continue;
            };

            let Ok(caster_base) = get_module_base(caster_process, CASTER) else {
                continue;
            };

            any_found = true;
            let client_mode_addr = caster_base + CLIENT_MODE_OFFSET;
            let Ok(client_value) = read_u8!(caster_process, client_mode_addr) else {
                continue;
            };

            if let Ok(cm) = ClientMode::try_from(client_value) {
                if matches!(cm, ClientMode::Unknown) {
                    continue;
                }

                self.caster_process = Some(caster_process);
                self.caster_base = Some(caster_base);
                return Ok(());
            }
        }

        if !any_found {
            return Err(MemoryError::ProcessNotFound(CASTER.to_string()));
        }

        Err(MemoryError::InvalidCCCaster)
    }

    /// Close process handle
    pub fn detach(&mut self) {
        if let Some(handle) = self.mb_process.take() {
            unsafe {
                let _ = CloseHandle(handle);
            }
        }
        if let Some(handle) = self.caster_process.take() {
            unsafe {
                let _ = CloseHandle(handle);
            }
        }
    }

    pub fn is_melty_running(&mut self) -> bool {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.sys
            .processes_by_exact_name(MBAA.as_ref())
            .next()
            .is_some()
    }

    pub fn poll_session_ids(&self) -> Result<Vec<String>, MemoryError> {
        let process = self
            .caster_process
            .ok_or(MemoryError::ProcessNotFound(CASTER.to_string()))?;
        Ok(scan_for_session_ids(process))
    }

    /// Starts reading memory and return a GameState
    pub fn poll_game_state(&self) -> Result<GameState, MemoryError> {
        let mb_process: HANDLE = self
            .mb_process
            .ok_or(MemoryError::ProcessNotFound(MBAA.to_string()))?;
        let caster_process = self
            .caster_process
            .ok_or(MemoryError::ProcessNotFound(CASTER.to_string()))?;

        self.read_game_state(mb_process, caster_process)
    }

    fn read_game_state(
        &self,
        mb_process: HANDLE,
        caster_process: HANDLE,
    ) -> Result<GameState, MemoryError> {
        let game_mode = self.read_mode(mb_process)?;

        let local_player = self.read_local_player(caster_process)?;
        let client_mode = self.read_client_mode(caster_process)?;

        match game_mode {
            GameMode::InGame | GameMode::Retry => {
                // CCCaster
                // MBAA
                let timers = self.read_timers(mb_process)?;
                let players = self.read_players(mb_process)?;

                return Ok(GameState::InGame {
                    local_player,
                    client_mode,
                    game_mode,
                    timers,
                    players,
                });
            }
            _ => Ok(GameState::NotInGame {
                game_mode,
                client_mode,
                host_position: local_player as u8,
            }),
        }
    }

    fn read_local_player(&self, caster_process: HANDLE) -> Result<LocalPlayer, MemoryError> {
        let base = self.caster_base.ok_or(MemoryError::ReadFailed(
            "caster base addr not found.".to_string(),
        ))?;
        let local_player_addr = base + LOCAL_PLAYER_OFFSET;
        let local_player_value = read_u8!(caster_process, local_player_addr)?;

        match local_player_value {
            0 => Ok(LocalPlayer::Unknown),
            1 => Ok(LocalPlayer::P1),
            2 => Ok(LocalPlayer::P2),
            e => Err(MemoryError::ParseFailed("local_player", e as u32)),
        }
    }

    fn read_client_mode(&self, caster_process: HANDLE) -> Result<ClientMode, MemoryError> {
        let base = self.caster_base.ok_or(MemoryError::ReadFailed(
            "caster base addr not found.".to_string(),
        ))?;
        let client_mode_addr = base + CLIENT_MODE_OFFSET;
        let client_value = read_u8!(caster_process, client_mode_addr)?;

        ClientMode::try_from(client_value)
            .map_err(|_| MemoryError::ParseFailed("client mode", client_value as u32))
    }

    fn read_mode(&self, process: HANDLE) -> Result<GameMode, MemoryError> {
        let mode_value = read_u32!(process, GAME_MODE_ADDR)?;
        match GameMode::try_from(mode_value) {
            Ok(mode) => Ok(mode),
            _ => Ok(GameMode::Unknown),
        }
    }

    fn read_timers(&self, process: HANDLE) -> Result<GameTimers, MemoryError> {
        Ok(GameTimers::new(
            read_u32!(process, WORLD_TIMER_ADDR)?,
            read_u32!(process, ROUND_TIMER_ADDR)?,
            read_u32!(process, REAL_TIMER_ADDR)?,
        ))
    }

    fn read_players(&self, process: HANDLE) -> Result<[Player; 2], MemoryError> {
        let p1 = self.parse_player(
            read_u32!(process, P1_CHARACTER_ADDR)?,
            read_u32!(process, P1_MOON_SELECTOR_ADDR)?,
            read_u32!(process, P1_WINS_ADDR)?,
        )?;
        let p2 = self.parse_player(
            read_u32!(process, P2_CHARACTER_ADDR)?,
            read_u32!(process, P2_MOON_SELECTOR_ADDR)?,
            read_u32!(process, P2_WINS_ADDR)?,
        )?;

        Ok([p1, p2])
    }

    fn parse_player(
        &self,
        char_u32: u32,
        moon_u32: u32,
        score: u32,
    ) -> Result<state::Player, MemoryError> {
        let char = GameChar::try_from(char_u32)
            .map_err(|_| MemoryError::ParseFailed("character", char_u32))?;
        let moon =
            Moon::try_from(moon_u32).map_err(|_| MemoryError::ParseFailed("moon", moon_u32))?;
        Ok(state::Player {
            character: char,
            moon,
            score,
        })
    }
}

impl Drop for MemoryManager {
    fn drop(&mut self) {
        self.detach();
    }
}

#[cfg(test)]
mod test {
    use std::time::{Duration, Instant};

    use crate::memory::MemoryManager;

    #[test]
    fn test_session_id_polling() {
        let mut m = MemoryManager::new();

        println!("Waiting for CCCaster and MBAA");
        while m.attach().is_err() {
            std::thread::sleep(Duration::from_secs(1));
        }
        println!("Attached.");
        let state = m.poll_game_state().expect("Failed to read game memory");
        println!("Game State: {:?}", state);

        let start = Instant::now();
        let timeout = Duration::from_secs(15);

        let r = loop {
            let results = m.poll_session_ids().expect("Failed to read memory.");

            if !results.is_empty() {
                break results;
            }

            if start.elapsed() >= timeout {
                assert!(false, "Session ID polling timed out. No ids found");
                break Vec::new();
            }

            println!("No session id found yet, retrying...");
            std::thread::sleep(Duration::from_secs(1));
        };
        if !r.is_empty() {
            for id in r.iter() {
                println!("ID: {}", id);
            }
        }
    }
}
