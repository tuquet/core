use crate::core::error::AutomationError;
use crate::core::models::id::BrowserId;
use crate::infrastructure::db::{DEFAULT_WORKSPACE_ID, FingerprintConfig};
use std::collections::HashSet;

pub fn is_valid_id(id: &str) -> bool {
    BrowserId::new(id).is_valid()
}

pub fn validate_name(name: &str) -> Result<String, AutomationError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AutomationError::BadRequest(
            "Browser profile name cannot be empty".to_string(),
        ));
    }
    if trimmed.chars().count() > 100 {
        return Err(AutomationError::BadRequest(
            "Browser profile name must be at most 100 characters".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

pub fn validate_proxy_uri(proxy: Option<&str>) -> Result<Option<String>, AutomationError> {
    let Some(raw) = proxy else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let lower = trimmed.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("socks5h://")
        .or_else(|| lower.strip_prefix("socks5://"))
        .or_else(|| lower.strip_prefix("https://"))
        .or_else(|| lower.strip_prefix("http://"))
        .ok_or_else(|| {
            AutomationError::BadRequest(
                "Invalid proxy scheme (expected http://, https://, socks5://, or socks5h://)"
                    .to_string(),
            )
        })?;
    if rest.is_empty() || rest.starts_with('@') || rest.ends_with('@') {
        return Err(AutomationError::BadRequest(
            "Invalid proxy URI: missing host".to_string(),
        ));
    }
    Ok(Some(trimmed.to_string()))
}

pub fn resolve_workspace_id(raw: Option<&str>) -> Result<String, AutomationError> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        Some(tid) => {
            if !is_valid_id(tid) {
                return Err(AutomationError::BadRequest(
                    "Invalid workspaceId".to_string(),
                ));
            }
            Ok(tid.to_string())
        }
        None => Ok(DEFAULT_WORKSPACE_ID.to_string()),
    }
}

pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for t in tags {
        let clean = t.trim().to_ascii_lowercase();
        if !clean.is_empty() && seen.insert(clean.clone()) {
            out.push(clean);
            if out.len() >= 20 {
                break;
            }
        }
    }
    out
}

pub fn normalize_extensions(exts: &[String], enforce_automation: bool) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for ext in exts {
        let clean = ext.trim().to_string();
        if !clean.is_empty() && seen.insert(clean.clone()) {
            out.push(clean);
        }
    }
    if enforce_automation && !seen.contains("automation") {
        out.insert(0, "automation".to_string());
    }
    out
}

pub fn sanitize_startup_flags(flags: &[String]) -> Result<Vec<String>, AutomationError> {
    const BLOCKED_PREFIXES: &[&str] = &[
        "--no-sandbox",
        "--disable-web-security",
        "--user-data-dir",
        "--remote-debugging-port",
        "--load-extension",
    ];
    let mut out = Vec::new();
    for raw in flags {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with('-') {
            return Err(AutomationError::BadRequest(format!(
                "Startup flag '{}' must start with '-'",
                trimmed
            )));
        }
        let lower = trimmed.to_ascii_lowercase();
        if BLOCKED_PREFIXES
            .iter()
            .any(|blocked| lower == *blocked || lower.starts_with(&format!("{blocked}=")))
        {
            return Err(AutomationError::BadRequest(format!(
                "Startup flag '{}' is blocked for sandbox safety",
                trimmed
            )));
        }
        if !out.contains(&trimmed.to_string()) {
            out.push(trimmed.to_string());
        }
    }
    Ok(out)
}

