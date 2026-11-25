use clap::Args;
use std::path::{Path, PathBuf};

use crate::infrastructure::updater::desktop::{
    DesktopInstallOutcome, desktop_bin_path, desktop_platform_id, install_or_update_desktop,
};
use crate::ui::is_interactive_tty;
use crate::ui::prompt::{
    ClackSpinner, clack_confirm, clack_intro, clack_outro, clack_step_info, clack_step_ok,
    clack_step_warn,
};

#[derive(Args, Debug, Clone)]
pub struct DeskArgs {
    /// Specific page/route to open (e.g. 'jobs', 'devices', 'browsers', 'workflows', 'overview')
    #[arg(short = 'p', long, value_name = "PAGE")]
    pub page: Option<String>,

    /// Direct shortcut to the Jobs monitor (/automation/jobs)
    #[arg(short = 'j', long)]
    pub jobs: bool,

    /// Direct shortcut to the Devices page (/automation/devices)
    #[arg(short = 'd', long)]
    pub devices: bool,

    /// Direct shortcut to the Browsers page (/automation/browsers)
    #[arg(short = 'b', long)]
    pub browsers: bool,

    /// Direct shortcut to the Workflows builder (/automation/workflows)
    #[arg(short = 'w', long)]
    pub workflows: bool,

    /// Target local Web host shell port (defaults to VITE_PORT or 5666 if running)
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Skip background daemon verification and auto-start
    #[arg(long)]
    pub no_daemon: bool,

    /// Open Web host shell in system browser instead of Operon Desktop
    #[arg(long)]
    pub web: bool,

    /// Release channel for Operon Desktop auto-install (stable | beta | nightly)
    #[arg(long, default_value = "stable", value_name = "CHANNEL")]
    pub channel: String,

    /// Auto-confirm downloading Operon Desktop if not yet installed
    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub fn resolve_daemon_port() -> u16 {
    std::env::var(crate::constants::ENV_DAEMON_PORT)
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(crate::constants::DEFAULT_RUNNER_PORT)
}

pub async fn is_daemon_healthy(port: u16) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1200))
        .build()
    else {
        return false;
    };
    let url = format!("http://127.0.0.1:{port}/api/v1/health");
    client
        .get(&url)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Ensure the local daemon is online, spawning it detached and polling `/api/v1/health` if needed.
pub async fn ensure_daemon_online(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    if is_daemon_healthy(port).await {
        clack_step_ok(format!("daemon online on 127.0.0.1:{port}"));
        return Ok(());
    }

    let sp = ClackSpinner::start(format!("Starting daemon on 127.0.0.1:{port}..."));
    let current_exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(&current_exe);
    cmd.args([
        "worker",
        "start",
        "-H",
        "127.0.0.1",
        "-p",
        &port.to_string(),
        "-d",
    ]);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }

    cmd.spawn()?;
    for _ in 0..8 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if is_daemon_healthy(port).await {
            sp.finish(format!("daemon started on 127.0.0.1:{port}"));
            return Ok(());
        }
    }

    sp.clear();
    clack_step_warn(format!(
        "daemon spawned on 127.0.0.1:{port} (still warming up)"
    ));
    Ok(())
}

pub fn resolve_route(args: &DeskArgs) -> &str {
    if args.jobs {
        return "/automation/jobs";
    }
    if args.devices {
        return "/automation/devices";
    }
    if args.browsers {
        return "/automation/browsers";
    }
    if args.workflows {
        return "/automation/workflows";
    }
    let Some(page) = args.page.as_deref() else {
        return "/automation/overview";
    };
    match page.to_lowercase().as_str() {
        "jobs" | "job" => "/automation/jobs",
        "devices" | "device" | "node" | "nodes" => "/automation/devices",
        "browsers" | "browser" | "profile" | "profiles" => "/automation/browsers",
        "workflows" | "workflow" => "/automation/workflows",
        "overview" => "/automation/overview",
        "dashboard" | "analytics" => "/dashboard/analytics",
        "secrets" | "secret" => "/secrets/credentials",
        other if other.starts_with('/') => page,
        _ => "/automation/overview",
    }
}

