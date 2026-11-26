use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CiCacheEntry {
    pub hash: String,
    pub at: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CiLogFileEntry {
    pub name: String,
    pub suite: String,
    pub size_bytes: u64,
    pub updated_at: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CiLogsResponse {
    pub logs_dir: Option<String>,
    pub cache: HashMap<String, CiCacheEntry>,
    pub files: Vec<CiLogFileEntry>,
}

fn is_whitelisted_ci_log(name: &str) -> Option<&str> {
    let inner = name.strip_prefix("ci-")?.strip_suffix(".log")?;
    if !inner.is_empty()
        && inner
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        Some(inner)
    } else {
        None
    }
}

/// Resolves the canonical SSOT CI log directory (`~/.operon/logs/ci/` or `$DATA_DIR/logs/ci/`).
pub fn resolve_ci_logs_dir() -> PathBuf {
    crate::config::canonical_ssot_dir()
        .join(crate::constants::DIR_LOGS)
        .join("ci")
}

/// Resolves the canonical SSOT daemon log file (`~/.operon/logs/daemon.log`).
pub fn resolve_daemon_log_path() -> PathBuf {
    crate::config::canonical_ssot_dir()
        .join(crate::constants::DIR_LOGS)
        .join(crate::constants::FILE_WORKER_LOG)
}

fn read_cache_map(logs_dir: &Path) -> HashMap<String, CiCacheEntry> {
    let cache_path = logs_dir.join(".ci-cache.json");
    std::fs::read_to_string(cache_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<HashMap<String, CiCacheEntry>>(&raw).ok())
        .unwrap_or_default()
}

fn format_modified_iso(meta: &std::fs::Metadata) -> String {
    meta.modified()
        .ok()
        .map(|st| chrono::DateTime::<chrono::Utc>::from(st).to_rfc3339())
        .unwrap_or_default()
}

pub fn collect_ci_logs(logs_dir: Option<&Path>) -> CiLogsResponse {
    let Some(dir) = logs_dir.filter(|p| p.is_dir()) else {
        return CiLogsResponse {
            logs_dir: logs_dir.map(|p| p.to_string_lossy().to_string()),
            cache: HashMap::new(),
            files: Vec::new(),
        };
    };

    let cache = read_cache_map(dir);
    let mut files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let Some(suite) = is_whitelisted_ci_log(&file_name).map(str::to_string) else {
                continue;
            };
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let meta = entry.metadata().ok();
            let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let updated_at = meta.as_ref().map(format_modified_iso).unwrap_or_default();

            files.push(CiLogFileEntry {
                name: file_name,
                suite,
                size_bytes,
                updated_at,
                content,
            });
        }
    }

    files.sort_by(|a, b| a.suite.cmp(&b.suite));

    CiLogsResponse {
        logs_dir: Some(dir.to_string_lossy().to_string()),
        cache,
        files,
    }
}

#[utoipa::path(
    tag = "System",
    get,
    path = "/api/v1/system/ci-logs",
    operation_id = "get_ci_logs",
    summary = "Read local CI logs and cache metadata from ~/.operon/logs/ci",
    responses(
        (status = 200, description = "Whitelisted ci-*.log files and .ci-cache.json from SSOT ~/.operon/logs/ci", body = CiLogsResponse)
    )
)]
pub async fn get_ci_logs() -> Json<CiLogsResponse> {
    let dir = resolve_ci_logs_dir();
    Json(collect_ci_logs(Some(&dir)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_only_whitelisted_ci_logs_and_parses_cache() {
        let tmp =
            std::env::temp_dir().join(format!("operon_ci_logs_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join(".ci-cache.json"),
            r#"{"glossary":{"hash":"abc","at":"2026-11-26T09:00:00Z"}}"#,
        )
        .unwrap();
        std::fs::write(tmp.join("notes.txt"), "ignored").unwrap();
        std::fs::write(tmp.join("ci-core.log"), "=== local-ci: core ===").unwrap();

        let res = collect_ci_logs(Some(&tmp));
        assert_eq!(res.files.len(), 1);
        assert_eq!(res.files[0].suite, "core");
        assert_eq!(res.files[0].name, "ci-core.log");
        assert_eq!(res.cache.get("glossary").unwrap().hash, "abc");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_ci_logs_dir_points_to_ssot_logs_ci() {
        let dir = resolve_ci_logs_dir();
        let norm = dir.to_string_lossy().replace('\\', "/");
        assert!(norm.ends_with("/logs/ci"));
    }
}
