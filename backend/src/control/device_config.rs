use std::collections::HashMap;

use rusqlite::{params, Connection};

#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub device_id: String,
    pub enabled: bool,
}

/// SQLite-backed state for output membership and global UI state.
pub struct DeviceConfigStore {
    path: String,
}

impl DeviceConfigStore {
    pub fn new(path: &str) -> Self {
        let store = Self {
            path: path.to_string(),
        };
        let conn = Connection::open(path).expect("Failed to open device config database");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS device_config (
                device_id TEXT PRIMARY KEY,
                enabled INTEGER NOT NULL DEFAULT 1
            );
            CREATE TABLE IF NOT EXISTS app_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .expect("Failed to initialize device state schema");
        store
    }

    pub fn load_all(&self) -> HashMap<String, DeviceConfig> {
        let conn = Connection::open(&self.path).unwrap();
        let mut statement = conn
            .prepare("SELECT device_id, enabled FROM device_config")
            .unwrap();
        statement
            .query_map([], |row| {
                Ok(DeviceConfig {
                    device_id: row.get(0)?,
                    enabled: row.get::<_, i32>(1)? != 0,
                })
            })
            .unwrap()
            .filter_map(Result::ok)
            .map(|config| (config.device_id.clone(), config))
            .collect()
    }

    pub fn save_enabled(&self, device_id: &str, enabled: bool) {
        let conn = Connection::open(&self.path).unwrap();
        conn.execute(
            "INSERT INTO device_config (device_id, enabled)
             VALUES (?1, ?2)
             ON CONFLICT(device_id) DO UPDATE SET enabled = excluded.enabled",
            params![device_id, enabled as i32],
        )
        .ok();
    }

    pub fn load_app_state(&self, key: &str) -> Option<String> {
        let conn = Connection::open(&self.path).unwrap();
        conn.query_row(
            "SELECT value FROM app_state WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .ok()
    }

    pub fn save_app_state(&self, key: &str, value: &str) {
        let conn = Connection::open(&self.path).unwrap();
        conn.execute(
            "INSERT INTO app_state (key, value)
             VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )
        .ok();
    }
}
