use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::io::{Cursor, Read};
use std::path::PathBuf;

use super::client::fetch_bytes_with_fallback;
use super::installer::atomic_swap_binary;
use super::types::{GITHUB_REPO, is_newer_version};

const DESKTOP_LATEST_BASE: &str =
    "https://raw.githubusercontent.com/tuquet/operon/release-state/updates";

#[derive(Debug, Deserialize)]
struct LatestDesktopManifest {
    version: String,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub artifacts: Vec<ReleaseArtifact>,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseArtifact {
    pub name: String,
    pub kind: String,
    pub platform: String,
    pub sha256: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledDesktopMeta {
    pub version: String,
    pub channel: String,
}

#[derive(Debug, Clone)]
pub enum DesktopInstallOutcome {
    UpToDate(String),
    Installed { version: String, path: PathBuf },
}

pub fn desktop_app_dir() -> PathBuf {
    crate::config::canonical_dir().join("apps").join("desk")
}

pub fn desktop_bin_path() -> PathBuf {
    #[cfg(windows)]
    {
        desktop_app_dir().join("Operon.exe")
    }
    #[cfg(not(windows))]
    {
        desktop_app_dir().join("Operon")
    }
}

pub fn desktop_meta_path() -> PathBuf {
    desktop_app_dir().join("version.json")
}

pub fn read_installed_desktop_meta() -> Option<InstalledDesktopMeta> {
    let raw = std::fs::read_to_string(desktop_meta_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn desktop_platform_id() -> Option<&'static str> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        Some("windows-x86_64")
    }
    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    {
        None
    }
}

pub fn parse_latest_version(bytes: &[u8]) -> Result<String, String> {
    let parsed: LatestDesktopManifest =
        serde_json::from_slice(bytes).map_err(|e| format!("Invalid latest.json: {e}"))?;
    let version = parsed.version.trim();
    if version.is_empty() {
        return Err("latest.json missing version field".to_string());
    }
    Ok(version.to_string())
}

pub fn find_portable_artifact<'a>(
    manifest: &'a ReleaseManifest,
    platform: &str,
) -> Result<&'a ReleaseArtifact, String> {
    manifest
        .artifacts
        .iter()
        .find(|a| a.platform == platform && a.kind == "portable")
        .ok_or_else(|| format!("No portable Operon Desktop artifact for platform '{platform}'"))
}

pub fn extract_portable_binary(zip_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Invalid portable zip: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed reading zip entry: {e}"))?;
        let base_name = std::path::Path::new(entry.name())
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if base_name.eq_ignore_ascii_case("Operon.exe")
            || base_name.eq_ignore_ascii_case("workstation.exe")
        {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed extracting '{base_name}': {e}"))?;
            return Ok(buf);
        }
    }

    Err("Portable archive did not contain Operon Desktop binary".to_string())
}

/// Install or update Operon Desktop from the `release-state` channel feed into `~/.operon/apps/desk/`.
pub async fn install_or_update_desktop(
    channel: &str,
    installed_version: Option<&str>,
) -> Result<DesktopInstallOutcome, Box<dyn std::error::Error>> {
    if !matches!(channel, "stable" | "beta" | "nightly") {
        return Err(
            format!("Unsupported release channel '{channel}' (use stable|beta|nightly)").into(),
        );
    }

    let platform = desktop_platform_id().ok_or_else(|| {
        format!(
            "Operon Desktop prebuilt bundles are not yet available for {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;

    let feed_url = format!("{DESKTOP_LATEST_BASE}/{channel}/latest.json");
    let latest_bytes = fetch_bytes_with_fallback(&feed_url, 30).await?;
    let latest_version = parse_latest_version(&latest_bytes)?;

    let target_bin = desktop_bin_path();
    if let Some(cur) = installed_version
        && target_bin.exists()
        && !is_newer_version(cur, &latest_version)
    {
        return Ok(DesktopInstallOutcome::UpToDate(latest_version));
    }

    let manifest_url = format!(
        "https://github.com/{GITHUB_REPO}/releases/download/console-v{latest_version}/release-manifest.json"
    );
    let manifest_bytes = fetch_bytes_with_fallback(&manifest_url, 30).await?;
    let manifest: ReleaseManifest = serde_json::from_slice(&manifest_bytes)?;
    let artifact = find_portable_artifact(&manifest, platform)?;

    let zip_bytes = fetch_bytes_with_fallback(&artifact.url, 180).await?;
    let actual_sha = hex::encode(sha2::Sha256::digest(&zip_bytes));
    if !actual_sha.eq_ignore_ascii_case(&artifact.sha256) {
        return Err(format!(
            "SHA256 mismatch for {}: expected {}, got {}",
            artifact.name, artifact.sha256, actual_sha
        )
        .into());
    }

    let binary_bytes = extract_portable_binary(&zip_bytes)?;
    std::fs::create_dir_all(desktop_app_dir())?;
    atomic_swap_binary(&target_bin, &binary_bytes)?;

    let meta = InstalledDesktopMeta {
        version: latest_version.clone(),
        channel: channel.to_string(),
    };
    std::fs::write(desktop_meta_path(), serde_json::to_vec_pretty(&meta)?)?;

    Ok(DesktopInstallOutcome::Installed {
        version: latest_version,
        path: target_bin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::FileOptions;

    #[test]
    fn test_parse_latest_version() {
        let json = br#"{"version":"5.8.0","notes":"ok","platforms":{}}"#;
        assert_eq!(parse_latest_version(json).unwrap(), "5.8.0");
        assert!(parse_latest_version(br#"{"version":"  "}"#).is_err());
    }

    #[test]
    fn test_find_portable_artifact() {
        let manifest = ReleaseManifest {
            version: "5.8.0".to_string(),
            artifacts: vec![
                ReleaseArtifact {
                    name: "Operon-5.8.0-windows-x86_64-setup.exe".to_string(),
                    kind: "installer".to_string(),
                    platform: "windows-x86_64".to_string(),
                    sha256: "aaa".to_string(),
                    url: "https://example.invalid/setup.exe".to_string(),
                },
                ReleaseArtifact {
                    name: "Operon-5.8.0-windows-x86_64-portable.zip".to_string(),
                    kind: "portable".to_string(),
                    platform: "windows-x86_64".to_string(),
                    sha256: "bbb".to_string(),
                    url: "https://example.invalid/portable.zip".to_string(),
                },
            ],
        };
        let found = find_portable_artifact(&manifest, "windows-x86_64").unwrap();
        assert_eq!(found.kind, "portable");
        assert_eq!(found.sha256, "bbb");
        assert!(find_portable_artifact(&manifest, "darwin-aarch64").is_err());
    }

    #[test]
    fn test_extract_portable_binary() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buf);
            writer
                .start_file("Operon.exe", FileOptions::default())
                .unwrap();
            writer.write_all(b"MZ-test").unwrap();
            writer.finish().unwrap();
        }
        assert_eq!(
            extract_portable_binary(&buf.into_inner()).unwrap(),
            b"MZ-test"
        );
    }
}
