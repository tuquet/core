use crate::infrastructure::db::{
    AutomationDb, CreateBrowserRecord, CreateTaskRecord, DEFAULT_WORKSPACE_ID, FingerprintConfig,
};
use rusqlite::{Result, params};
use serde::Serialize;
use serde_json::json;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeedSummary {
    pub workspace_id: String,
    pub browsers: usize,
    pub tasks: usize,
    pub jobs: usize,
    pub logs: usize,
}

struct SeedBrowserSpec<'a> {
    id: &'a str,
    name: &'a str,
    group: &'a str,
    tags: &'a [&'a str],
    os_platform: &'a str,
    proxy: Option<&'a str>,
    timezone: &'a str,
    seed: u32,
    cores: u32,
    ram_gb: u32,
}

struct SeedTaskSpec<'a> {
    id: &'a str,
    browser_id: Option<&'a str>,
    title: &'a str,
    status: &'a str,
    label: &'a str,
    priority: &'a str,
    assignee: &'a str,
    favorite: bool,
    progress: i64,
}

const SEED_BROWSERS: &[SeedBrowserSpec<'_>] = &[
    SeedBrowserSpec {
        id: "prf_seed_tiktok_01",
        name: "TikTok Stealth Alpha",
        group: "social",
        tags: &["tiktok", "warmup", "vn"],
        os_platform: "windows",
        proxy: Some("socks5://127.0.0.1:1080"),
        timezone: "Asia/Ho_Chi_Minh",
        seed: 104_821,
        cores: 8,
        ram_gb: 16,
    },
    SeedBrowserSpec {
        id: "prf_seed_shopee_02",
        name: "Shopee Ops Mac",
        group: "commerce",
        tags: &["shopee", "checkout"],
        os_platform: "macos",
        proxy: Some("http://127.0.0.1:8118"),
        timezone: "Asia/Ho_Chi_Minh",
        seed: 209_314,
        cores: 10,
        ram_gb: 16,
    },
    SeedBrowserSpec {
        id: "prf_seed_fb_03",
        name: "Meta Ads Runner",
        group: "ads",
        tags: &["facebook", "ads"],
        os_platform: "windows",
        proxy: None,
        timezone: "America/Los_Angeles",
        seed: 318_450,
        cores: 12,
        ram_gb: 32,
    },
    SeedBrowserSpec {
        id: "prf_seed_ig_04",
        name: "Instagram Scraper Linux",
        group: "social",
        tags: &["instagram", "headless"],
        os_platform: "linux",
        proxy: Some("socks5h://127.0.0.1:1080"),
        timezone: "Europe/Berlin",
        seed: 427_902,
        cores: 4,
        ram_gb: 8,
    },
    SeedBrowserSpec {
        id: "prf_seed_guest_05",
        name: "Clean Sandbox Profile",
        group: "default",
        tags: &["qa", "sandbox"],
        os_platform: "windows",
        proxy: None,
        timezone: "UTC",
        seed: 512_009,
        cores: 8,
        ram_gb: 16,
    },
];

