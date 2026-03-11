use windows::{
    Win32::Foundation::{HANDLE, HMODULE},
    Win32::System::{
        Diagnostics::Debug::ReadProcessMemory,
        Threading::{OpenProcess, PROCESS_ACCESS_RIGHTS},
        ProcessStatus::{EnumProcessModules, GetModuleFileNameExA},
    }
};

use super::manager::MemoryError;

macro_rules! read_u32 {
    ($process:expr, $addr:expr) => {
        read_u32($process, $addr)
            .map_err(|_| MemoryError::ReadFailed(stringify!($addr).to_string()))
    };
}
pub fn read_u32(process: HANDLE, address: usize) -> Result<u32, ()> {
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

macro_rules! read_u8 {
    ($process:expr, $addr:expr) => {
        read_u8($process, $addr)
            .map_err(|_| MemoryError::ReadFailed(stringify!($addr).to_string()))
    };
}
pub fn read_u8(process: HANDLE, address: usize) -> Result<u8, ()> {
    unsafe {
        let mut buffer: u8 = 0;
        let mut bytes_read: usize = 0;

        let result = ReadProcessMemory(
            process,
            address as *const _,
            &mut buffer as *mut _ as *mut _,
            std::mem::size_of::<u8>(),
            Some(&mut bytes_read),
        );

        if result.is_ok() && bytes_read == size_of::<u8>() {
            Ok(buffer)
        } else {
            Err(())
        }
    }
}

pub fn get_module_base(process: HANDLE, target: &str) -> Result<usize, MemoryError> {
    let mut modules = [HMODULE::default(); 1024];
    let mut needed: u32 = 0;

    unsafe {
        _ = EnumProcessModules(
            process,
            modules.as_mut_ptr(),
            (modules.len() * size_of::<HMODULE>()) as u32,
            &mut needed,
        )
        .map_err(|_| MemoryError::ReadFailed(target.to_string()))?;

        // getting the needed count in bytes
        let count = needed as usize / size_of::<HMODULE>();

        for module in &modules[..count] {
            let mut name = [0u8; 260]; // MAX_PATH
            let len = GetModuleFileNameExA(Some(process), Some(*module), &mut name);

            if len == 0 {
                continue;
            };

            let name_str = std::str::from_utf8(&name[..len as usize]).unwrap_or("");

            if name_str.to_lowercase().contains(&target) {
                return Ok(module.0 as usize);
            }
        }
    }
    Err(MemoryError::ProcessNotFound(target.to_string()))
}

pub fn open_process(
    dwdesiredaccess: PROCESS_ACCESS_RIGHTS,
    binherithandle: bool,
    dwprocessid: u32
) -> Result<HANDLE, MemoryError> {
    unsafe {
        OpenProcess(dwdesiredaccess, binherithandle, dwprocessid)
            .map_err(|_| MemoryError::OpenProcessFailed)
    }
}

