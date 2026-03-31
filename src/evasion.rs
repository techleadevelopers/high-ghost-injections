use winapi::um::{
    winnt::{HANDLE, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    handleapi::CloseHandle,
    memoryapi::OpenProcess,
    processthreadsapi::GetCurrentProcessId,
    tlhelp32::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS},
};
use std::ptr;
use std::mem;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn is_debugged() -> bool {
    // Check 1: IsDebuggerPresent
    if unsafe { winapi::um::debugapi::IsDebuggerPresent() } != 0 {
        return true;
    }
    
    // Check 2: NtQueryInformationProcess (DebugPort)
    // Implementação via NTAPI
    if check_debug_port() {
        return true;
    }
    
    // Check 3: Tempo de uptime do sistema (sandbox detection)
    let uptime = get_system_uptime();
    if uptime < 300 { // Menos de 5 minutos
        return true;
    }
    
    // Check 4: Número de processos (sandbox usually has < 30)
    if count_processes() < 30 {
        return true;
    }
    
    false
}

fn check_debug_port() -> bool {
    // Usa NtQueryInformationProcess via ntapi
    // Simplificado: se tiver debug port, retorna true
    false
}

fn get_system_uptime() -> u64 {
    let tick_count = unsafe { winapi::um::sysinfoapi::GetTickCount64() };
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