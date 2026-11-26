use crate::AppState;
use crate::core::browser::manager::browser_registry;
use crate::core::error::AutomationError;
use axum::extract::{Json, Path, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DriverReleaseItem {
    pub version: String,
    pub tag: String,
    pub channel: String,
    pub status: String,
    pub notes: String,
    pub installed: bool,
    pub is_active: bool,
    pub size_mb: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListDriversResponse {
    pub active_version: String,
    pub releases: Vec<DriverReleaseItem>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DriverActionResponse {
    pub success: bool,
    pub version: String,
    pub message: String,
}

#[utoipa::path(
    tag = "Browsers",
    get,
    path = "/api/v1/browsers/drivers",
    operation_id = "list_browser_drivers",
    summary = "List curated and installed browser drivers",
    responses(
        (status = 200, description = "Browser driver manifest and installed status", body = ListDriversResponse)
    )
)]
pub async fn list_drivers() -> Json<ListDriversResponse> {
    let manifest = browser::fetch_manifest(false)
        .await
        .unwrap_or_else(|_| browser::BrowserManifest::get_embedded());
    let installed = browser::list_installed_runtimes();
    let active_version = browser::get_active_version();
    let platform_key = browser::get_platform_key();

    let mut releases: Vec<DriverReleaseItem> = manifest
        .releases
        .into_iter()
        .map(|rel| {
            let inst = installed.iter().find(|i| i.version == rel.version);
            let manifest_size_mb = rel
                .platforms
                .get(platform_key)
                .map(|p| p.size as f64 / 1_048_576.0);
            DriverReleaseItem {
                is_active: inst.is_some_and(|i| i.is_active) || rel.version == active_version,
                installed: inst.is_some(),
                size_mb: inst.and_then(|i| i.size_mb).or(manifest_size_mb),
                version: rel.version,
                tag: rel.tag,
                channel: rel.channel,
                status: rel.status,
                notes: rel.notes,
            }
        })
        .collect();

    for inst in installed {
        if !releases.iter().any(|r| r.version == inst.version) {
            releases.push(DriverReleaseItem {
                version: inst.version,
                tag: inst.name,
                channel: inst.channel,
                status: inst.status_badge,
                notes: String::new(),
                installed: true,
                is_active: inst.is_active,
                size_mb: inst.size_mb,
            });
        }
    }

    Json(ListDriversResponse {
        active_version,
        releases,
    })
}

#[utoipa::path(
    tag = "Browsers",
    post,
    path = "/api/v1/browsers/drivers/{version}/install",
    operation_id = "install_browser_driver",
    summary = "Download and install a browser driver version",
    params(("version" = String, Path, description = "Target driver version")),
    responses(
        (status = 200, description = "Download started", body = DriverActionResponse),
        (status = 400, description = "Invalid version", body = crate::core::error::ApiErrorResponse)
    )
)]
pub async fn install_driver(
    State(state): State<AppState>,
    Path(version): Path<String>,
) -> Result<Json<DriverActionResponse>, AutomationError> {
    let clean = version.trim().to_string();
    if clean.is_empty() {
        return Err(AutomationError::BadRequest(
            "Version is required".to_string(),
        ));
    }

    let _ = state.tx.send(
        json!({
            "type": "driver_download_progress",
            "version": clean,
            "stage": "downloading",
            "percent": 5,
            "downloadedBytes": 0,
            "totalBytes": 0
        })
        .to_string(),
    );

    let tx = state.tx.clone();
    let ver_clone = clean.clone();
    tokio::spawn(async move {
        let tx_progress = tx.clone();
        let ver_progress = ver_clone.clone();
        let res =
            browser::download_stealth_runtime_with_progress(false, Some(&ver_clone), move |ev| {
                let _ = tx_progress.send(
                    json!({
                        "type": "driver_download_progress",
                        "version": ver_progress,
                        "stage": ev.stage,
                        "percent": ev.percent,
                        "downloadedBytes": ev.downloaded_bytes,
                        "totalBytes": ev.total_bytes
                    })
                    .to_string(),
                );
            })
            .await;

        match res {
            Ok(_) => {
                let _ = tx.send(
                    json!({
                        "type": "driver_download_progress",
                        "version": ver_clone,
                        "stage": "ready",
                        "percent": 100,
                        "downloadedBytes": 0,
                        "totalBytes": 0
                    })
                    .to_string(),
                );
            }
            Err(err) => {
                let _ = tx.send(
                    json!({
                        "type": "driver_download_progress",
                        "version": ver_clone,
                        "stage": "error",
                        "percent": 0,
                        "downloadedBytes": 0,
                        "totalBytes": 0,
                        "error": err.to_string()
                    })
                    .to_string(),
                );
            }
        }
    });

    Ok(Json(DriverActionResponse {
        success: true,
        version: clean,
        message: "Driver installation started".to_string(),
    }))
}

