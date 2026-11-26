use crate::AppState;
use crate::core::browser::profile_config::resolve_workspace_id;
use crate::core::error::AutomationError;
use crate::infrastructure::db::{
    BulkUpdateTasksRecord, CreateTaskRecord, TaskRecord, UpdateTaskRecord,
};
use axum::extract::{Json, Path, Query, State};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};

fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

fn validate_task_title(raw: &str) -> Result<String, AutomationError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AutomationError::BadRequest(
            "Task title cannot be empty".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn normalize_optional_browser_id(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn ensure_browser_in_workspace(
    db: &crate::infrastructure::db::AutomationDb,
    browser_id: Option<&str>,
    workspace_id: &str,
) -> Result<(), AutomationError> {
    if let Some(bid) = browser_id {
        let browser = db
            .browsers()
            .get_browser(bid)
            .map_err(|e| AutomationError::DatabaseError(e.to_string()))?
            .ok_or_else(|| {
                AutomationError::NotFound(format!("Browser profile '{}' not found", bid))
            })?;
        if browser.workspace_id != workspace_id {
            return Err(AutomationError::Conflict(format!(
                "Browser '{}' belongs to workspace '{}', cannot link to task in workspace '{}'",
                bid, browser.workspace_id, workspace_id
            )));
        }
    }
    Ok(())
}

fn emit_task_event(state: &AppState, action: &str, id: Option<&str>) {
    let _ = state.tx.send(
        json!({
            "type": "task_changed",
            "action": action,
            "id": id,
        })
        .to_string(),
    );
}

#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct GetTasksQuery {
    pub workspace_id: Option<String>,
    pub browser_id: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub id: Option<String>,
    pub workspace_id: Option<String>,
    pub browser_id: Option<String>,
    pub title: String,
    pub status: Option<String>,
    pub label: Option<String>,
    pub priority: Option<String>,
    pub assignee: Option<String>,
    pub favorite: Option<bool>,
    pub progress: Option<i64>,
}

#[derive(Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PatchTaskRequest {
    #[serde(default, deserialize_with = "deserialize_some")]
    #[schema(value_type = Option<String>)]
    pub browser_id: Option<Option<String>>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub label: Option<String>,
    pub priority: Option<String>,
    pub assignee: Option<String>,
    pub favorite: Option<bool>,
    pub progress: Option<i64>,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkUpdateTasksRequest {
    pub ids: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    #[schema(value_type = Option<String>)]
    pub browser_id: Option<Option<String>>,
    pub status: Option<String>,
    pub label: Option<String>,
    pub priority: Option<String>,
    pub assignee: Option<String>,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchDeleteTasksRequest {
    pub ids: Vec<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskMutationCountResponse {
    pub count: usize,
}

#[utoipa::path(
    tag = "Tasks",
    get,
    path = "/api/v1/tasks",
    operation_id = "list_tasks",
    summary = "List tasks linked with browser profiles",
    params(GetTasksQuery),
    responses(
        (status = 200, description = "List of SQLite tasks joined with browser names", body = Vec<TaskRecord>),
        (status = 500, description = "Database query failure", body = crate::core::error::ApiErrorResponse)
    )
)]
pub async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<GetTasksQuery>,
) -> Result<Json<Vec<TaskRecord>>, AutomationError> {
    let db = state.db.lock().await;
    let items = db
        .tasks()
        .list_tasks(
            query.workspace_id.as_deref(),
            query.browser_id.as_deref(),
            query.status.as_deref(),
            query.search.as_deref(),
        )
        .map_err(|e| AutomationError::DatabaseError(e.to_string()))?;
    Ok(Json(items))
}

#[utoipa::path(
    tag = "Tasks",
    post,
    path = "/api/v1/tasks",
    operation_id = "create_task",
    summary = "Create a task optionally linked to a browser profile",
    request_body = CreateTaskRequest,
    responses(
        (status = 200, description = "Task created", body = TaskRecord),
        (status = 400, description = "Invalid task input", body = crate::core::error::ApiErrorResponse),
        (status = 404, description = "Linked browser profile not found", body = crate::core::error::ApiErrorResponse)
    )
)]
pub async fn create_task(
    State(state): State<AppState>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<Json<TaskRecord>, AutomationError> {
    let title = validate_task_title(&req.title)?;
    let workspace_id = resolve_workspace_id(req.workspace_id.as_deref())?;
    let browser_id = normalize_optional_browser_id(req.browser_id.as_deref());
    let task_id = req
        .id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "TASK-{}",
                uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
            )
        });

    let created = {
        let db = state.db.lock().await;
        ensure_browser_in_workspace(&db, browser_id.as_deref(), &workspace_id)?;
        db.tasks()
            .create_task(&CreateTaskRecord {
                id: &task_id,
                workspace_id: &workspace_id,
                browser_id: browser_id.as_deref(),
                title: &title,
                status: req.status.as_deref().unwrap_or("todo"),
                label: req.label.as_deref().unwrap_or("feature"),
                priority: req.priority.as_deref().unwrap_or("medium"),
                assignee: req.assignee.as_deref().unwrap_or("linh"),
                favorite: req.favorite.unwrap_or(false),
                progress: req.progress.unwrap_or(0),
            })
            .map_err(|e| AutomationError::DatabaseError(e.to_string()))?
    };

    emit_task_event(&state, "create", Some(&created.id));
    Ok(Json(created))
}

