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

    let _ = state
        .tx
        .send(json!({ "type": "settings_updated" }).to_string());

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
    Path(version): Path<String>,
) -> Result<Json<DriverActionResponse>, AutomationError> {
    if !browser_registry().read().await.is_empty() {
        return Err(AutomationError::Conflict(
            "Stop all running browser sessions before deleting a driver".to_string(),
        ));
    }
    if version == browser::get_active_version() {
        return Err(AutomationError::Conflict(
            "Cannot delete the currently selected browser driver".to_string(),
        ));
    }

    let installed = browser::list_installed_runtimes();
    let target = installed
        .into_iter()
        .find(|i| i.version == version)
        .ok_or_else(|| {
            AutomationError::NotFound(format!("Driver version '{}' is not installed", version))
        })?;

    tokio::fs::remove_dir_all(&target.path)
        .await
        .map_err(|e| AutomationError::Internal(e.to_string()))?;

    Ok(Json(DriverActionResponse {
        success: true,
        version,
        message: "Browser driver deleted".to_string(),
    }))
}