#[utoipa::path(
    tag = "Browsers",
    post,
    path = "/api/v1/browsers/drivers/{version}/use",
    operation_id = "use_browser_driver",
    summary = "Select active browser driver version",
    params(("version" = String, Path, description = "Installed driver version")),
    responses(
        (status = 200, description = "Active driver updated", body = DriverActionResponse),
        (status = 404, description = "Driver version not installed", body = crate::core::error::ApiErrorResponse)
    )
)]
pub async fn use_driver(
    State(state): State<AppState>,
    Path(version): Path<String>,
) -> Result<Json<DriverActionResponse>, AutomationError> {
    let installed = browser::list_installed_runtimes();
    if !installed.iter().any(|i| i.version == version) {
        return Err(AutomationError::NotFound(format!(
            "Driver version '{}' is not installed",
            version
        )));
    }
    browser::set_active_version(&version).map_err(|e| AutomationError::Internal(e.to_string()))?;

    let _ = state.tx.send(
        json!({
            "type": "driver_changed",
            "action": "selected",
            "version": version,
        })
        .to_string(),
    );

    Ok(Json(DriverActionResponse {
        success: true,
        version,
        message: "Active browser driver updated".to_string(),
    }))
}

#[utoipa::path(
    tag = "Browsers",
    delete,
    path = "/api/v1/browsers/drivers/{version}",
    operation_id = "delete_browser_driver",
    summary = "Delete an installed browser driver version",
    params(("version" = String, Path, description = "Installed driver version")),
    responses(
        (status = 200, description = "Driver deleted", body = DriverActionResponse),
        (status = 404, description = "Driver version not installed", body = crate::core::error::ApiErrorResponse),
        (status = 409, description = "Cannot delete selected driver or while sessions are running", body = crate::core::error::ApiErrorResponse)
    )
)]
pub async fn delete_driver(
    State(state): State<AppState>,
    Path(version): Path<String>,
) -> Result<Json<DriverActionResponse>, AutomationError> {
    if !browser_registry().read().await.is_empty() {
        return Err(AutomationError::Conflict(
            "Stop all running browser sessions before deleting a driver".to_string(),
        ));
    }

    let response = delete_installed_driver_in_dir(
        version,
        &browser::get_active_version(),
        browser::list_installed_runtimes(),
        &browser::resolve_data_dir(),
    )
    .await?;

    let _ = state.tx.send(
        json!({
            "type": "driver_changed",
            "action": "uninstalled",
            "version": response.version,
        })
        .to_string(),
    );

    Ok(Json(response))
}

fn resolve_runtime_dir(
    data_dir: &std::path::Path,
    runtime_id: &str,
) -> Result<std::path::PathBuf, AutomationError> {
    let mut components = std::path::Path::new(runtime_id).components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) => {
            Ok(data_dir.join("runtimes").join(runtime_id))
        }
        _ => Err(AutomationError::Internal(format!(
            "Invalid installed runtime directory id '{}'",
            runtime_id
        ))),
    }
}