const SEED_TASKS: &[SeedTaskSpec<'_>] = &[
    SeedTaskSpec {
        id: "TASK-1001",
        browser_id: Some("prf_seed_tiktok_01"),
        title: "Warmup TikTok For-You feed 15m and collect trending hashtags",
        status: "in progress",
        label: "feature",
        priority: "high",
        assignee: "linh",
        favorite: true,
        progress: 50,
    },
    SeedTaskSpec {
        id: "TASK-1002",
        browser_id: Some("prf_seed_tiktok_01"),
        title: "Verify SOCKS5 WebRTC shield leak check on TikTok login",
        status: "done",
        label: "bug",
        priority: "high",
        assignee: "minh",
        favorite: true,
        progress: 100,
    },
    SeedTaskSpec {
        id: "TASK-1003",
        browser_id: Some("prf_seed_shopee_02"),
        title: "Export daily flash-sale SKU prices via Shopee seller portal",
        status: "todo",
        label: "feature",
        priority: "high",
        assignee: "alex",
        favorite: true,
        progress: 25,
    },
    SeedTaskSpec {
        id: "TASK-1004",
        browser_id: Some("prf_seed_shopee_02"),
        title: "Refresh session cookies before midnight flash-sale window",
        status: "backlog",
        label: "documentation",
        priority: "medium",
        assignee: "sara",
        favorite: false,
        progress: 0,
    },
    SeedTaskSpec {
        id: "TASK-1005",
        browser_id: Some("prf_seed_fb_03"),
        title: "Audit Business Manager ad-account spend limits",
        status: "in progress",
        label: "feature",
        priority: "medium",
        assignee: "kai",
        favorite: false,
        progress: 75,
    },
    SeedTaskSpec {
        id: "TASK-1006",
        browser_id: Some("prf_seed_fb_03"),
        title: "Fix 2FA checkpoint prompt selector on Meta Ads Manager",
        status: "todo",
        label: "bug",
        priority: "high",
        assignee: "linh",
        favorite: false,
        progress: 0,
    },
    SeedTaskSpec {
        id: "TASK-1007",
        browser_id: Some("prf_seed_ig_04"),
        title: "Scrape reel engagement metrics into CSV export",
        status: "done",
        label: "feature",
        priority: "medium",
        assignee: "alex",
        favorite: false,
        progress: 100,
    },
    SeedTaskSpec {
        id: "TASK-1008",
        browser_id: Some("prf_seed_ig_04"),
        title: "Investigate headless Turnstile timeout under German proxy",
        status: "in progress",
        label: "bug",
        priority: "low",
        assignee: "minh",
        favorite: false,
        progress: 25,
    },
    SeedTaskSpec {
        id: "TASK-1009",
        browser_id: Some("prf_seed_guest_05"),
        title: "Smoke-test clean profile launch and extension sideload",
        status: "done",
        label: "documentation",
        priority: "low",
        assignee: "sara",
        favorite: false,
        progress: 100,
    },
    SeedTaskSpec {
        id: "TASK-1010",
        browser_id: Some("prf_seed_guest_05"),
        title: "Deprecated legacy v1 extension bridge parity check",
        status: "canceled",
        label: "documentation",
        priority: "low",
        assignee: "kai",
        favorite: false,
        progress: 0,
    },
    SeedTaskSpec {
        id: "TASK-1011",
        browser_id: None,
        title: "Assign dedicated stealth profile for weekend crawler batch",
        status: "todo",
        label: "feature",
        priority: "medium",
        assignee: "linh",
        favorite: true,
        progress: 0,
    },
    SeedTaskSpec {
        id: "TASK-1012",
        browser_id: None,
        title: "Document workspace isolation rules for unassigned tasks",
        status: "backlog",
        label: "documentation",
        priority: "low",
        assignee: "sara",
        favorite: false,
        progress: 0,
    },
];

