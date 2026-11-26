use automation_core::config::AppConfig;
use automation_core::infrastructure::db::{
    DEFAULT_WORKSPACE_ID, open_persistent_db, reset_and_seed, resolve_db_path,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut workspace_id = DEFAULT_WORKSPACE_ID.to_string();
    let mut clear_all = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--all" => clear_all = true,
            "--workspace" | "-w" => {
                if let Some(val) = args.get(i + 1) {
                    workspace_id = val.clone();
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!(
                    "Usage: cargo run -p operon --example db_reset -- [--workspace <id>] [--all]"
                );
                return;
            }
            _ => {}
        }
        i += 1;
    }

    let config = AppConfig::load();
    let db_path = resolve_db_path(&config.data_dir);
    let db = open_persistent_db(&config.data_dir).expect("failed to open persistent SQLite DB");
    let summary =
        reset_and_seed(&db, &workspace_id, clear_all).expect("failed to reset and seed SQLite DB");

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "success": true,
            "dbPath": db_path,
            "summary": summary,
        }))
        .unwrap()
    );
}
