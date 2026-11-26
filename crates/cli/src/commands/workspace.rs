use crate::cli::WorkspaceSubcommands;
use crate::config::AppConfig;
use crate::infrastructure::cloud::{CloudApiClient, CredentialStore, WorkspaceManager};
use crate::ui::{
    Card, ClackOption, ClackSpinner, Column, OutputFormat, Table, badge_offline, badge_online,
    clack_cancel, clack_intro, clack_outro, clack_select, respond_with,
};

pub async fn handle(
    command: Option<WorkspaceSubcommands>,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Some(WorkspaceSubcommands::List { format }) => list_workspaces(format.resolve()).await,
        Some(WorkspaceSubcommands::Switch { target, format }) => {
            switch_workspace(target.as_deref(), format.resolve()).await
        }
        Some(WorkspaceSubcommands::Current { format }) => {
            show_current_workspace(format.resolve()).await
        }
        None => {
            if !crate::ui::is_interactive_tty() {
                return show_current_workspace(OutputFormat::Json).await;
            }
            clack_intro("opn auth workspace", "Multi-Workspace Workspace Controller");
            let opts = vec![
                ClackOption::new(
                    "Current Active Workspace",
                    "Inspect the currently bound workspace & RLS scope",
                    "current",
                ),
                ClackOption::new(
                    "Switch Active Workspace",
                    "Select and bind to another accessible workspace",
                    "switch",
                ),
                ClackOption::new(
                    "List Accessible Workspaces",
                    "Display table of all organizations your account belongs to",
                    "list",
                ),
            ];
            let Some(idx) = clack_select("Select workspace action:", &opts, 0, 6)? else {
                clack_cancel("Workspace hub closed.");
                return Ok(());
            };
            match idx {
                0 => show_current_workspace(OutputFormat::Card).await,
                1 => switch_workspace(None, OutputFormat::Card).await,
                2 => list_workspaces(OutputFormat::Table).await,
                _ => Ok(()),
            }
        }
    }
}

pub async fn list_workspaces(format: OutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load();
    let creds = match CredentialStore::load(&config.data_dir).await {
        Some(c) => c,
        None => {
            return Err(
                "No active cloud session found on this machine. Run 'opn auth login' first.".into(),
            );
        }
    };

    let cloud_url =
        crate::infrastructure::cloud::resolve_supabase_url(None, creds.cloud_url.as_deref())?;
    let api_key =
        crate::infrastructure::cloud::resolve_supabase_anon_key(creds.api_key.as_deref())?;

    let client = CloudApiClient::build_http_client();

    let user_id = creds.user_id.clone().unwrap_or_default();
    let access_token = match creds.access_token.as_ref() {
        Some(t) => t.clone(),
        None => {
            if let Some(tid) = creds.workspace_id.as_deref() {
                let out = serde_json::json!({
                    "enrolled": true,
                    "standalone_workspace_id": tid,
                    "multi_workspace": false,
                    "message": "Desktop enrolled under standalone workspace. Log in with an operator account to manage multi-workspace workspaces."
                });
                return respond_with(format, &out, |_| {
                    clack_intro("opn auth workspace", "Standalone Desktop Enrollment");
                    crate::ui::clack_step_info(format!(
                        "Desktop enrolled under standalone workspace: \x1b[1;97m{}\x1b[0m",
                        tid
                    ));
                    clack_outro(
                        "Log in with an operator account via 'opn auth login' to manage multi-workspace workspaces.",
                    );
                });
            }
            return Err("Desktop is not logged in. Run 'opn auth login' first.".into());
        }
    };

    let sp = if !format.is_json() {
        Some(ClackSpinner::start(
            "Fetching accessible multi-workspace workspaces...",
        ))
    } else {
        None
    };

    let mut workspaces = WorkspaceManager::fetch_user_workspaces(
        &client,
        &cloud_url,
        &api_key,
        &access_token,
        &user_id,
    )
    .await
    .unwrap_or_else(|_| creds.available_workspaces.clone().unwrap_or_default());

    if let Some(s) = sp {
        s.clear();
    }

    let active_id = creds.workspace_id.as_deref().unwrap_or("");
    let active_slug = creds.workspace_slug.as_deref().unwrap_or("");

    for t in workspaces.iter_mut() {
        t.is_active = t.id == active_id || t.slug.eq_ignore_ascii_case(active_slug);
    }

    respond_with(format, &workspaces, |t_list| {
        if format.is_card() {
            println!();
            for t in t_list {
                let mut card = Card::new("WORKSPACE");
                if t.is_active {
                    card.with_badge(badge_online("ACTIVE"));
                } else {
                    card.with_badge(badge_offline("STANDBY"));
                }
                card.with_min_width(56);
                card.add_kv("Workspace Name", &t.name);
                card.add_kv("Slug", &t.slug);
                card.add_kv("Role", t.role.as_deref().unwrap_or("member"));
                card.add_kv("Workspace ID", &t.id);
                card.print();
                println!();
            }
            return;
        }

        let columns = vec![
            Column {
                title: "WORKSPACE".to_string(),
                min_width: 22,
                align_right: false,
            },
            Column {
                title: "SLUG".to_string(),
                min_width: 18,
                align_right: false,
            },
            Column {
                title: "ROLE".to_string(),
                min_width: 10,
                align_right: false,
            },
            Column {
                title: "STATUS".to_string(),
                min_width: 10,
                align_right: false,
            },
            Column {
                title: "ACTIVE".to_string(),
                min_width: 12,
                align_right: false,
            },
        ];
        let mut table = Table::new(columns);

        for t in t_list {
            let active_badge = if t.is_active {
                badge_online("ACTIVE")
            } else {
                badge_offline("STANDBY")
            };
            let status_str = t.status.as_deref().unwrap_or("active");
            let role_str = t.role.as_deref().unwrap_or("member");

            table.add_row(vec![
                t.name.clone(),
                t.slug.clone(),
                role_str.to_string(),
                status_str.to_string(),
                active_badge,
            ]);
        }

        clack_intro(
            "opn auth workspace",
            "Accessible Multi-Workspace Workspaces",
        );
        table.print();
        clack_outro("Switch active workspace: opn auth workspace switch [slug]");
    })
}