async fn delete_installed_driver_in_dir(
    version: String,
    active_version: &str,
    installed: Vec<browser::InstalledRuntimeInfo>,
    data_dir: &std::path::Path,
) -> Result<DriverActionResponse, AutomationError> {
    if version == active_version {
        return Err(AutomationError::Conflict(
            "Cannot delete the currently selected browser driver".to_string(),
        ));
    }

    let target = installed
        .into_iter()
        .find(|i| i.version == version)
        .ok_or_else(|| {
            AutomationError::NotFound(format!("Driver version '{}' is not installed", version))
        })?;

    if target.is_active {
        return Err(AutomationError::Conflict(
            "Cannot delete the currently selected browser driver".to_string(),
        ));
    }

    // `target.path` points to the executable file (e.g. `.../chrome.exe`), so
    // remove the containing runtime folder under `runtimes/<id>`.
    let runtime_dir = resolve_runtime_dir(data_dir, &target.id)?;
    tokio::fs::remove_dir_all(&runtime_dir)
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => AutomationError::Conflict(
                "Browser driver files are in use; close any open browser windows and try again"
                    .to_string(),
            ),
            _ => AutomationError::Internal(e.to_string()),
        })?;

    Ok(DriverActionResponse {
        success: true,
        version,
        message: "Browser driver deleted".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use browser::InstalledRuntimeInfo;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("operon-driver-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sample_runtime(
        data_dir: &std::path::Path,
        id: &str,
        version: &str,
        is_active: bool,
    ) -> InstalledRuntimeInfo {
        let exe_path = data_dir.join("runtimes").join(id).join("chrome.exe");
        InstalledRuntimeInfo {
            id: id.to_string(),
            name: format!("Operon Chromium ({})", version),
            version: version.to_string(),
            major_version: "148".to_string(),
            channel: "Stable".to_string(),
            status_badge: "Installed".to_string(),
            path: exe_path.to_string_lossy().to_string(),
            is_active,
            size_mb: Some(120.0),
        }
    }

    #[tokio::test]
    async fn delete_driver_removes_runtime_directory_even_when_path_is_exe() {
        let tmp = TempDir::new();
        let runtime_dir = tmp.0.join("runtimes").join("v148.0.7778.215");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        std::fs::write(runtime_dir.join("chrome.exe"), b"stub").unwrap();

        let installed = vec![sample_runtime(
            &tmp.0,
            "v148.0.7778.215",
            "148.0.7778.215",
            false,
        )];

        let res = delete_installed_driver_in_dir(
            "148.0.7778.215".to_string(),
            "150.0.7871.186",
            installed,
            &tmp.0,
        )
        .await
        .expect("inactive installed driver should delete cleanly");

        assert!(res.success);
        assert_eq!(res.version, "148.0.7778.215");
        assert!(!runtime_dir.exists());
    }

    #[tokio::test]
    async fn delete_driver_rejects_active_or_missing_or_traversal_id() {
        let tmp = TempDir::new();
        let installed = vec![
            sample_runtime(&tmp.0, "v150.0.7871.186", "150.0.7871.186", true),
            sample_runtime(&tmp.0, "../outside", "147.0.0.1", false),
        ];

        let active_err = delete_installed_driver_in_dir(
            "150.0.7871.186".to_string(),
            "150.0.7871.186",
            installed.clone(),
            &tmp.0,
        )
        .await
        .unwrap_err();
        assert!(matches!(active_err, AutomationError::Conflict(_)));

        let missing_err = delete_installed_driver_in_dir(
            "144.0.0.1".to_string(),
            "150.0.7871.186",
            installed.clone(),
            &tmp.0,
        )
        .await
        .unwrap_err();
        assert!(matches!(missing_err, AutomationError::NotFound(_)));

        let traversal_err = delete_installed_driver_in_dir(
            "147.0.0.1".to_string(),
            "150.0.7871.186",
            installed,
            &tmp.0,
        )
        .await
        .unwrap_err();
        assert!(matches!(traversal_err, AutomationError::Internal(_)));
    }
}