/// Resets `browsers`, `tasks`, `jobs`, and `logs` and seeds deterministic UI acceptance fixtures.
pub fn reset_and_seed(
    db: &AutomationDb,
    workspace_id: &str,
    clear_all: bool,
) -> Result<SeedSummary> {
    let target_ws = if workspace_id.trim().is_empty() {
        DEFAULT_WORKSPACE_ID
    } else {
        workspace_id.trim()
    };

    let tx = db.raw_conn().unchecked_transaction()?;

    if clear_all {
        tx.execute("DELETE FROM logs", [])?;
        tx.execute("DELETE FROM jobs", [])?;
        tx.execute("DELETE FROM tasks", [])?;
        tx.execute("DELETE FROM browsers", [])?;
    } else {
        tx.execute(
            "DELETE FROM logs WHERE job_id IN (
                SELECT id FROM jobs
                WHERE task_id IN (SELECT id FROM tasks WHERE workspace_id = ?1)
                   OR browser_id IN (SELECT id FROM browsers WHERE workspace_id = ?1)
            )",
            params![target_ws],
        )?;
        tx.execute(
            "DELETE FROM jobs
             WHERE task_id IN (SELECT id FROM tasks WHERE workspace_id = ?1)
                OR browser_id IN (SELECT id FROM browsers WHERE workspace_id = ?1)",
            params![target_ws],
        )?;
        tx.execute(
            "DELETE FROM tasks WHERE workspace_id = ?1",
            params![target_ws],
        )?;
        tx.execute(
            "DELETE FROM browsers WHERE workspace_id = ?1",
            params![target_ws],
        )?;
    }
    tx.commit()?;

    let browsers_repo = db.browsers();
    let exts = vec!["automation".to_string()];
    let empty_flags: Vec<String> = Vec::new();

    for b in SEED_BROWSERS {
        let tags: Vec<String> = b.tags.iter().map(|s| (*s).to_string()).collect();
        let (screen_w, screen_h, dpr) = if b.os_platform == "macos" {
            (2560, 1440, 2.0)
        } else {
            (1920, 1080, 1.0)
        };
        let fp = FingerprintConfig {
            seed: Some(b.seed),
            os_platform: Some(b.os_platform.to_string()),
            os_version: Some("10.0.0".to_string()),
            hardware_concurrency: Some(b.cores),
            device_memory_gb: Some(b.ram_gb),
            screen_width: Some(screen_w),
            screen_height: Some(screen_h),
            device_pixel_ratio: Some(dpr),
            webrtc_mode: Some("proxy_shielded".to_string()),
            noise_level: Some("balanced".to_string()),
            lang: Some("en-US".to_string()),
            accept_lang: Some("en-US,en;q=0.9".to_string()),
        };

        browsers_repo.create_browser_record(&CreateBrowserRecord {
            id: b.id,
            workspace_id: target_ws,
            name: b.name,
            group_name: Some(b.group),
            tags: Some(&tags),
            user_agent: None,
            timezone: Some(b.timezone),
            proxy: b.proxy,
            extensions: Some(&exts),
            fingerprint_config: Some(&fp),
            startup_flags: Some(&empty_flags),
        })?;
    }

    let tasks_repo = db.tasks();
    for t in SEED_TASKS {
        tasks_repo.create_task(&CreateTaskRecord {
            id: t.id,
            workspace_id: target_ws,
            browser_id: t.browser_id,
            title: t.title,
            status: t.status,
            label: t.label,
            priority: t.priority,
            assignee: t.assignee,
            favorite: t.favorite,
            progress: t.progress,
        })?;
    }

    let jobs_repo = db.jobs();
    let sample_jobs = [
        (
            "job_seed_01",
            Some("prf_seed_tiktok_01"),
            Some("TASK-1002"),
            "Verify WebRTC shield leak check",
            "completed",
            "info",
            "All WebRTC ICE candidates routed cleanly through SOCKS5 proxy",
        ),
        (
            "job_seed_02",
            Some("prf_seed_tiktok_01"),
            Some("TASK-1001"),
            "Warmup TikTok For-You feed",
            "running",
            "info",
            "Scrolling feed iteration 4/10 with human jitter",
        ),
        (
            "job_seed_03",
            Some("prf_seed_ig_04"),
            Some("TASK-1008"),
            "Turnstile timeout diagnostic",
            "failed",
            "error",
            "Turnstile solver timed out after 20000ms",
        ),
    ];

    let mut logs_inserted = 0;
    for (job_id, browser_id, task_id, name, status, log_type, log_msg) in sample_jobs {
        jobs_repo.create_job_with_links(
            job_id,
            browser_id,
            task_id,
            name,
            &json!({ "seeded": true }),
            &json!({ "browserId": browser_id, "taskId": task_id }),
            status,
        );
        jobs_repo.insert_log(job_id, log_type, log_msg)?;
        logs_inserted += 1;
    }

    if let Ok(mut settings) = db.settings().get_settings() {
        settings
            .browser
            .set_default_profile_for(target_ws, Some("prf_seed_tiktok_01".to_string()));
        let _ = db.settings().save_settings(&settings);
    }

    Ok(SeedSummary {
        workspace_id: target_ws.to_string(),
        browsers: SEED_BROWSERS.len(),
        tasks: SEED_TASKS.len(),
        jobs: sample_jobs.len(),
        logs: logs_inserted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_and_seed_is_idempotent_and_links_fks() {
        let db = AutomationDb::new_in_memory().unwrap();

        let first = reset_and_seed(&db, DEFAULT_WORKSPACE_ID, false).unwrap();
        assert_eq!(first.browsers, 5);
        assert_eq!(first.tasks, 12);
        assert_eq!(first.jobs, 3);

        // Second run cleanly wipes and re-creates identical fixture set without FK errors
        let second = reset_and_seed(&db, DEFAULT_WORKSPACE_ID, false).unwrap();
        assert_eq!(second.browsers, 5);
        assert_eq!(second.tasks, 12);

        let tasks = db
            .tasks()
            .list_tasks(Some(DEFAULT_WORKSPACE_ID), None, None, None)
            .unwrap();
        assert_eq!(tasks.len(), 12);
        let linked = tasks
            .iter()
            .find(|t| t.id == "TASK-1001")
            .expect("TASK-1001 seeded");
        assert_eq!(linked.browser_id.as_deref(), Some("prf_seed_tiktok_01"));
        assert_eq!(linked.browser_name.as_deref(), Some("TikTok Stealth Alpha"));

        let unassigned = db
            .tasks()
            .list_tasks(
                Some(DEFAULT_WORKSPACE_ID),
                Some("__unassigned__"),
                None,
                None,
            )
            .unwrap();
        assert_eq!(unassigned.len(), 2);
    }
}