pub fn generate_fingerprint_defaults(
    name: &str,
    input: Option<FingerprintConfig>,
    proxy: Option<&str>,
    timezone: Option<&str>,
) -> FingerprintConfig {
    let provided = input.unwrap_or_default();
    let generated = browser::BrowserProfile::generate(
        name,
        provided.seed,
        provided.os_platform.clone(),
        provided.hardware_concurrency,
        provided.device_memory_gb,
        proxy.map(str::to_string),
        timezone.map(str::to_string),
        provided.lang.clone(),
    );

    FingerprintConfig {
        seed: provided.seed.or(generated.fingerprint_seed),
        os_platform: provided.os_platform.or(generated.os_platform),
        os_version: provided.os_version.or(generated.os_version),
        hardware_concurrency: provided
            .hardware_concurrency
            .or(generated.hardware_concurrency),
        device_memory_gb: provided.device_memory_gb.or(generated.device_memory_gb),
        screen_width: provided.screen_width.or(generated.screen_width),
        screen_height: provided.screen_height.or(generated.screen_height),
        device_pixel_ratio: provided.device_pixel_ratio.or(generated.device_pixel_ratio),
        webrtc_mode: provided
            .webrtc_mode
            .or_else(|| Some("proxy_shielded".to_string())),
        noise_level: provided
            .noise_level
            .or_else(|| Some("balanced".to_string())),
        lang: provided.lang.or(generated.lang),
        accept_lang: provided.accept_lang.or(generated.accept_lang),
    }
}

pub fn push_antidetect_args(
    args: &mut Vec<String>,
    fp: &FingerprintConfig,
    has_proxy: bool,
    has_grid: bool,
) {
    if let Some(seed) = fp.seed {
        args.push(format!("--fingerprint={}", seed));
    }
    if let Some(ref platform) = fp.os_platform
        && !platform.trim().is_empty()
    {
        args.push(format!("--fingerprint-platform={}", platform.trim()));
    }
    if let Some(ref ver) = fp.os_version
        && !ver.trim().is_empty()
    {
        args.push(format!("--fingerprint-platform-version={}", ver.trim()));
    }
    if let Some(cores) = fp.hardware_concurrency {
        args.push(format!("--fingerprint-hardware-concurrency={}", cores));
    }
    if let Some(ram) = fp.device_memory_gb {
        args.push(format!("--fingerprint-device-memory={}", ram));
    }
    if let Some(ref lang) = fp.lang
        && !lang.trim().is_empty()
    {
        args.push(format!("--lang={}", lang.trim()));
    }
    if let Some(ref accept_lang) = fp.accept_lang
        && !accept_lang.trim().is_empty()
    {
        args.push(format!("--accept-lang={}", accept_lang.trim()));
    }
    if !has_grid && let (Some(w), Some(h)) = (fp.screen_width, fp.screen_height) {
        args.push(format!("--window-size={},{}", w, h));
    }
    if let Some(dpr) = fp.device_pixel_ratio {
        args.push(format!("--force-device-scale-factor={:.2}", dpr));
    }
    if has_proxy && fp.webrtc_mode.as_deref() != Some("default") {
        args.push("--disable-non-proxied-udp".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_startup_flags_blocks_sandbox_escapes() {
        assert!(sanitize_startup_flags(&["--no-sandbox".to_string()]).is_err());
        assert!(sanitize_startup_flags(&["--user-data-dir=/tmp/x".to_string()]).is_err());
        assert!(sanitize_startup_flags(&["badflag".to_string()]).is_err());
        let ok =
            sanitize_startup_flags(&[" --mute-audio ".to_string(), "--mute-audio".to_string()])
                .unwrap();
        assert_eq!(ok, vec!["--mute-audio"]);
    }

    #[test]
    fn push_antidetect_args_respects_grid_and_webrtc() {
        let fp = FingerprintConfig {
            seed: Some(42),
            screen_width: Some(1280),
            screen_height: Some(720),
            webrtc_mode: Some("proxy_shielded".to_string()),
            ..Default::default()
        };
        let mut args = Vec::new();
        push_antidetect_args(&mut args, &fp, true, false);
        assert!(args.contains(&"--fingerprint=42".to_string()));
        assert!(args.contains(&"--window-size=1280,720".to_string()));
        assert!(args.contains(&"--disable-non-proxied-udp".to_string()));

        let mut grid_args = Vec::new();
        push_antidetect_args(&mut grid_args, &fp, false, true);
        assert!(!grid_args.iter().any(|a| a.starts_with("--window-size=")));
        assert!(!grid_args.contains(&"--disable-non-proxied-udp".to_string()));
    }
}
