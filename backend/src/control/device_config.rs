use std::collections::HashMap;

use rusqlite::{params, Connection};

#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub device_id: String,
    pub enabled: bool,
    pub volume: Option<f64>,
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
                enabled INTEGER NOT NULL DEFAULT 1,
                volume REAL
            );
            CREATE TABLE IF NOT EXISTS app_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .expect("Failed to initialize device state schema");
        let has_volume = {
            let mut columns = conn.prepare("PRAGMA table_info(device_config)").unwrap();
            let found = columns
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .filter_map(Result::ok)
                .any(|column| column == "volume");
            found
        };
        if !has_volume {
            conn.execute("ALTER TABLE device_config ADD COLUMN volume REAL", [])
                .expect("Failed to add per-device volume storage");
        }
        store
    }

    pub fn load_all(&self) -> HashMap<String, DeviceConfig> {
        let conn = Connection::open(&self.path).unwrap();
        let mut statement = conn
            .prepare("SELECT device_id, enabled, volume FROM device_config")
            .unwrap();
        statement
            .query_map([], |row| {
                Ok(DeviceConfig {
                    device_id: row.get(0)?,
                    enabled: row.get::<_, i32>(1)? != 0,
                    volume: row.get(2)?,
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

    pub fn save_volume(&self, device_id: &str, volume: f64) {
        let conn = Connection::open(&self.path).unwrap();
        conn.execute(
            "INSERT INTO device_config (device_id, volume)
             VALUES (?1, ?2)
             ON CONFLICT(device_id) DO UPDATE SET volume = excluded.volume",
            params![device_id, volume],
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

    pub fn delete_app_state(&self, key: &str) {
        let conn = Connection::open(&self.path).unwrap();
        conn.execute("DELETE FROM app_state WHERE key = ?1", params![key])
            .ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_device_config_and_preserves_both_values() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("device_config.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE device_config (
                device_id TEXT PRIMARY KEY,
                enabled INTEGER NOT NULL DEFAULT 1
            );
            INSERT INTO device_config (device_id, enabled) VALUES ('speaker', 0);",
        )
        .unwrap();
        drop(conn);

        let store = DeviceConfigStore::new(path.to_str().unwrap());
        store.save_volume("speaker", 0.65);
        store.save_enabled("speaker", true);

        let config = store.load_all().remove("speaker").unwrap();
        assert!(config.enabled);
        assert_eq!(config.volume, Some(0.65));
    }
}
