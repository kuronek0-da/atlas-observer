use std::vec;

use windows::{
    Win32::Foundation::{HANDLE, HMODULE},
    Win32::System::{
        Diagnostics::Debug::ReadProcessMemory,
        Memory::{
            MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
            PAGE_READONLY, PAGE_READWRITE, VirtualQueryEx,
        },
        ProcessStatus::{EnumProcessModules, GetModuleFileNameExA},
        Threading::{OpenProcess, PROCESS_ACCESS_RIGHTS},
    },
};

use super::process::MemoryError;

macro_rules! read_u32 {
    ($process:expr, $addr:expr) => {
        read_u32($process, $addr)
            .map_err(|_| MemoryError::ReadFailed(stringify!($addr).to_string()))
    };
}
pub fn read_u32(process: HANDLE, address: usize) -> Result<u32, ()> {
    // TODO: remove and use read_memory(...)
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
        read_u8($process, $addr).map_err(|_| MemoryError::ReadFailed(stringify!($addr).to_string()))
    };
}
pub fn read_u8(process: HANDLE, address: usize) -> Result<u8, ()> {
    // TODO: remove and use read_memory(...)
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

pub fn scan_for_session_ids(process: HANDLE) -> Vec<String> {
    let mut results = Vec::new();
    let mut address: usize = 0;
    let mut mbi = MEMORY_BASIC_INFORMATION::default();
    loop {
        if query_region(process, address, &mut mbi).is_err() {
            break;
        }

        // Always advance address before any early continues
        address = (mbi.BaseAddress as usize).wrapping_add(mbi.RegionSize);
        if address == 0 {
            break;
        }

        let is_readable = mbi.State == MEM_COMMIT
            && (mbi.Protect == PAGE_READWRITE
                || mbi.Protect == PAGE_READONLY
                || mbi.Protect == PAGE_EXECUTE_READ
                || mbi.Protect == PAGE_EXECUTE_READWRITE);

        if !is_readable {
            continue;
        }

            let buffer = match read_memory(process, mbi.BaseAddress as usize, mbi.RegionSize) {
            Ok(v) => v,
            Err(_) => continue,
        };

        for i in 0..buffer.len() {
            if i + 30 > buffer.len() {
                break;
            }
            if is_session_id(&buffer[i..i + 30]) {
                if let Ok(session) = std::str::from_utf8(&buffer[i..i + 30]) {
                    let session_string = session.to_string();
                    if !results.contains(&session_string) {
                        results.push(session_string);
                    }
                }
            }
        }
    }
    results
}

fn is_session_id(bytes: &[u8]) -> bool {
    if bytes.len() < 30 {
        return false;
    }

    // Validate [A-Z][a-z][0-9] x10 pattern
    for i in 0..10 {
        let base = i * 3;
        if !bytes[base].is_ascii_uppercase() {
            return false;
        }
        if !bytes[base + 1].is_ascii_lowercase() {
            return false;
        }
        if !bytes[base + 2].is_ascii_digit() {
            return false;
        }
    }
    true
}

/// Queries information about a range of pages in the virtual address space
/// of a target process.
///
/// Wraps `VirtualQueryEx` from the Windows API.
///
/// # Arguments
/// * `process_handle` - A handle to the target process. Must have been opened
///   with `PROCESS_QUERY_INFORMATION` access rights.
/// * `address` - The address to query. Windows will return info about the
///   region that contains this address, not necessarily starting at it.
///
/// # Returns
/// * `Ok()` - Updates mbi (MEMORY_BASIC_INFORMATION):
///   - `BaseAddress`  — where the region starts
///   - `RegionSize`   — size of the region in bytes
///   - `State`        — `MEM_COMMIT`, `MEM_RESERVE`, or `MEM_FREE`
///   - `Protect`      — page protection flags (read/write/execute)
///   - `Type`         — `MEM_PRIVATE`, `MEM_MAPPED`, or `MEM_IMAGE`
/// * `Err(MemoryError::QueryFailed)` - The address is beyond the process
///   memory space or the handle lacks required access rights.
///
/// # Example
/// ```rust
/// let mbi = query_region(handle, 0x1000)?;
/// println!("Region size: {} bytes", mbi.RegionSize);
/// ```
pub fn query_region(
    process_handle: HANDLE,
    address: usize,
    mbi: &mut MEMORY_BASIC_INFORMATION,
) -> Result<(), MemoryError> {
    let result = unsafe {
        VirtualQueryEx(
            process_handle,
            Some(address as *const _),
            mbi,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };

    if result == 0 {
        return Err(MemoryError::QueryFailed(format!("{:#010x}", address)));
    }

    Ok(())
}

/// Reads a block of bytes from the virtual address space of a target process
/// into a local buffer.
///
/// Wraps `ReadProcessMemory` from the Windows API.
///
/// # Arguments
/// * `process_handle` - A handle to the target process. Must have been opened
///   with `PROCESS_VM_READ` access rights.
/// * `address` - The base address in the target process to read from.
/// * `size` - Number of bytes to read.
///
/// # Returns
/// * `Ok(Vec<u8>)` - The bytes read from the target process. The length may
///   be less than `size` if the read was partial (e.g. region boundary).
/// * `Err(MemoryError::ReadFailed)` - The read failed entirely. Common causes:
///   - Memory was freed between `query_region` and this call
///   - Handle lacks `PROCESS_VM_READ` rights
///   - Address is not accessible (guard page, etc.)
///
/// # Example
/// ```rust
/// let bytes = read_memory(handle, 0x1000, 64)?;
/// println!("First byte: {:#04x}", bytes[0]);
/// ```
fn read_memory(
    process_handle: HANDLE,
    address: usize,
    size: usize,
) -> Result<Vec<u8>, MemoryError> {
    let mut buffer = vec![0u8; size];
    let mut bytes_read: usize = 0;

    let ok = unsafe {
        ReadProcessMemory(
            process_handle,
            address as *const _,
            buffer.as_mut_ptr() as *mut _,
            size,
            Some(&mut bytes_read),
        )
    };

    if ok.is_err() {
        return Err(MemoryError::ReadFailed(format!(
            "[address: {}, size: {}]",
            address, size
        )));
    }

    // Truncate to what was actually read in case of a partial read
    buffer.truncate(bytes_read);
    Ok(buffer)
}

/// Finds the base address of a loaded module in a target process by name.
///
/// Wraps `EnumProcessModules` and `GetModuleFileNameExA` from the Windows API.
///
/// The match is **case-insensitive** and **substring-based**, so you can pass
/// `"cccaster"` instead of the full `"cccaster.v3.1.exe"` path.
///
/// # Arguments
/// * `process` - A handle to the target process. Must have been opened with
///   `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ` access rights — both are
///   required, `EnumProcessModules` needs query and `GetModuleFileNameExA`
///   needs VM read.
/// * `target` - A case-insensitive substring of the module name to find,
///   e.g. `"cccaster"` or `"cccaster.v3.1.exe"`.
///
/// # Returns
/// * `Ok(usize)` - The base address of the matched module in the target
///   process address space. You can use this to resolve static offsets like
///   `base + 0x38208C`.
/// * `Err(MemoryError::ReadFailed)` - `EnumProcessModules` failed. Usually
///   means the handle lacks required access rights.
/// * `Err(MemoryError::ModuleNotFound)` - No loaded module matched `target`.
///   Either the module isn't loaded yet or the name is wrong.
///
/// # Example
/// ```rust
/// let base = get_module_base(handle, "cccaster")?;
/// let client_mode_addr = base + 0x38208C;
/// println!("ClientMode at: {:#010x}", client_mode_addr);
/// ```
pub fn get_module_base(process: HANDLE, target: &str) -> Result<usize, MemoryError> {
    let mut modules = [HMODULE::default(); 1024];
    let mut needed: u32 = 0;

    unsafe {
        EnumProcessModules(
            process,
            modules.as_mut_ptr(),
            (modules.len() * size_of::<HMODULE>()) as u32,
            &mut needed,
        )
        .map_err(|_| MemoryError::ReadFailed(target.to_string()))?;

        let count = needed as usize / size_of::<HMODULE>();
        let target_lower = target.to_lowercase();

        for module in &modules[..count] {
            let mut name = [0u8; 260]; // MAX_PATH
            let len = GetModuleFileNameExA(Some(process), Some(*module), &mut name);

            if len == 0 {
                continue;
            }

            let name_str = std::str::from_utf8(&name[..len as usize]).unwrap_or("");

            if name_str.to_lowercase().contains(&target_lower) {
                return Ok(module.0 as usize);
            }
        }
    }

    Err(MemoryError::ModuleNotFound(target.to_string()))
}

/// Opens a handle to an existing process.
///
/// Wraps `OpenProcess` from the Windows API.
///
/// # Arguments
/// * `access` - The desired access rights for the handle. Common values:
///   - `PROCESS_VM_READ` — required for `ReadProcessMemory`
///   - `PROCESS_QUERY_INFORMATION` — required for `EnumProcessModules`
///     and `VirtualQueryEx`
///   - These can be combined with `|` if you need both
/// * `inherit_handle` - Whether child processes spawned by the current
///   process should inherit this handle. Almost always `false`.
/// * `pid` - The process ID of the target. You can find this via Task
///   Manager or by iterating processes with `EnumProcesses`.
///
/// # Returns
/// * `Ok(HANDLE)` - A handle to the target process with the requested
///   access rights. Must be closed with `CloseHandle` when done, or
///   wrapped in a type that does so on drop.
/// * `Err(MemoryError::OpenProcessFailed)` - The call failed. Common causes:
///   - The PID doesn't exist (process already exited)
///   - You requested access rights your process isn't privileged for
///   - The target process is protected (e.g. system processes)
///
/// # Example
/// ```rust
/// let handle = open_process(
///     PROCESS_VM_READ | PROCESS_QUERY_INFORMATION,
///     false,
///     1234
/// )?;
/// ```
pub fn open_process(
    access: PROCESS_ACCESS_RIGHTS,
    inherit_handle: bool,
    pid: u32,
) -> Result<HANDLE, MemoryError> {
    unsafe {
        OpenProcess(access, inherit_handle, pid).map_err(|_| MemoryError::OpenProcessFailed(pid))
    }
}
