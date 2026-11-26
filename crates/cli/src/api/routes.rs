use crate::api::handlers::browsers::{
    auto_detect_browsers, batch_delete_browsers, clone_browser, create_browser, delete_browser,
    get_browser_detail, get_browsers, import_csv, move_browser, patch_browser, probe_browser_proxy,
    set_default_browser, sideload_extension, start_browser, stop_browser, unbind_browser,
    update_browser, update_browser_extensions,
};
use crate::api::handlers::drivers::{delete_driver, install_driver, list_drivers, use_driver};
use crate::api::handlers::health::health;
use crate::api::handlers::history::{clear_history, delete_history_item, get_history, get_logs};
use crate::api::handlers::jobs::{
    get_active_jobs, get_job_status, job_finish, job_log, kill_job, pause_job, resume_job,
    submit_job, worker_sse,
};
use crate::api::handlers::lint::lint_workflow;
use crate::api::handlers::secrets::{
    delete_secret, get_secrets, list_credentials, pull_secrets, push_secrets, rotate_secrets,
    set_secret,
};
use crate::api::handlers::settings::{get_settings, patch_settings, update_settings};
use crate::api::handlers::sse::sse;
use crate::api::handlers::system::{
    cloud_login, cloud_logout, desktop_auth_callback, desktop_auth_callback_post, get_ci_logs,
    get_cloud_status, get_metrics, get_system_info, get_workspaces, kill_browsers, open_studio,
    post_ci_run, reset_db, start_desktop_auth, switch_workspace, trigger_cloud_sync,
};
use crate::api::handlers::tasks::{
    batch_delete_tasks, bulk_update_tasks, create_task, delete_task, list_tasks, patch_task,
};
use crate::api::handlers::ws::ws_handler;
use axum::{
    Router, response::IntoResponse, routing::delete, routing::get, routing::patch, routing::post,
};
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::api::handlers::jobs::submit_job,
        crate::api::handlers::jobs::get_job_status,
        crate::api::handlers::jobs::get_active_jobs,
        crate::api::handlers::jobs::kill_job,
        crate::api::handlers::jobs::pause_job,
        crate::api::handlers::jobs::resume_job,
        crate::api::handlers::jobs::worker_sse,
        crate::api::handlers::jobs::job_log,
        crate::api::handlers::jobs::job_finish,
        crate::api::handlers::history::get_history,
        crate::api::handlers::history::get_logs,
        crate::api::handlers::history::delete_history_item,
        crate::api::handlers::history::clear_history,
        crate::api::handlers::health::health,
        crate::api::handlers::sse::sse,
        crate::api::handlers::system::kill_browsers,
        crate::api::handlers::system::open_studio,
        crate::api::handlers::system::get_metrics,
        crate::api::handlers::system::get_ci_logs,
        crate::api::handlers::system::post_ci_run,
        crate::api::handlers::system::get_system_info,
        crate::api::handlers::system::get_cloud_status,
        crate::api::handlers::system::trigger_cloud_sync,
        crate::api::handlers::system::cloud_login,
        crate::api::handlers::system::cloud_logout,
        crate::api::handlers::system::get_workspaces,
        crate::api::handlers::system::switch_workspace,
        crate::api::handlers::system::reset_db,
        crate::api::handlers::system::start_desktop_auth,
        crate::api::handlers::system::desktop_auth_callback,
        crate::api::handlers::system::desktop_auth_callback_post,
        crate::api::handlers::lint::lint_workflow,
        crate::api::handlers::secrets::get_secrets,
        crate::api::handlers::secrets::list_credentials,
        crate::api::handlers::secrets::set_secret,
        crate::api::handlers::secrets::delete_secret,
        crate::api::handlers::secrets::push_secrets,
        crate::api::handlers::secrets::pull_secrets,
        crate::api::handlers::secrets::rotate_secrets,
        crate::api::handlers::browsers::get_browsers,
        crate::api::handlers::browsers::get_browser_detail,
        crate::api::handlers::browsers::create_browser,
        crate::api::handlers::browsers::update_browser,
        crate::api::handlers::browsers::patch_browser,
        crate::api::handlers::browsers::delete_browser,
        crate::api::handlers::browsers::clone_browser,
        crate::api::handlers::browsers::move_browser,
        crate::api::handlers::browsers::unbind_browser,
        crate::api::handlers::browsers::probe_browser_proxy,
        crate::api::handlers::browsers::batch_delete_browsers,
        crate::api::handlers::browsers::start_browser,
        crate::api::handlers::browsers::stop_browser,
        crate::api::handlers::settings::get_settings,
        crate::api::handlers::settings::update_settings,
        crate::api::handlers::settings::patch_settings,
        crate::api::handlers::browsers::import_csv,
        crate::api::handlers::browsers::sideload_extension,
        crate::api::handlers::browsers::update_browser_extensions,
        crate::api::handlers::browsers::auto_detect_browsers,
        crate::api::handlers::browsers::set_default_browser,
        crate::api::handlers::drivers::list_drivers,
        crate::api::handlers::drivers::install_driver,
        crate::api::handlers::drivers::use_driver,
        crate::api::handlers::drivers::delete_driver,
        crate::api::handlers::tasks::list_tasks,
        crate::api::handlers::tasks::create_task,
        crate::api::handlers::tasks::patch_task,
        crate::api::handlers::tasks::bulk_update_tasks,
        crate::api::handlers::tasks::batch_delete_tasks,
        crate::api::handlers::tasks::delete_task,
        crate::api::handlers::ws::ws_handler
    ),
    info(
        title = "CLI & Core Gateway API",
        version = "1.0.0",
        description = "Embedded Backend for Frontend (BFF) Gateway, WebSocket/SSE event hub, and SQLite storage gateway for the Ecosystem.",
        contact(
            name = "Ecosystem",
            url = "https://github.com/operon/cli"
        ),
        license(
            name = "MIT"
        )
    ),
    servers(
        (url = "http://127.0.0.1:8765", description = "Local Daemon Gateway Server")
    ),
    components(
        schemas(
            crate::core::error::ApiErrorResponse,
            crate::api::handlers::health::HealthResponse,
            crate::api::handlers::system::WorkspaceListResponse,
            crate::api::handlers::system::SwitchWorkspaceRequest,
            crate::api::handlers::system::SwitchWorkspaceResponse,
            crate::infrastructure::cloud::WorkspaceInfo,
            crate::api::handlers::system::CloudStatusResponse,
            crate::api::handlers::system::CloudSyncResponse,
            crate::api::handlers::system::CloudLoginRequest,
            crate::api::handlers::system::CloudLoginResponse,
            crate::api::handlers::system::CloudLogoutResponse,
            crate::api::handlers::system::SystemMetricsResponse,
            crate::api::handlers::system::CiCacheEntry,
            crate::api::handlers::system::CiLogFileEntry,
            crate::api::handlers::system::CiLogsResponse,
            crate::api::handlers::system::RunnerIdentityResponse,
            crate::api::handlers::system::SystemResponse,
            crate::infrastructure::db::SeedSummary,
            crate::api::handlers::system::ResetDbRequest,
            crate::api::handlers::system::ResetDbResponse,
            crate::infrastructure::db::FingerprintConfig,
            crate::core::browser::importer::CsvImportError,
            crate::api::handlers::browsers::BrowserResponse,
            crate::api::handlers::browsers::CreateBrowserRequest,
            crate::api::handlers::browsers::CreateBrowserResponse,
            crate::api::handlers::browsers::UpdateBrowserRequest,
            crate::api::handlers::browsers::UpdateBrowserResponse,
            crate::api::handlers::browsers::PatchBrowserRequest,
            crate::api::handlers::browsers::CloneBrowserRequest,
            crate::api::handlers::browsers::MoveBrowserRequest,
            crate::api::handlers::browsers::UnbindBrowserResponse,
            crate::api::handlers::browsers::ProbeProxyRequest,
            crate::api::handlers::browsers::ProbeProxyResponse,
            crate::api::handlers::browsers::BatchDeleteBrowsersRequest,
            crate::api::handlers::browsers::BatchDeleteFailure,
            crate::api::handlers::browsers::BatchDeleteBrowsersResponse,
            crate::api::handlers::browsers::UpdateExtensionsRequest,
            crate::api::handlers::browsers::DeleteBrowserResponse,
            crate::api::handlers::browsers::StartBrowserSessionRequest,
            crate::api::handlers::browsers::StartBrowserSessionResponse,
            crate::api::handlers::browsers::StopBrowserResponse,
            crate::api::handlers::browsers::SetDefaultBrowserResponse,
            crate::api::handlers::drivers::DriverReleaseItem,
            crate::api::handlers::drivers::ListDriversResponse,
            crate::api::handlers::drivers::DriverActionResponse,
            crate::api::handlers::browsers::ImportCsvPayload,
            crate::api::handlers::browsers::ImportCsvResponse,
            crate::api::handlers::browsers::SideloadExtensionPayload,
            crate::api::handlers::browsers::SideloadExtensionResponse,
            crate::api::handlers::jobs::SubmitJobPayload,
            crate::api::handlers::jobs::SubmitJobResponse,
            crate::api::handlers::jobs::ActiveJobResponse,
            crate::api::handlers::jobs::JobStatusResponse,
            crate::api::handlers::jobs::JobControlResponse,
            crate::api::handlers::jobs::JobLogPayload,
            crate::api::handlers::history::HistoryActionResponse,
            crate::api::handlers::lint::LintWorkflowRequest,
            crate::api::handlers::lint::LintWorkflowResponse,
            crate::api::handlers::lint::LintIssue,
            crate::api::handlers::secrets::SecretRow,
        crate::api::handlers::secrets::SecretStoreStatus,
        crate::api::handlers::secrets::CredentialList,
        crate::api::handlers::secrets::SetSecretRequest,
        crate::api::handlers::secrets::SecretOpResult,
            crate::infrastructure::db::TaskRecord,
            crate::api::handlers::tasks::CreateTaskRequest,
            crate::api::handlers::tasks::PatchTaskRequest,
            crate::api::handlers::tasks::BulkUpdateTasksRequest,
            crate::api::handlers::tasks::BatchDeleteTasksRequest,
            crate::api::handlers::tasks::TaskMutationCountResponse,
            crate::core::models::settings::AppSettings,
            crate::core::models::settings::UpdateAppSettingsRequest
        )
    ),
    tags(
        (name = "Health", description = "System health check endpoints"),
        (name = "Jobs", description = "Workflow job dispatching and execution telemetry endpoints"),
        (name = "Tasks", description = "SQLite automation tasks linked to browser profiles"),
        (name = "Browsers", description = "Chromium instance lifecycle and profile isolation endpoints"),
        (name = "History", description = "Execution audit history and log queries"),
        (name = "Storage", description = "Local SQLite variables, tables, and workflow storage endpoints"),
        (name = "System", description = "System metrics and developer studio launcher"),
        (name = "Lint", description = "Workflow static analysis endpoints"),
        (name = "Settings", description = "Core runtime configuration endpoints")
    )
)]
pub struct ApiDoc;

