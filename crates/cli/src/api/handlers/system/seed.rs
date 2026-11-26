use crate::AppState;
use crate::core::browser::profile_config::resolve_workspace_id;
use crate::core::error::AutomationError;
use crate::infrastructure::db::{SeedSummary, reset_and_seed};
use axum::extract::{Json, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResetDbRequest {
    pub workspace_id: Option<String>,
    pub clear_all: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResetDbResponse {
    pub success: bool,
    pub message: String,
    pub summary: SeedSummary,
}

#[utoipa::path(
    tag = "System",
    post,
    path = "/api/v1/system/db/reset",
    operation_id = "reset_and_seed_db",
    summary = "Reset local SQLite tables and re-seed acceptance fixtures",
    request_body = ResetDbRequest,
    responses(
        (status = 200, description = "Database reset and seeded", body = ResetDbResponse),
        (status = 400, description = "Invalid workspaceId", body = crate::core::error::ApiErrorResponse)
    )
)]
pub async fn reset_db(
    State(state): State<AppState>,
    body: Option<Json<ResetDbRequest>>,
) -> Result<Json<ResetDbResponse>, AutomationError> {
    let req = body.map(|Json(v)| v).unwrap_or_default();
    let workspace_id = resolve_workspace_id(req.workspace_id.as_deref())?;
    let clear_all = req.clear_all.unwrap_or(false);

    let summary = {
        let db = state.db.lock().await;
        reset_and_seed(&db, &workspace_id, clear_all)
            .map_err(|e| AutomationError::DatabaseError(e.to_string()))?
    };

    let _ = state.tx.send(
        json!({ "type": "browser_changed", "action": "reset", "workspaceId": workspace_id })
            .to_string(),
    );
    let _ = state.tx.send(
        json!({ "type": "task_changed", "action": "reset", "workspaceId": workspace_id })
            .to_string(),
    );
    let _ = state
        .tx
        .send(json!({ "type": "settings_updated", "workspaceId": workspace_id }).to_string());

    Ok(Json(ResetDbResponse {
        success: true,
        message: format!(
            "Reset and seeded {} browsers, {} tasks, {} jobs for workspace '{}'",
            summary.browsers, summary.tasks, summary.jobs, summary.workspace_id
        ),
        summary,
    }))
}
