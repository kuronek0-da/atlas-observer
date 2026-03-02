use std::{ffi::CString, mem, ptr, thread, time::Duration};
use winapi::shared::minwindef::{DWORD, FALSE};
use winapi::um::handleapi::{ CloseHandle, INVALID_HANDLE_VALUE };
use winapi::um::memoryapi::ReadProcessMemory;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::tlhelp32::*;
use winapi::um::winnt::{HANDLE, PROCESS_VM_READ, PROCESS_QUERY_INFORMATION};

const P1_WINS_ADDR: usize = 0x559550;
const P2_WINS_ADDR: usize = 0x559580;

unsafe fn get_process_id(process_name: &str) -> Option<DWORD> {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if snapshot == INVALID_HANDLE_VALUE {
        return None;
    }

    let mut entry: PROCESSENTRY32 = mem::zeroed();
    entry.dwSize = mem::size_of::<PROCESSENTRY32>() as u32;

    if Process32First(snapshot, &mut entry) == FALSE {
        CloseHandle(snapshot);
        return None;
    }

    loop {
        let exe_name = CString::from_vec_unchecked(
            entry.szExeFile
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as u8)
                .collect(),
        );

        if exe_name.to_string_lossy().eq_ignore_ascii_case(process_name) {
            CloseHandle(snapshot);
            return Some(entry.th32ProcessID);
        }

        if Process32Next(snapshot, &mut entry) == FALSE {
            break;
        }
    }

    CloseHandle(snapshot);
    None
}

unsafe fn read_u32(process: HANDLE, addr: usize) -> u32 {
    let mut buffer: u32 = 0;
    ReadProcessMemory(
        process,
        addr as _,
        &mut buffer as *mut _ as _,
        mem::size_of::<u32>(),
        ptr::null_mut(),
    );
    buffer
}

fn main() {
    unsafe {
        let process_name = "MBAA.exe"; // ajuste se necessário

        let pid = match get_process_id(process_name) {
            Some(id) => id,
            None => {
                println!("Processo não encontrado.");
                return;
            }
        };

        println!("PID encontrado: {}", pid);

        let process = OpenProcess(
            PROCESS_VM_READ | PROCESS_QUERY_INFORMATION,
            FALSE,
            pid,
        );

        if process.is_null() {
            println!("Falha ao abrir processo.");
            return;
        }

        println!("Lendo memória...");

        loop {
            let p1 = read_u32(process, P1_WINS_ADDR);
            let p2 = read_u32(process, P2_WINS_ADDR);

            println!("P1 Wins: {} | P2 Wins: {}", p1, p2);

            thread::sleep(Duration::from_millis(500));
        }

        // nunca chega aqui por causa do loop
        // CloseHandle(process);
    }
}