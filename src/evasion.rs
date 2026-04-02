use winapi::um::{
    tlhelp32::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS},
    handleapi::CloseHandle,
};
use std::mem;

// Declaração externa das funções da kernel32
#[link(name = "kernel32")]
extern "system" {
    fn IsDebuggerPresent() -> i32;
    fn GetTickCount64() -> u64;
}

pub fn is_debugged() -> bool {
    // Check 1: IsDebuggerPresent
    if unsafe { IsDebuggerPresent() } != 0 {
        return true;
    }
    
    // Check 2: Tempo de uptime do sistema
    let uptime = get_system_uptime();
    if uptime < 300 { // Menos de 5 minutos
        return true;
    }
    
    // Check 3: Número de processos (sandbox geralmente tem < 30)
    if count_processes() < 30 {
        return true;
    }
    
    false
}

fn get_system_uptime() -> u64 {
    let tick_count = unsafe { GetTickCount64() };
    tick_count / 1000 // segundos
}

fn count_processes() -> u32 {
    let snapshot = unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    };
    
    if snapshot as isize == -1isize {
        return 0;
    }
    
    let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
    entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
    
    let mut count = 0;
    let success = unsafe { Process32FirstW(snapshot, &mut entry) };
    
    if success != 0 {
        count += 1;
        loop {
            let success = unsafe { Process32NextW(snapshot, &mut entry) };
            if success == 0 {
                break;
            }
            count += 1;
        }
    }
    
    unsafe { CloseHandle(snapshot); }
    count
}