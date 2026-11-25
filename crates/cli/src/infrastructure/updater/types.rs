use serde::{Deserialize, Serialize};

pub const GITHUB_REPO: &str = "tuquet/operon";
pub const CACHE_TTL_SECS: u64 = 86400; // 24 hours

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
    pub checked_at: u64,
}

/// Strip symmetric (`core-v`) or legacy (`v`) release tag prefix.
pub fn normalize_tag(raw: &str) -> &str {
    let trimmed = raw.trim();
    trimmed
        .strip_prefix("core-v")
        .or_else(|| trimmed.strip_prefix('v'))
        .unwrap_or(trimmed)
}

pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let parse_ver = |v: &str| -> Vec<u32> {
        normalize_tag(v)
            .split(['.', '-'])
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    };

    let cur_parts = parse_ver(current);
    let lat_parts = parse_ver(latest);

    for (c, l) in cur_parts.iter().zip(lat_parts.iter()) {
        if l > c {
            return true;
        } else if l < c {
            return false;
        }
    }

    lat_parts.len() > cur_parts.len()
}
