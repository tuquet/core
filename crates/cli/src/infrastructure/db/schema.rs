use rusqlite::{Connection, Result};

pub fn init_db(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS jobs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            data TEXT NOT NULL, -- JSON
            options TEXT, -- JSON
            status TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            job_id TEXT NOT NULL,
            type TEXT NOT NULL,
            message TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
        );
        
        CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
        CREATE INDEX IF NOT EXISTS idx_logs_job_id ON logs(job_id);

        CREATE TABLE IF NOT EXISTS browsers (
            id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL DEFAULT 'local-default',
            name TEXT NOT NULL,
            group_name TEXT DEFAULT 'default',
            tags TEXT DEFAULT '[]',
            user_agent TEXT,
            timezone TEXT,
            proxy TEXT,
            browser_type TEXT DEFAULT 'Chromium',
            extensions TEXT DEFAULT '["automation"]',
            fingerprint_config TEXT DEFAULT '{}',
            startup_flags TEXT DEFAULT '[]',
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS system_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        "#,
    )?;

    // Rip-and-replace: the legacy Automa-style storage layer is removed — credentials live in the
    // encrypted Secret Store (SPEC-secret-store), variables/secrets in Cloud, data tables in Cloud PostgREST.
    let _ = conn.execute("DROP TABLE IF EXISTS storage_credentials", []);
    let _ = conn.execute("DROP TABLE IF EXISTS storage_table_rows", []);
    let _ = conn.execute("DROP TABLE IF EXISTS storage_tables", []);
    let _ = conn.execute("DROP TABLE IF EXISTS storage_variables", []);
    let _ = conn.execute("DROP TABLE IF EXISTS workflows", []);

    let mut stmt_after = conn.prepare("PRAGMA table_info(browsers)")?;
    let current_cols = stmt_after
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(Result::ok)
        .collect::<Vec<String>>();

    let browser_migrations = [
        (
            "workspace_id",
            "ALTER TABLE browsers ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'local-default'",
        ),
        (
            "group_name",
            "ALTER TABLE browsers ADD COLUMN group_name TEXT DEFAULT 'default'",
        ),
        (
            "tags",
            "ALTER TABLE browsers ADD COLUMN tags TEXT DEFAULT '[]'",
        ),
        (
            "extensions",
            "ALTER TABLE browsers ADD COLUMN extensions TEXT DEFAULT '[\"automation\"]'",
        ),
        (
            "fingerprint_config",
            "ALTER TABLE browsers ADD COLUMN fingerprint_config TEXT DEFAULT '{}'",
        ),
        (
            "startup_flags",
            "ALTER TABLE browsers ADD COLUMN startup_flags TEXT DEFAULT '[]'",
        ),
    ];

    for (col, ddl) in browser_migrations {
        if !current_cols.iter().any(|c| c == col) {
            conn.execute(ddl, [])?;
        }
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_browsers_workspace ON browsers(workspace_id)",
        [],
    )?;

    Ok(())
}
