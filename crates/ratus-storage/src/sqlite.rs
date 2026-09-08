//! SQLite persistent storage backend using WAL mode.

use parking_lot::Mutex;
use ratus_core::config::EndpointConfig;
use ratus_core::error::{RatusError, Result};
use ratus_core::models::EndpointResult;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Persistent SQLite storage engine.
#[derive(Clone)]
pub struct SqliteStorage {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStorage {
    /// Open or create a SQLite database file with WAL mode enabled.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| RatusError::Storage(e.to_string()))?;

        // Enable Write-Ahead Logging for high-concurrency performance
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| RatusError::Storage(e.to_string()))?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|e| RatusError::Storage(e.to_string()))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| RatusError::Storage(e.to_string()))?;

        // Initialize schema
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS endpoint_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                endpoint_key TEXT NOT NULL,
                endpoint_name TEXT NOT NULL,
                endpoint_group TEXT,
                success INTEGER NOT NULL,
                status_code INTEGER NOT NULL,
                duration_us INTEGER NOT NULL,
                errors TEXT,
                timestamp TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_endpoint_results_key ON endpoint_results(endpoint_key, id DESC);
            "#,
        )
        .map_err(|e| RatusError::Storage(e.to_string()))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open an in-memory SQLite database (primarily for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| RatusError::Storage(e.to_string()))?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS endpoint_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                endpoint_key TEXT NOT NULL,
                endpoint_name TEXT NOT NULL,
                endpoint_group TEXT,
                success INTEGER NOT NULL,
                status_code INTEGER NOT NULL,
                duration_us INTEGER NOT NULL,
                errors TEXT,
                timestamp TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_endpoint_results_key ON endpoint_results(endpoint_key, id DESC);
            "#,
        )
        .map_err(|e| RatusError::Storage(e.to_string()))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Persist a probe result for an endpoint.
    pub fn save_result(&self, endpoint: &EndpointConfig, result: &EndpointResult) -> Result<()> {
        let key = endpoint.key();
        let errors_json = if result.errors.is_empty() {
            None
        } else {
            serde_json::to_string(&result.errors).ok()
        };

        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO endpoint_results (
                endpoint_key, endpoint_name, endpoint_group, success,
                status_code, duration_us, errors, timestamp
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                key,
                endpoint.name,
                endpoint.group,
                if result.success { 1 } else { 0 },
                result.status_code as i64,
                result.duration.as_micros() as i64,
                errors_json,
                result.timestamp.to_rfc3339()
            ],
        )
        .map_err(|e| RatusError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Save result directly by key and name.
    pub fn save_result_by_key(
        &self,
        key: &str,
        name: &str,
        group: Option<&str>,
        result: &EndpointResult,
    ) -> Result<()> {
        let errors_json = if result.errors.is_empty() {
            None
        } else {
            serde_json::to_string(&result.errors).ok()
        };

        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO endpoint_results (
                endpoint_key, endpoint_name, endpoint_group, success,
                status_code, duration_us, errors, timestamp
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                key,
                name,
                group,
                if result.success { 1 } else { 0 },
                result.status_code as i64,
                result.duration.as_micros() as i64,
                errors_json,
                result.timestamp.to_rfc3339()
            ],
        )
        .map_err(|e| RatusError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Load recent probe results for all endpoints up to `limit_per_endpoint`.
    pub fn load_all_recent(
        &self,
        limit_per_endpoint: usize,
    ) -> Result<HashMap<String, Vec<EndpointResult>>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                r#"
                SELECT endpoint_key, success, status_code, duration_us, errors, timestamp
                FROM endpoint_results
                ORDER BY id DESC
                "#,
            )
            .map_err(|e| RatusError::Storage(e.to_string()))?;

        let mut map: HashMap<String, Vec<EndpointResult>> = HashMap::new();
        let rows = stmt
            .query_map([], |row| {
                let key: String = row.get(0)?;
                let success_int: i64 = row.get(1)?;
                let status_code: i64 = row.get(2)?;
                let duration_us: i64 = row.get(3)?;
                let errors_opt: Option<String> = row.get(4)?;
                let timestamp_str: String = row.get(5)?;

                Ok((
                    key,
                    success_int,
                    status_code,
                    duration_us,
                    errors_opt,
                    timestamp_str,
                ))
            })
            .map_err(|e| RatusError::Storage(e.to_string()))?;

        for row_res in rows {
            let (key, success_int, status_code, duration_us, errors_opt, timestamp_str) =
                row_res.map_err(|e| RatusError::Storage(e.to_string()))?;

            let list = map.entry(key).or_default();
            if list.len() < limit_per_endpoint {
                let ts = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

                let errors = errors_opt
                    .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
                    .unwrap_or_default();

                list.push(EndpointResult {
                    timestamp: ts,
                    success: success_int != 0,
                    status_code: status_code as u16,
                    duration: Duration::from_micros(duration_us as u64),
                    errors,
                    condition_results: Vec::new(),
                    ip: None,
                    hostname: None,
                });
            }
        }

        for list in map.values_mut() {
            list.reverse();
        }

        Ok(map)
    }

    /// Reset / truncate persistent storage.
    pub fn reset(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM endpoint_results", [])
            .map_err(|e| RatusError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_persistence() {
        let db = SqliteStorage::open_in_memory().expect("in-memory db");
        let ep = EndpointConfig {
            name: "test-ep".to_string(),
            group: Some("grp".to_string()),
            url: Some("http://localhost".to_string()),
            method: "GET".to_string(),
            body: None,
            headers: None,
            interval: Duration::from_secs(10),
            conditions: vec!["[STATUS] == 200".to_string()],
            alerts: None,
            client: None,
            ui: None,
            dns: None,
            ssh: None,
            enabled: true,
        };

        let r1 = EndpointResult::success(200, Duration::from_millis(15));
        let r2 = EndpointResult::failure(500, Duration::from_millis(25), "Timeout");

        db.save_result(&ep, &r1).unwrap();
        db.save_result(&ep, &r2).unwrap();

        let loaded = db.load_all_recent(10).unwrap();
        let key = ep.key();
        assert!(loaded.contains_key(&key));
        let results = &loaded[&key];
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status_code, 200);
        assert_eq!(results[1].status_code, 500);

        db.reset().unwrap();
        let loaded_empty = db.load_all_recent(10).unwrap();
        assert!(loaded_empty.is_empty());
    }
}
