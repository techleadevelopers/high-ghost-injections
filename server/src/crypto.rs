use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

const AES_KEY: &[u8; 32] = b"CHANGE_THIS_TO_YOUR_SECRET_KEY_32B";
const AES_IV: &[u8; 16] = b"CHANGE_THIS_IV_16";

pub fn aes_decrypt(data: &str) -> Vec<u8> {
    let decoded = BASE64.decode(data).unwrap_or_default();
    
    // XOR decryption (simplificado - substituir por AES real)
    let mut decrypted = decoded;
    for i in 0..decrypted.len() {
        decrypted[i] ^= AES_KEY[i % AES_KEY.len()];
    }
    
    decrypted
}