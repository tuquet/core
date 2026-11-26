use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};

pub mod browsers;
pub mod jobs;
pub mod schema;
pub mod seed;
pub mod settings;
pub mod tasks;

pub use browsers::{
    Browser, CreateBrowserRecord, DEFAULT_WORKSPACE_ID, FingerprintConfig, PatchBrowserRecord,
    SqliteBrowserRepository,
};
pub use jobs::{JobDetails, JobHistoryItem, JobInfo, LogItem, SqliteJobRepository};
pub use seed::{SeedSummary, reset_and_seed};
pub use settings::SqliteSettingsRepository;
pub use tasks::{
    BulkUpdateTasksRecord, CreateTaskRecord, SqliteTaskRepository, TaskRecord, UpdateTaskRecord,
};

/// Resolve canonical persistent SQLite path (`~/.operon/data/operon.sqlite` for default
/// automation dir, or `<data_dir>/operon.sqlite` when isolated via `--data-dir` / `DATA_DIR`).
pub fn resolve_db_path(config_data_dir: &str) -> PathBuf {
    let ssot_root = crate::config::canonical_ssot_dir();
    let default_automation_dir = ssot_root.join(crate::constants::PILLAR_AUTOMATION);
    if Path::new(config_data_dir) == default_automation_dir {
        ssot_root
            .join(crate::constants::DIR_DATA)
            .join(crate::constants::FILE_OPERON_SQLITE)
    } else {
        PathBuf::from(config_data_dir).join(crate::constants::FILE_OPERON_SQLITE)
    }
}

/// Open persistent SQLite DB, migrating legacy `automation/automation.sqlite` on default SSOT runs.
pub fn open_persistent_db(config_data_dir: &str) -> Result<AutomationDb> {
    let db_path = resolve_db_path(config_data_dir);
    let ssot_root = crate::config::canonical_ssot_dir();
    let default_db_path = ssot_root
        .join(crate::constants::DIR_DATA)
        .join(crate::constants::FILE_OPERON_SQLITE);

    if db_path == default_db_path && !db_path.exists() {
        let legacy_db_path = ssot_root
            .join(crate::constants::PILLAR_AUTOMATION)
            .join(crate::constants::FILE_LEGACY_AUTOMATION_SQLITE);
        if legacy_db_path.exists() {
            if let Some(parent) = db_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(&legacy_db_path, &db_path);
        }
    }

    AutomationDb::new(&db_path)
}

/// Recover orphaned `prf_*` browser profile folders on disk that are absent from SQLite.
pub fn reconcile_orphan_browser_dirs(
    db: &AutomationDb,
    data_dir: &str,
    browsers_rel: &str,
) -> usize {
    let mut scan_roots = vec![PathBuf::from(data_dir).join(browsers_rel)];
    let ssot_profiles_dir = browser::BrowserProfile::default_profiles_dir();
    if !scan_roots.contains(&ssot_profiles_dir) {
        scan_roots.push(ssot_profiles_dir);
    }

    let mut recovered = 0;
    for root in scan_roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.starts_with("prf_")
                || !crate::core::models::id::BrowserId::new(name).is_valid()
            {
                continue;
            }
            if let Ok(None) = db.browsers().get_browser(name)
                && db
                    .browsers()
                    .create_browser(name, name, None, None, None, None)
                    .is_ok()
            {
                recovered += 1;
            }
        }
    }
    recovered
}

pub struct AutomationDb {
    conn: Connection,
}

impl AutomationDb {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        if let Some(parent) = path_ref.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path_ref)?;
        schema::init_db(&conn)?;
        Ok(Self { conn })
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::init_db(&conn)?;
        Ok(Self { conn })
    }

    pub fn raw_conn(&self) -> &Connection {
        &self.conn
    }

    // --- Sub-Repositories ---

    pub fn jobs(&self) -> SqliteJobRepository<'_> {
        SqliteJobRepository::new(&self.conn)
    }

    pub fn browsers(&self) -> SqliteBrowserRepository<'_> {
        SqliteBrowserRepository::new(&self.conn)
    }

    pub fn settings(&self) -> SqliteSettingsRepository<'_> {
        SqliteSettingsRepository::new(&self.conn)
    }

    pub fn tasks(&self) -> SqliteTaskRepository<'_> {
        SqliteTaskRepository::new(&self.conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persistent_db_survives_reopen_and_reconciles_orphans() {
        let tmp = std::env::temp_dir().join(format!("opn-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(tmp.join("browsers/prf_abc123def456")).unwrap();
        std::fs::create_dir_all(tmp.join("browsers/not_a_prf_dir")).unwrap();

        let data_dir = tmp.to_string_lossy().to_string();
        {
            let db = open_persistent_db(&data_dir).unwrap();
            assert_eq!(reconcile_orphan_browser_dirs(&db, &data_dir, "browsers"), 1);
            assert_eq!(reconcile_orphan_browser_dirs(&db, &data_dir, "browsers"), 0);
            db.browsers()
                .create_browser("prf_manual999", "Manual Profile", None, None, None, None)
                .unwrap();
        }

        // Reopen DB (simulating daemon restart / opn update) and confirm both profiles persist
        let reopened = open_persistent_db(&data_dir).unwrap();
        let list = reopened
            .browsers()
            .list_browsers(None, None, None, None)
            .unwrap();
        let ids: Vec<&str> = list.iter().map(|b| b.id.as_str()).collect();
        assert!(ids.contains(&"prf_abc123def456"));
        assert!(ids.contains(&"prf_manual999"));
        assert!(!ids.contains(&"not_a_prf_dir"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