pub async fn switch_workspace(
    target_opt: Option<&str>,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load();

    let resolved_target: String = match target_opt {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => {
            // Zero-Arg Interactive: load accessible workspaces and let user select via Clack
            let creds = CredentialStore::load(&config.data_dir)
                .await
                .ok_or("No active cloud session found. Run 'opn auth login' first.")?;
            let workspaces = creds.available_workspaces.unwrap_or_default();
            if workspaces.is_empty() {
                return Err("No multi-workspace workspaces cached. Run 'opn auth workspace' or 'opn auth login' first.".into());
            }

            clack_intro(
                "opn auth workspace switch",
                "Interactive Workspace Selector",
            );
            let active_slug = creds.workspace_slug.as_deref().unwrap_or("");
            let mut default_idx = 0usize;
            let opts: Vec<ClackOption> = workspaces
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    if t.slug.eq_ignore_ascii_case(active_slug) {
                        default_idx = i;
                    }
                    let role = t.role.as_deref().unwrap_or("member");
                    let active_tag = if t.slug.eq_ignore_ascii_case(active_slug) {
                        " • active"
                    } else {
                        ""
                    };
                    ClackOption::new(
                        &t.name,
                        format!("{} (role: {}{})", t.slug, role, active_tag),
                        format!("{} ({})", t.name, t.slug),
                    )
                })
                .collect();

            match clack_select(
                "Select target workspace to activate",
                &opts,
                default_idx,
                10,
            )? {
                Some(idx) => workspaces[idx].slug.clone(),
                None => {
                    clack_cancel("Workspace switch cancelled.");
                    return Ok(());
                }
            }
        }
    };

    match WorkspaceManager::switch_workspace(&config.data_dir, &resolved_target).await {
        Ok(workspace) => {
            let out = serde_json::json!({
                "switched": true,
                "workspace": {
                    "id": workspace.id,
                    "slug": workspace.slug,
                    "name": workspace.name,
                    "role": workspace.role,
                    "status": workspace.status
                }
            });

            respond_with(format, &out, |_| {
                let mut card = Card::new("WORKSPACE");
                card.with_badge(badge_online("SWITCHED"));
                card.with_min_width(64);
                card.add_kv("Active Workspace", &workspace.name);
                card.add_kv("Workspace Slug", &workspace.slug);
                card.add_kv("Workspace ID", &workspace.id);
                if let Some(r) = &workspace.role {
                    card.add_kv("Your Role", r);
                }
                card.with_footer("Desktop runner successfully rebound to target workspace");
                println!();
                card.print();
                println!();
            })
        }
        Err(e) => Err(format!("Failed to switch workspace: {}", e).into()),
    }
}

pub async fn show_current_workspace(
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load();
    let creds = match CredentialStore::load(&config.data_dir).await {
        Some(c) => c,
        None => {
            let out = serde_json::json!({
                "authenticated": false,
                "message": "No active cloud session found on this machine. Run 'opn auth login' to authenticate."
            });
            return respond_with(format, &out, |_| {
                let mut card = Card::new("WORKSPACE");
                card.with_badge(badge_offline("NOT AUTHENTICATED"));
                card.with_min_width(64);
                card.add_line("No active cloud session found on this machine.");
                card.with_footer("Run 'opn auth login' to authenticate with Cloud");
                println!();
                card.print();
                println!();
            });
        }
    };

    let current_json = serde_json::json!({
        "active": true,
        "workspace_id": creds.workspace_id,
        "workspace_slug": creds.workspace_slug.as_deref().unwrap_or("default"),
        "workspace_name": creds.workspace_name.as_deref().unwrap_or("Personal Workspace"),
        "role": creds.workspace_role.as_deref().unwrap_or("owner"),
        "user_email": creds.email,
        "device_id": creds.device_id,
        "runner_device": creds.name
    });

    respond_with(format, &current_json, |_| {
        let mut card = Card::new("WORKSPACE");
        card.with_badge(badge_online("ACTIVE"));
        card.with_min_width(64);
        card.add_kv(
            "Workspace Name",
            creds
                .workspace_name
                .as_deref()
                .unwrap_or("Personal Workspace"),
        );
        card.add_kv(
            "Workspace Slug",
            creds.workspace_slug.as_deref().unwrap_or("default"),
        );
        card.add_kv(
            "Workspace ID",
            creds.workspace_id.as_deref().unwrap_or("standalone"),
        );
        if let Some(role) = creds.workspace_role.as_deref() {
            card.add_kv("Assigned Role", role);
        }
        if let Some(email) = creds.email.as_deref() {
            card.add_kv("Logged In As", email);
        }
        card.add_kv("Runner Device", &creds.name);
        card.with_footer("Run 'opn auth workspace' to view all available workspaces");
        println!();
        card.print();
        println!();
    })
}
