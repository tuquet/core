use crate::core::error::AutomationError;
use axum::Json;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use utoipa::ToSchema;

const ALLOWED_CI_SUITES: &[&str] = &["all", "glossary", "core", "console", "docker"];
static ACTIVE_CI_SUITE: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn active_ci_lock() -> &'static Mutex<Option<String>> {
    ACTIVE_CI_SUITE.get_or_init(|| Mutex::new(None))
}

fn current_running_suites() -> Vec<String> {
    active_ci_lock()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .into_iter()
        .collect()
}

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
    pub runner_available: bool,
    pub running_suites: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CiRunRequest {
    pub suite: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CiRunResponse {
    pub started: bool,
    pub suite: String,
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

pub fn resolve_ci_repo_root() -> Option<PathBuf> {
    let has_script = |dir: &Path| dir.join("scripts").join("local-ci.mjs").is_file();

    if let Ok(raw) = std::env::var("OPERON_REPO_ROOT") {
        let candidate = PathBuf::from(raw.trim());
        if has_script(&candidate) {
            return Some(candidate);
        }
    }

    if let Ok(cwd) = std::env::current_dir()
        && let Some(found) = cwd.ancestors().find(|p| has_script(p))
    {
        return Some(found.to_path_buf());
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| has_script(p))
        .map(Path::to_path_buf)
}

pub fn collect_ci_logs(logs_dir: Option<&Path>) -> CiLogsResponse {
    let runner_available = resolve_ci_repo_root().is_some();
    let running_suites = current_running_suites();

    let Some(dir) = logs_dir.filter(|p| p.is_dir()) else {
        return CiLogsResponse {
            logs_dir: logs_dir.map(|p| p.to_string_lossy().to_string()),
            cache: HashMap::new(),
            files: Vec::new(),
            runner_available,
            running_suites,
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
        runner_available,
        running_suites,
    }
}

#[utoipa::path(
    tag = "System",
    get,
    path = "/api/v1/system/ci-logs",
    operation_id = "get_ci_logs",
    summary = "Read local CI logs and cache metadata",
    responses(
        (status = 200, description = "Whitelisted ci-*.log files and .ci-cache.json", body = CiLogsResponse)
    )
)]
pub async fn get_ci_logs() -> Json<CiLogsResponse> {
    let dir = resolve_ci_logs_dir();
    Json(collect_ci_logs(Some(&dir)))
}

#[utoipa::path(
    tag = "System",
    post,
    path = "/api/v1/system/ci-run",
    operation_id = "post_ci_run",
    summary = "Trigger local CI suite run via scripts/local-ci.mjs",
    request_body = CiRunRequest,
    responses(
        (status = 202, description = "Local CI suite started", body = CiRunResponse),
        (status = 400, description = "Unsupported suite", body = crate::core::error::ApiErrorResponse),
        (status = 404, description = "Repo runner script unavailable", body = crate::core::error::ApiErrorResponse),
        (status = 409, description = "Another CI suite is already running", body = crate::core::error::ApiErrorResponse)
    )
)]
pub async fn post_ci_run(
    body: Option<Json<CiRunRequest>>,
) -> Result<(StatusCode, Json<CiRunResponse>), AutomationError> {
    let suite = body
        .and_then(|Json(req)| req.suite)
        .unwrap_or_else(|| "all".to_string())
        .trim()
        .to_string();

    if !ALLOWED_CI_SUITES.contains(&suite.as_str()) {
        return Err(AutomationError::BadRequest(format!(
            "Unsupported CI suite: {suite}"
        )));
    }

    let repo_root = resolve_ci_repo_root().ok_or_else(|| {
        AutomationError::NotFound("scripts/local-ci.mjs not found in repository".to_string())
    })?;
    let script_path = repo_root.join("scripts").join("local-ci.mjs");

    {
        let mut active = active_ci_lock()
            .lock()
            .map_err(|_| AutomationError::Internal("CI runner lock poisoned".to_string()))?;
        if let Some(running) = active.as_ref() {
            return Err(AutomationError::Conflict(format!(
                "CI suite \"{running}\" is already running"
            )));
        }
        *active = Some(suite.clone());
    }

    let spawn_result = tokio::process::Command::new("node")
        .arg(&script_path)
        .arg(&suite)
        .arg("--no-cache")
        .current_dir(&repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    let mut child = match spawn_result {
        Ok(child) => child,
        Err(err) => {
            if let Ok(mut active) = active_ci_lock().lock() {
                *active = None;
            }
            return Err(AutomationError::ExecutionFailed(format!(
                "Failed to spawn local-ci.mjs: {err}"
            )));
        }
    };

    tokio::spawn(async move {
        let _ = child.wait().await;
        if let Ok(mut active) = active_ci_lock().lock() {
            *active = None;
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(CiRunResponse {
            started: true,
            suite,
        }),
    ))
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