#[utoipa::path(
    tag = "Tasks",
    patch,
    path = "/api/v1/tasks/{id}",
    operation_id = "patch_task",
    summary = "Partially update a task or its linked browser profile",
    params(("id" = String, Path, description = "Task identifier")),
    request_body = PatchTaskRequest,
    responses(
        (status = 200, description = "Task updated", body = TaskRecord),
        (status = 404, description = "Task or linked browser profile not found", body = crate::core::error::ApiErrorResponse)
    )
)]
pub async fn patch_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchTaskRequest>,
) -> Result<Json<TaskRecord>, AutomationError> {
    let title = match req.title.as_deref() {
        Some(t) => Some(validate_task_title(t)?),
        None => None,
    };
    let next_browser: Option<Option<String>> = req
        .browser_id
        .map(|opt| normalize_optional_browser_id(opt.as_deref()));

    let updated = {
        let db = state.db.lock().await;
        let existing = db
            .tasks()
            .get_task(&id)
            .map_err(|e| AutomationError::DatabaseError(e.to_string()))?
            .ok_or_else(|| AutomationError::NotFound(format!("Task '{}' not found", id)))?;
        if let Some(Some(ref bid)) = next_browser {
            ensure_browser_in_workspace(&db, Some(bid.as_str()), &existing.workspace_id)?;
        }
        let browser_ref: Option<Option<&str>> = next_browser.as_ref().map(|o| o.as_deref());
        db.tasks()
            .update_task(
                &id,
                &UpdateTaskRecord {
                    browser_id: browser_ref,
                    title: title.as_deref(),
                    status: req.status.as_deref(),
                    label: req.label.as_deref(),
                    priority: req.priority.as_deref(),
                    assignee: req.assignee.as_deref(),
                    favorite: req.favorite,
                    progress: req.progress,
                },
            )
            .map_err(|e| AutomationError::DatabaseError(e.to_string()))?
            .ok_or_else(|| AutomationError::NotFound(format!("Task '{}' not found", id)))?
    };

    emit_task_event(&state, "patch", Some(&updated.id));
    Ok(Json(updated))
}

#[utoipa::path(
    tag = "Tasks",
    post,
    path = "/api/v1/tasks/bulk",
    operation_id = "bulk_update_tasks",
    summary = "Bulk update tasks (including assigning a browser profile)",
    request_body = BulkUpdateTasksRequest,
    responses(
        (status = 200, description = "Updated task count", body = TaskMutationCountResponse),
        (status = 404, description = "Linked browser profile not found", body = crate::core::error::ApiErrorResponse)
    )
)]
pub async fn bulk_update_tasks(
    State(state): State<AppState>,
    Json(req): Json<BulkUpdateTasksRequest>,
) -> Result<Json<TaskMutationCountResponse>, AutomationError> {
    let next_browser: Option<Option<String>> = req
        .browser_id
        .map(|opt| normalize_optional_browser_id(opt.as_deref()));

    let count = {
        let db = state.db.lock().await;
        if let Some(Some(ref bid)) = next_browser {
            let browser = db
                .browsers()
                .get_browser(bid)
                .map_err(|e| AutomationError::DatabaseError(e.to_string()))?
                .ok_or_else(|| {
                    AutomationError::NotFound(format!("Browser profile '{}' not found", bid))
                })?;
            for task_id in &req.ids {
                if let Some(task) = db
                    .tasks()
                    .get_task(task_id)
                    .map_err(|e| AutomationError::DatabaseError(e.to_string()))?
                    && task.workspace_id != browser.workspace_id
                {
                    return Err(AutomationError::Conflict(format!(
                        "Task '{}' belongs to workspace '{}', cannot link to browser '{}' in workspace '{}'",
                        task_id, task.workspace_id, bid, browser.workspace_id
                    )));
                }
            }
        }
        let browser_ref: Option<Option<&str>> = next_browser.as_ref().map(|o| o.as_deref());
        db.tasks()
            .bulk_update(
                &req.ids,
                &BulkUpdateTasksRecord {
                    browser_id: browser_ref,
                    status: req.status.as_deref(),
                    label: req.label.as_deref(),
                    priority: req.priority.as_deref(),
                    assignee: req.assignee.as_deref(),
                },
            )
            .map_err(|e| AutomationError::DatabaseError(e.to_string()))?
    };

    emit_task_event(&state, "bulk_update", None);
    Ok(Json(TaskMutationCountResponse { count }))
}

#[utoipa::path(
    tag = "Tasks",
    post,
    path = "/api/v1/tasks/batch/delete",
    operation_id = "batch_delete_tasks",
    summary = "Delete multiple tasks in a single transaction",
    request_body = BatchDeleteTasksRequest,
    responses(
        (status = 200, description = "Deleted task count", body = TaskMutationCountResponse)
    )
)]
pub async fn batch_delete_tasks(
    State(state): State<AppState>,
    Json(req): Json<BatchDeleteTasksRequest>,
) -> Result<Json<TaskMutationCountResponse>, AutomationError> {
    let count = {
        let db = state.db.lock().await;
        db.tasks()
            .delete_tasks(&req.ids)
            .map_err(|e| AutomationError::DatabaseError(e.to_string()))?
    };

    emit_task_event(&state, "delete", None);
    Ok(Json(TaskMutationCountResponse { count }))
}

#[utoipa::path(
    tag = "Tasks",
    delete,
    path = "/api/v1/tasks/{id}",
    operation_id = "delete_task",
    summary = "Delete a single task",
    params(("id" = String, Path, description = "Task identifier")),
    responses(
        (status = 200, description = "Deleted task count", body = TaskMutationCountResponse)
    )
)]
pub async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TaskMutationCountResponse>, AutomationError> {
    let count = {
        let db = state.db.lock().await;
        db.tasks()
            .delete_tasks(std::slice::from_ref(&id))
            .map_err(|e| AutomationError::DatabaseError(e.to_string()))?
    };

    emit_task_event(&state, "delete", Some(&id));
    Ok(Json(TaskMutationCountResponse { count }))
}