pub async fn handle(args: DeskArgs) -> Result<(), Box<dyn std::error::Error>> {
    clack_intro("opn desk", "Operon Desktop & Host Shell");

    if !args.no_daemon {
        let port = resolve_daemon_port();
        if let Err(e) = ensure_daemon_online(port).await {
            clack_step_warn(format!("Failed to auto-start daemon: {e}"));
        }
    }

    let route = resolve_route(&args);

    if args.web {
        return launch_web_shell(args.port, route).await;
    }

    if let Some(existing) = find_installed_desktop_binary() {
        spawn_desktop_app(&existing, route)?;
        clack_outro(&format!("Launched Operon Desktop ({})", existing.display()));
        return Ok(());
    }

    if desktop_platform_id().is_none() {
        clack_step_warn(format!(
            "Operon Desktop native bundle is not yet published for {}-{}; opening Web host shell.",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        return launch_web_shell(args.port, route).await;
    }

    let approved = if args.yes {
        true
    } else if is_interactive_tty() {
        clack_confirm(
            &format!(
                "Operon Desktop is not installed in {}. Download & install from '{}' channel?",
                desktop_bin_path().display(),
                args.channel
            ),
            true,
        )?
        .unwrap_or(false)
    } else {
        false
    };

    if !approved {
        clack_step_info(
            "Skipped Operon Desktop download (pass --yes to auto-install, or --web for browser).",
        );
        return launch_web_shell(args.port, route).await;
    }

    let sp = ClackSpinner::start(format!(
        "Downloading Operon Desktop ({} channel)...",
        args.channel
    ));
    match install_or_update_desktop(&args.channel, None).await? {
        DesktopInstallOutcome::Installed { version, path } => {
            sp.finish(format!("Installed Operon Desktop v{version}"));
            spawn_desktop_app(&path, route)?;
            clack_outro(&format!("Launched Operon Desktop ({})", path.display()));
            Ok(())
        }
        DesktopInstallOutcome::UpToDate(version) => {
            sp.finish(format!("Operon Desktop v{version} ready"));
            let path = desktop_bin_path();
            spawn_desktop_app(&path, route)?;
            clack_outro(&format!("Launched Operon Desktop ({})", path.display()));
            Ok(())
        }
    }
}

fn find_installed_desktop_binary() -> Option<PathBuf> {
    let candidates = [
        desktop_bin_path(),
        PathBuf::from("console/apps/workstation/src-tauri/target/release/Operon.exe"),
        PathBuf::from("../console/apps/workstation/src-tauri/target/release/Operon.exe"),
        PathBuf::from(
            "../console/apps/workstation/src-tauri/target/release/operon-workstation.exe",
        ),
    ];
    candidates.into_iter().find(|p| p.exists())
}

fn spawn_desktop_app(path: &Path, route: &str) -> Result<(), Box<dyn std::error::Error>> {
    clack_step_info(format!(
        "Opening Operon Desktop (route: {route}) -> {}",
        path.display()
    ));
    std::process::Command::new(path).spawn()?;
    Ok(())
}

async fn resolve_web_url(explicit_port: Option<u16>, route: &str) -> String {
    if let Some(port) = explicit_port.or_else(|| {
        std::env::var("VITE_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
    }) {
        return format!("http://localhost:{port}{route}");
    }

    if let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(400))
        .build()
        && client.get("http://localhost:5666").send().await.is_ok()
    {
        return format!("http://localhost:5666{route}");
    }

    format!("https://tuquet-console.pages.dev{route}")
}

async fn launch_web_shell(
    explicit_port: Option<u16>,
    route: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let target_url = resolve_web_url(explicit_port, route).await;
    clack_step_ok(format!("Opening Web host shell: {target_url}"));
    open_in_system_browser(&target_url)?;
    clack_outro("Host shell opened.");
    Ok(())
}

fn open_in_system_browser(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_args() -> DeskArgs {
        DeskArgs {
            page: None,
            jobs: false,
            devices: false,
            browsers: false,
            workflows: false,
            port: None,
            no_daemon: false,
            web: false,
            channel: "stable".to_string(),
            yes: false,
        }
    }

    #[test]
    fn test_resolve_route() {
        let mut args = default_args();
        assert_eq!(resolve_route(&args), "/automation/overview");
        args.jobs = true;
        assert_eq!(resolve_route(&args), "/automation/jobs");
        args.jobs = false;
        args.page = Some("browsers".to_string());
        assert_eq!(resolve_route(&args), "/automation/browsers");
        args.page = Some("/custom/view".to_string());
        assert_eq!(resolve_route(&args), "/custom/view");
    }
}