async fn root_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "service": "cli",
        "version": env!("CARGO_PKG_VERSION"),
        "role": "embedded-bff-gateway",
        "status": "running",
        "docs": "/swagger-ui/",
        "api": "/api/v1"
    }))
}

pub fn create_router(state: crate::AppState) -> Router {
    let v1_router = Router::new()
        .route("/health", get(health))
        .route("/events", get(sse))
        .route("/system/studio/session", post(open_studio))
        .route("/system/metrics", get(get_metrics))
        .route("/system/ci-logs", get(get_ci_logs))
        .route("/system/ci-run", post(post_ci_run))
        .route("/system/info", get(get_system_info))
        .route("/system/cloud/status", get(get_cloud_status))
        .route("/system/cloud/sync", post(trigger_cloud_sync))
        .route("/system/cloud/login", post(cloud_login))
        .route("/system/cloud/logout", post(cloud_logout))
        .route("/system/cloud/auth/start", post(start_desktop_auth))
        .route(
            "/auth/callback",
            get(desktop_auth_callback).post(desktop_auth_callback_post),
        )
        .route("/system/workspaces", get(get_workspaces))
        .route("/system/workspaces/switch", post(switch_workspace))
        .route("/system/db/reset", post(reset_db))
        .route(
            "/system/settings",
            get(get_settings).put(update_settings).patch(patch_settings),
        )
        // Secret Store — paths mirror the `secrets` schema / table names (SPEC-core-route-foundation §5.1)
        .route("/secrets", get(get_secrets))
        .route(
            "/secrets/credentials",
            get(list_credentials).post(set_secret),
        )
        .route("/secrets/credentials/{name}", delete(delete_secret))
        .route("/secrets/credentials/push", post(push_secrets))
        .route("/secrets/credentials/pull", post(pull_secrets))
        .route("/secrets/workspace-keys/rotate", post(rotate_secrets))
        .route("/lint", post(lint_workflow))
        .route("/jobs", post(submit_job).get(get_active_jobs))
        .route("/jobs/{job_id}", delete(kill_job))
        .route(
            "/jobs/{job_id}/status",
            get(get_job_status).patch(job_finish),
        )
        .route("/jobs/{job_id}/pause", post(pause_job))
        .route("/jobs/{job_id}/resume", post(resume_job))
        .route("/jobs/{job_id}/logs", post(job_log))
        .route("/internal/daemon/events", get(worker_sse))
        .route("/history", get(get_history).delete(clear_history))
        .route("/history/{job_id}/logs", get(get_logs))
        .route("/history/{job_id}", delete(delete_history_item))
        .route("/browsers", get(get_browsers).post(create_browser))
        .route("/browsers/auto-detect", post(auto_detect_browsers))
        .route("/browsers/proxy/probe", post(probe_browser_proxy))
        .route("/browsers/batch/delete", post(batch_delete_browsers))
        .route("/browsers/sessions", delete(kill_browsers))
        .route(
            "/browsers/{id}",
            get(get_browser_detail)
                .put(update_browser)
                .patch(patch_browser)
                .delete(delete_browser),
        )
        .route("/browsers/{id}/clone", post(clone_browser))
        .route("/browsers/{id}/move", post(move_browser))
        .route("/browsers/{id}/unbind", post(unbind_browser))
        .route(
            "/browsers/{id}/session",
            post(start_browser).delete(stop_browser),
        )
        .route("/browsers/{id}/set-default", post(set_default_browser))
        .route(
            "/browsers/{id}/extensions",
            post(sideload_extension).put(update_browser_extensions),
        )
        .route("/browsers/import-csv", post(import_csv))
        .route("/browsers/drivers", get(list_drivers))
        .route("/browsers/drivers/{version}/install", post(install_driver))
        .route("/browsers/drivers/{version}/use", post(use_driver))
        .route("/browsers/drivers/{version}", delete(delete_driver))
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/bulk", post(bulk_update_tasks))
        .route("/tasks/batch/delete", post(batch_delete_tasks))
        .route("/tasks/{id}", patch(patch_task).delete(delete_task))
        .route("/ws", get(ws_handler));

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            |origin: &axum::http::HeaderValue, _parts: &axum::http::request::Parts| {
                if let Ok(s) = origin.to_str() {
                    let s_trimmed = s.trim_end_matches('/');
                    let host_with_port = s_trimmed
                        .strip_prefix("https://")
                        .or_else(|| s_trimmed.strip_prefix("http://"))
                        .unwrap_or(s_trimmed);
                    let host = host_with_port.split(':').next().unwrap_or(host_with_port);

                    s_trimmed.starts_with("chrome-extension://")
                        || s_trimmed.starts_with("moz-extension://")
                        || s_trimmed.starts_with("tauri://")
                        || host == "localhost"
                        || host == "127.0.0.1"
                        || host == "tauri.localhost"
                        || host == "asset.localhost"
                        || host == "operon.operon.dev"
                        || host == "operon.com"
                        || host.ends_with(".operon.dev")
                        || host == "tuquet.com"
                        || host.ends_with(".tuquet.com")
                        || s_trimmed == "https://automation-studio.vercel.app"
                        || s_trimmed == "https://operon.github.io"
                } else {
                    false
                }
            },
        ))
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_private_network(true);

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/", get(root_handler))
        .nest("/api/v1", v1_router.clone())
        .nest("/api", v1_router)
        .with_state(state)
        .layer(cors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;
    use crate::infrastructure::db::AutomationDb;
    use std::sync::Arc;
    use tokio::sync::{Mutex, broadcast};

    fn create_test_state() -> AppState {
        let (tx, _) = broadcast::channel(10);
        let (worker_tx, _) = broadcast::channel(10);
        let db = AutomationDb::new_in_memory().unwrap();
        let config = Arc::new(crate::config::AppConfig::load());
        AppState {
            db: Arc::new(Mutex::new(db)),
            config,
            tx,
            worker_tx,
            active_jobs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    #[tokio::test]
    async fn test_root_service_info() {
        let state = create_test_state();
        let app = create_router(state);

        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let res_root = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_root.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_cors_origins() {
        let state = create_test_state();
        let app = create_router(state);

        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let valid_origins = [
            "tauri://localhost",
            "http://tauri.localhost",
            "https://tauri.localhost",
            "http://asset.localhost",
            "https://operon.operon.dev",
            "http://operon.operon.dev",
            "https://app.operon.dev",
            "https://console.operon.dev",
            "http://console.operon.dev",
            "https://console.tuquet.com",
            "https://tuquet.com",
            "http://localhost:5173",
            "http://127.0.0.1:8765",
            "chrome-extension://abcdefghijklmnop",
        ];

        for origin in valid_origins {
            let req = Request::builder()
                .method("OPTIONS")
                .uri("/api/v1/health")
                .header("Origin", origin)
                .header("Access-Control-Request-Method", "GET")
                .body(axum::body::Body::empty())
                .unwrap();

            let res = app.clone().oneshot(req).await.unwrap();
            assert_eq!(
                res.status(),
                StatusCode::OK,
                "Failed for origin: {}",
                origin
            );
            let allow_origin = res.headers().get("access-control-allow-origin");
            assert!(
                allow_origin.is_some(),
                "CORS header missing for origin: {}",
                origin
            );
        }
    }

    #[tokio::test]
    async fn test_get_system_workspaces() {
        let state = create_test_state();
        let app = create_router(state);

        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let req = Request::builder()
            .method("GET")
            .uri("/api/v1/system/workspaces")
            .body(axum::body::Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[test]
    fn test_openapi_spec_generation() {
        let openapi = ApiDoc::openapi();
        let json_result = openapi.to_pretty_json();
        assert!(
            json_result.is_ok(),
            "OpenAPI spec serialization failed: {:?}",
            json_result.err()
        );
        let json_str = json_result.unwrap();
        assert!(json_str.contains("CLI & Core Gateway API"));
        assert!(json_str.contains("/api/v1/system/workspaces"));
        assert!(json_str.contains("/api/v1/system/ci-logs"));
        assert!(json_str.contains("/api/v1/system/cloud/auth/start"));
        assert!(json_str.contains("/api/v1/auth/callback"));
        assert!(json_str.contains("/api/v1/browsers"));
        assert!(json_str.contains("/api/v1/tasks"));
    }

    #[tokio::test]
    async fn test_tasks_and_browsers_sqlite_fk_roundtrip() {
        let state = create_test_state();
        let app = create_router(state);

        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        // 1. Create browser profile
        let create_browser_req = Request::builder()
            .method("POST")
            .uri("/api/v1/browsers")
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "id": "prf_task_link", "name": "Linked Browser" }).to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(create_browser_req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // 2. Reject task referencing unknown browser
        let bad_task_req = Request::builder()
            .method("POST")
            .uri("/api/v1/tasks")
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "title": "Bad", "browserId": "prf_nope" }).to_string(),
            ))
            .unwrap();
        let res_bad = app.clone().oneshot(bad_task_req).await.unwrap();
        assert_eq!(res_bad.status(), StatusCode::NOT_FOUND);

        // 3. Create task linked to browser and verify joined browserName
        let create_task_req = Request::builder()
            .method("POST")
            .uri("/api/v1/tasks")
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({
                    "id": "TASK-DB-1",
                    "title": "Sync cookies",
                    "browserId": "prf_task_link"
                })
                .to_string(),
            ))
            .unwrap();
        let res_task = app.clone().oneshot(create_task_req).await.unwrap();
        assert_eq!(res_task.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(res_task.into_body(), usize::MAX)
            .await
            .unwrap();
        let created: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(created["browserId"], "prf_task_link");
        assert_eq!(created["browserName"], "Linked Browser");

        // 4. Reject cross-workspace task link (409 Conflict)
        let cross_ws_req = Request::builder()
            .method("POST")
            .uri("/api/v1/tasks")
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({
                    "title": "Cross-workspace leak",
                    "workspaceId": "ws_other",
                    "browserId": "prf_task_link"
                })
                .to_string(),
            ))
            .unwrap();
        let res_cross = app.clone().oneshot(cross_ws_req).await.unwrap();
        assert_eq!(res_cross.status(), StatusCode::CONFLICT);

        // 5. Move browser to ws_other -> task in old workspace is unlinked (browserId = null)
        let move_req = Request::builder()
            .method("POST")
            .uri("/api/v1/browsers/prf_task_link/move")
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "targetWorkspaceId": "ws_other" }).to_string(),
            ))
            .unwrap();
        let res_move = app.clone().oneshot(move_req).await.unwrap();
        assert_eq!(res_move.status(), StatusCode::OK);

        let after_move_req = Request::builder()
            .method("GET")
            .uri("/api/v1/tasks")
            .body(axum::body::Body::empty())
            .unwrap();
        let res_after_move = app.clone().oneshot(after_move_req).await.unwrap();
        let move_bytes = axum::body::to_bytes(res_after_move.into_body(), usize::MAX)
            .await
            .unwrap();
        let moved_list: Vec<serde_json::Value> = serde_json::from_slice(&move_bytes).unwrap();
        assert_eq!(moved_list.len(), 1);
        assert!(moved_list[0]["browserId"].is_null());

        // 6. Re-link in ws_other via PATCH then delete browser -> ON DELETE SET NULL
        let patch_req = Request::builder()
            .method("PATCH")
            .uri("/api/v1/tasks/TASK-DB-1")
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "browserId": "prf_task_link" }).to_string(),
            ))
            .unwrap();
        let res_patch_cross = app.clone().oneshot(patch_req).await.unwrap();
        assert_eq!(res_patch_cross.status(), StatusCode::CONFLICT);

        let del_browser_req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/browsers/prf_task_link")
            .body(axum::body::Body::empty())
            .unwrap();
        let res_del = app.clone().oneshot(del_browser_req).await.unwrap();
        assert_eq!(res_del.status(), StatusCode::OK);

        let list_tasks_req = Request::builder()
            .method("GET")
            .uri("/api/v1/tasks")
            .body(axum::body::Body::empty())
            .unwrap();
        let res_list = app.oneshot(list_tasks_req).await.unwrap();
        assert_eq!(res_list.status(), StatusCode::OK);
        let list_bytes = axum::body::to_bytes(res_list.into_body(), usize::MAX)
            .await
            .unwrap();
        let list: Vec<serde_json::Value> = serde_json::from_slice(&list_bytes).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0]["browserId"].is_null());
        assert!(list[0]["browserName"].is_null());
    }

    #[tokio::test]
    async fn test_get_ci_logs_endpoint_returns_json() {
        let state = create_test_state();
        let app = create_router(state);

        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let req = Request::builder()
            .method("GET")
            .uri("/api/v1/system/ci-logs")
            .body(axum::body::Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(payload["cache"].is_object());
        assert!(payload["files"].is_array());
        assert!(payload["runnerAvailable"].is_boolean());
        assert!(payload["runningSuites"].is_array());
    }

    #[tokio::test]
    async fn test_post_ci_run_rejects_invalid_suite() {
        let state = create_test_state();
        let app = create_router(state);

        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/system/ci-run")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(r#"{"suite":"invalid"}"#))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
