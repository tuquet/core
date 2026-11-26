use std::path::Path;

use serde_json::{Value, json};

use crate::api::handlers::system::ci_logs::{
    collect_ci_logs, resolve_ci_logs_dir, resolve_daemon_log_path,
};

fn matches_error_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("[fail")
        || lower.contains("error")
        || lower.contains("warn")
        || lower.contains("failed")
        || lower.contains("access is denied")
        || lower.contains("caused by:")
}

fn filter_log_lines(
    content: &str,
    errors_only: bool,
    search: Option<&str>,
    tail: usize,
) -> Vec<String> {
    let needle = search.map(|s| s.to_ascii_lowercase());
    let filtered: Vec<String> = content
        .lines()
        .filter(|line| !errors_only || matches_error_line(line))
        .filter(|line| match needle.as_deref() {
            Some(q) if !q.is_empty() => line.to_ascii_lowercase().contains(q),
            _ => true,
        })
        .map(str::to_string)
        .collect();

    let start = filtered.len().saturating_sub(tail);
    filtered[start..].to_vec()
}

pub fn read_ssot_logs_payload(
    ci_dir: &Path,
    daemon_log_path: &Path,
    args: &Value,
) -> Result<String, String> {
    let source = args.get("source").and_then(|v| v.as_str()).unwrap_or("ci");
    let suite_filter = args.get("suite").and_then(|v| v.as_str()).map(str::trim);
    let errors_only = args
        .get("errors_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let search = args.get("search").and_then(|v| v.as_str()).map(str::trim);
    let tail = args
        .get("tail")
        .and_then(|v| v.as_u64())
        .map(|v| (v as usize).clamp(1, 500))
        .unwrap_or(100);

    match source {
        "daemon" => {
            if !daemon_log_path.is_file() {
                return Ok(serde_json::to_string_pretty(&json!({
                    "source": "daemon",
                    "path": daemon_log_path.to_string_lossy(),
                    "exists": false,
                    "lines": []
                }))
                .unwrap_or_default());
            }
            let content = std::fs::read_to_string(daemon_log_path).map_err(|e| e.to_string())?;
            let lines = filter_log_lines(&content, errors_only, search, tail);
            Ok(serde_json::to_string_pretty(&json!({
                "source": "daemon",
                "path": daemon_log_path.to_string_lossy(),
                "exists": true,
                "line_count": lines.len(),
                "lines": lines
            }))
            .unwrap_or_default())
        }
        "ci" => {
            let collected = collect_ci_logs(Some(ci_dir));
            let entries: Vec<Value> = collected
                .files
                .into_iter()
                .filter(|f| match suite_filter {
                    Some(s) if !s.is_empty() => f.suite == s,
                    _ => true,
                })
                .map(|f| {
                    let lines = filter_log_lines(&f.content, errors_only, search, tail);
                    json!({
                        "suite": f.suite,
                        "file": f.name,
                        "updated_at": f.updated_at,
                        "size_bytes": f.size_bytes,
                        "line_count": lines.len(),
                        "lines": lines,
                    })
                })
                .collect();

            Ok(serde_json::to_string_pretty(&json!({
                "source": "ci",
                "logs_dir": collected.logs_dir,
                "cache": collected.cache,
                "suites": entries,
            }))
            .unwrap_or_default())
        }
        other => Err(format!(
            "Unsupported log source '{}': expected 'ci' or 'daemon'",
            other
        )),
    }
}

/// MCP tool handler for reading filtered local SSOT logs (`~/.operon/logs/`).
pub async fn execute_logs_read(args: &Value) -> Result<String, String> {
    let ci_dir = resolve_ci_logs_dir();
    let daemon_log = resolve_daemon_log_path();
    read_ssot_logs_payload(&ci_dir, &daemon_log, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_logs_read_filters_ci_suite_errors_and_tail() {
        let tmp =
            std::env::temp_dir().join(format!("operon_mcp_logs_test_{}", uuid::Uuid::new_v4()));
        let ci_dir = tmp.join("ci");
        std::fs::create_dir_all(&ci_dir).unwrap();
        std::fs::write(
            ci_dir.join("ci-core.log"),
            "=== local-ci: core ===\ninfo ok\nerror: clippy failed\n[FAIL 1] cargo clippy (2.0s)\n",
        )
        .unwrap();
        std::fs::write(
            ci_dir.join("ci-glossary.log"),
            "[PASS] check:glossary (1.0s)\n",
        )
        .unwrap();

        let out = read_ssot_logs_payload(
            &ci_dir,
            &tmp.join("daemon.log"),
            &json!({ "source": "ci", "suite": "core", "errors_only": true, "tail": 10 }),
        )
        .unwrap();

        let parsed: Value = serde_json::from_str(&out).unwrap();
        let suites = parsed["suites"].as_array().unwrap();
        assert_eq!(suites.len(), 1);
        assert_eq!(suites[0]["suite"], "core");
        let lines = suites[0]["lines"].as_array().unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "error: clippy failed");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
