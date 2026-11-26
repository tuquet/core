use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;
use crate::infrastructure::cloud::{
    CloudApiClient, CredentialStore, WorkspaceInfo, WorkspaceManager,
};

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "success": true,
    "activeWorkspaceId": "uuid-1234",
    "activeWorkspaceSlug": "default-workspace",
    "activeWorkspaceName": "Default Workspace",
    "workspaces": [
        {
            "id": "uuid-1234",
            "slug": "default-workspace",
            "name": "Default Workspace",
            "role": "owner",
            "status": "active",
            "isActive": true
        }
    ]
}))]
/// Response descriptor containing current and accessible workspaces
pub struct WorkspaceListResponse {
    /// Whether query succeeded
    pub success: bool,
    /// Currently active workspace ID
    pub active_workspace_id: Option<String>,
    /// Currently active workspace vanity slug
    pub active_workspace_slug: Option<String>,
    /// Currently active workspace display name
    pub active_workspace_name: Option<String>,
    /// List of all accessible workspaces
    pub workspaces: Vec<WorkspaceInfo>,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"target": "alpha-corp"}))]
/// Request payload to switch current active workspace
pub struct SwitchWorkspaceRequest {
    /// Workspace identifier UUID or vanity slug to switch into
    pub target: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "success": true,
    "message": "Switched active workspace to 'alpha-corp'",
    "activeWorkspace": {
        "id": "uuid-1234",
        "slug": "alpha-corp",
        "name": "Alpha Corp",
        "role": "owner",
        "status": "active",
        "isActive": true
    }
}))]
/// Response returned after successfully switching active workspace
pub struct SwitchWorkspaceResponse {
    /// Whether switch succeeded
    pub success: bool,
    /// Status confirmation message
    pub message: String,
    /// Details of newly selected active workspace
    pub active_workspace: Option<WorkspaceInfo>,
}

#[utoipa::path(
    tag = "System",
    get,
    path = "/api/v1/system/workspaces",
    operation_id = "get_system_workspaces",
    summary = "List all accessible workspaces and active workspace",
    description = "Queries cached and cloud-synced workspaces for the authenticated operator without exposing cloud keys to frontend.",
    responses(
        (status = 200, description = "List of accessible workspaces", body = WorkspaceListResponse),
        (status = 401, description = "Desktop not enrolled or unauthenticated", body = crate::core::error::ApiErrorResponse)
    )
)]
pub async fn get_workspaces(State(state): State<AppState>) -> impl IntoResponse {
    let creds = match CredentialStore::load(&state.config.data_dir).await {
        Some(c) => c,
        None => {
            return (
                StatusCode::OK,
                Json(WorkspaceListResponse {
                    success: false,
                    active_workspace_id: None,
                    active_workspace_slug: None,
                    active_workspace_name: None,
                    workspaces: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let cloud_url = match crate::infrastructure::cloud::resolve_supabase_url(
        state.config.cloud_url.as_deref(),
        creds.cloud_url.as_deref(),
    ) {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(crate::core::error::ApiErrorResponse {
                    error: e,
                    status: 400,
                }),
            )
                .into_response();
        }
    };
    let api_key =
        match crate::infrastructure::cloud::resolve_supabase_anon_key(creds.api_key.as_deref()) {
            Ok(k) => k,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(crate::core::error::ApiErrorResponse {
                        error: e,
                        status: 400,
                    }),
                )
                    .into_response();
            }
        };

    let client = CloudApiClient::build_http_client();
    let user_id = creds.user_id.clone().unwrap_or_default();

    let mut workspaces = if let Some(ref access_token) = creds.access_token {
        WorkspaceManager::fetch_user_workspaces(
            &client,
            &cloud_url,
            &api_key,
            access_token,
            &user_id,
        )
        .await
        .unwrap_or_else(|_| creds.available_workspaces.clone().unwrap_or_default())
    } else {
        creds.available_workspaces.clone().unwrap_or_default()
    };

    let active_id = creds.workspace_id.as_deref().unwrap_or("");
    let active_slug = creds.workspace_slug.as_deref().unwrap_or("");

    for t in workspaces.iter_mut() {
        t.is_active = (!active_id.is_empty() && t.id == active_id)
            || (!active_slug.is_empty() && t.slug.eq_ignore_ascii_case(active_slug));
    }

    (
        StatusCode::OK,
        Json(WorkspaceListResponse {
            success: true,
            active_workspace_id: creds.workspace_id,
            active_workspace_slug: creds.workspace_slug,
            active_workspace_name: creds.workspace_name,
            workspaces,
        }),
    )
        .into_response()
}

#[utoipa::path(
    tag = "System",
    post,
    path = "/api/v1/system/workspaces/switch",
    operation_id = "switch_system_workspace",
    summary = "Switch active workspace",
    description = "Re-enrolls this runner device under the target workspace, updates local credentials, and emits a workspace_switched event to SSE.",
    request_body = SwitchWorkspaceRequest,
    responses(
        (status = 200, description = "Successfully switched active workspace", body = SwitchWorkspaceResponse),
        (status = 400, description = "Invalid workspace ID or slug", body = crate::core::error::ApiErrorResponse),
        (status = 500, description = "Failed to switch workspace", body = crate::core::error::ApiErrorResponse)
    )
)]
pub async fn switch_workspace(
    State(state): State<AppState>,
    Json(payload): Json<SwitchWorkspaceRequest>,
) -> impl IntoResponse {
    // Allow local workspace switching seamlessly when operating without Cloud enrollment
    if payload.target.trim().starts_with("local-") {
        let local_id = payload.target.trim().to_string();
        let active_workspace = WorkspaceInfo {
            id: local_id.clone(),
            slug: local_id.clone(),
            name: if local_id == crate::infrastructure::db::DEFAULT_WORKSPACE_ID {
                "Local Workspace".to_string()
            } else {
                local_id.clone()
            },
            role: Some("owner".to_string()),
            status: Some("active".to_string()),
            is_active: true,
        };
        let _ = state.tx.send(
            serde_json::json!({
                "type": "workspace_switched",
                "workspaceId": active_workspace.id,
                "workspaceSlug": active_workspace.slug,
                "workspaceName": active_workspace.name
            })
            .to_string(),
        );
        return (
            StatusCode::OK,
            Json(SwitchWorkspaceResponse {
                success: true,
                message: format!("Switched active workspace to '{}'", active_workspace.name),
                active_workspace: Some(active_workspace),
            }),
        )
            .into_response();
    }

    match WorkspaceManager::switch_workspace(&state.config.data_dir, &payload.target).await {
        Ok(active_workspace) => {
            // Broadcast workspace switch event to global SSE stream
            let _ = state.tx.send(
                serde_json::json!({
                    "type": "workspace_switched",
                    "workspaceId": active_workspace.id,
                    "workspaceSlug": active_workspace.slug,
                    "workspaceName": active_workspace.name
                })
                .to_string(),
            );

            (
                StatusCode::OK,
                Json(SwitchWorkspaceResponse {
                    success: true,
                    message: format!("Switched active workspace to '{}'", active_workspace.name),
                    active_workspace: Some(active_workspace),
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(SwitchWorkspaceResponse {
                success: false,
                message: format!("Failed to switch workspace: {}", e),
                active_workspace: None,
            }),
        )
            .into_response(),
    }
}
