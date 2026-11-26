use super::client::CloudApiClient;
use super::credential_store::{CredentialStore, WorkspaceInfo};
use super::supabase_auth::SupabaseAuthClient;
use reqwest::Client;
use serde_json::Value;

pub struct WorkspaceManager;

impl WorkspaceManager {
    /// Fetch all workspaces/workspaces the authenticated user belongs to via Supabase PostgREST
    pub async fn fetch_user_workspaces(
        client: &Client,
        cloud_url: &str,
        api_key: &str,
        access_token: &str,
        user_id: &str,
    ) -> Result<Vec<WorkspaceInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let base = cloud_url.trim_end_matches('/');

        // 1. Fetch workspaces filtered by RLS
        let workspaces_url = format!(
            "{}/rest/v1/workspaces?select=id,slug,name,status,created_by",
            base
        );
        let res = client
            .get(&workspaces_url)
            .header("apikey", api_key)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(format!("Failed to fetch workspaces from Supabase: {}", err_text).into());
        }

        let workspaces_json: Vec<Value> = res.json().await?;

        // 2. Fetch user's membership and roles in workspaces
        let members_url = format!(
            "{}/rest/v1/workspace_members?select=workspace_id,status,member_roles(role_id,roles(name,display_name))&user_id=eq.{}",
            base, user_id
        );
        let member_res = client
            .get(&members_url)
            .header("apikey", api_key)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await;

        let mut role_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        if let Ok(m_res) = member_res
            && m_res.status().is_success()
            && let Ok(members_json) = m_res.json::<Vec<Value>>().await
        {
            for m in members_json {
                if let Some(tid) = m.get("workspace_id").and_then(|v| v.as_str()) {
                    // Extract role name if present
                    if let Some(mr_array) = m.get("member_roles").and_then(|v| v.as_array()) {
                        for mr in mr_array {
                            if let Some(role_obj) = mr.get("roles")
                                && let Some(name) = role_obj.get("name").and_then(|v| v.as_str())
                            {
                                role_map.insert(tid.to_string(), name.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }

        let mut results = Vec::new();
        for t in workspaces_json {
            let id = t
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let slug = t
                .get("slug")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let name = t
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let status = t
                .get("status")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let created_by = t
                .get("created_by")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            // Default role is 'owner' if created_by matches user_id, or mapped role, or 'member'
            let role = if created_by == user_id {
                Some("owner".to_string())
            } else {
                role_map
                    .get(&id)
                    .cloned()
                    .or_else(|| Some("member".to_string()))
            };

            results.push(WorkspaceInfo {
                id,
                slug,
                name,
                role,
                status,
                is_active: false,
            });
        }

        Ok(results)
    }

    /// Select or resolve the active workspace from available workspaces
    pub fn resolve_active_workspace<'a>(
        workspaces: &'a mut [WorkspaceInfo],
        requested: Option<&str>,
        fallback_workspace_id: Option<&str>,
    ) -> Result<&'a mut WorkspaceInfo, Box<dyn std::error::Error + Send + Sync>> {
        if workspaces.is_empty() {
            return Err("No accessible workspaces found for this user account".into());
        }

        let target_idx = if let Some(req) = requested {
            let req_clean = req.trim();
            workspaces
                .iter()
                .position(|t| {
                    t.slug.eq_ignore_ascii_case(req_clean) || t.id.eq_ignore_ascii_case(req_clean)
                })
                .ok_or_else(|| {
                    format!(
                        "Requested workspace '{}' not found in your accessible workspaces",
                        req
                    )
                })?
        } else if let Some(fb) = fallback_workspace_id {
            workspaces
                .iter()
                .position(|t| t.id.eq_ignore_ascii_case(fb) || t.slug.eq_ignore_ascii_case(fb))
                .unwrap_or(0)
        } else {
            0
        };

        for (i, t) in workspaces.iter_mut().enumerate() {
            t.is_active = i == target_idx;
        }

        Ok(&mut workspaces[target_idx])
    }

    /// Switch active workspace, re-enroll device under target workspace, and persist credentials
    pub async fn switch_workspace(
        data_dir: &str,
        target_slug_or_id: &str,
    ) -> Result<WorkspaceInfo, Box<dyn std::error::Error + Send + Sync>> {
        let mut creds = CredentialStore::load(data_dir)
            .await
            .ok_or("No active cloud session found on this machine. Run 'opn login' first.")?;

        let cloud_url =
            super::credential_store::resolve_supabase_url(None, creds.cloud_url.as_deref())?;
        let api_key = super::credential_store::resolve_supabase_anon_key(creds.api_key.as_deref())?;

        let client = CloudApiClient::build_http_client();

        // 1. Refresh access token if expired
        let access_token = match (creds.access_token.as_ref(), creds.refresh_token.as_ref()) {
            (Some(tok), Some(refresh)) if creds.is_token_expired() => {
                if let Ok(new_sess) =
                    SupabaseAuthClient::refresh_access_token(&client, &cloud_url, &api_key, refresh)
                        .await
                {
                    creds.access_token = Some(new_sess.access_token.clone());
                    creds.refresh_token = new_sess.refresh_token.or(Some(refresh.clone()));
                    creds.token_expires_at = new_sess.expires_at;
                    new_sess.access_token
                } else {
                    tok.clone()
                }
            }
            (Some(tok), _) => tok.clone(),
            _ => {
                return Err(
                    "No active user session found. Please run 'opn login' to authenticate.".into(),
                );
            }
        };

        let user_id = creds.user_id.clone().unwrap_or_default();

        // 2. Refresh workspaces list
        let mut workspaces =
            Self::fetch_user_workspaces(&client, &cloud_url, &api_key, &access_token, &user_id)
                .await
                .unwrap_or_else(|_| creds.available_workspaces.clone().unwrap_or_default());

        let target_workspace =
            Self::resolve_active_workspace(&mut workspaces, Some(target_slug_or_id), None)?.clone();

        // 3. Re-enroll device under target workspace
        let new_device_creds = CloudApiClient::enroll(
            &client,
            &cloud_url,
            Some(&creds.name),
            Some(&target_workspace.slug),
            Some("cli_workspace_switch"),
            Some(&access_token),
        )
        .await?;

        // 4. Update credentials
        creds.device_id = new_device_creds.device_id;
        creds.device_token = new_device_creds.device_token;
        creds.workspace_id = Some(target_workspace.id.clone());
        creds.workspace_slug = Some(target_workspace.slug.clone());
        creds.workspace_name = Some(target_workspace.name.clone());
        creds.workspace_role = target_workspace.role.clone();
        creds.available_workspaces = Some(workspaces);

        CredentialStore::save(data_dir, &creds).await?;

        Ok(target_workspace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_workspaces() -> Vec<WorkspaceInfo> {
        vec![
            WorkspaceInfo {
                id: "uuid-111".to_string(),
                slug: "alpha-corp".to_string(),
                name: "Alpha Corp".to_string(),
                role: Some("owner".to_string()),
                status: Some("active".to_string()),
                is_active: false,
            },
            WorkspaceInfo {
                id: "uuid-222".to_string(),
                slug: "beta-labs".to_string(),
                name: "Beta Labs".to_string(),
                role: Some("member".to_string()),
                status: Some("active".to_string()),
                is_active: false,
            },
            WorkspaceInfo {
                id: "uuid-333".to_string(),
                slug: "gamma-dev".to_string(),
                name: "Gamma Dev".to_string(),
                role: Some("admin".to_string()),
                status: Some("active".to_string()),
                is_active: false,
            },
        ]
    }

    #[test]
    fn test_resolve_active_workspace_by_slug() {
        let mut workspaces = sample_workspaces();
        let active =
            WorkspaceManager::resolve_active_workspace(&mut workspaces, Some("beta-labs"), None)
                .expect("Should resolve beta-labs");
        assert_eq!(active.id, "uuid-222");
        assert_eq!(active.slug, "beta-labs");
        assert!(active.is_active);

        // Verify other workspaces are not active
        assert!(!workspaces[0].is_active);
        assert!(workspaces[1].is_active);
        assert!(!workspaces[2].is_active);
    }

    #[test]
    fn test_resolve_active_workspace_by_id_case_insensitive() {
        let mut workspaces = sample_workspaces();
        let active =
            WorkspaceManager::resolve_active_workspace(&mut workspaces, Some("UUID-333"), None)
                .expect("Should resolve UUID-333 case insensitively");
        assert_eq!(active.slug, "gamma-dev");
        assert!(active.is_active);
        assert!(workspaces[2].is_active);
    }

    #[test]
    fn test_resolve_active_workspace_fallback() {
        let mut workspaces = sample_workspaces();
        let active =
            WorkspaceManager::resolve_active_workspace(&mut workspaces, None, Some("uuid-222"))
                .expect("Should fallback to existing workspace id");
        assert_eq!(active.slug, "beta-labs");
        assert!(active.is_active);
    }

    #[test]
    fn test_resolve_active_workspace_default_first() {
        let mut workspaces = sample_workspaces();
        let active = WorkspaceManager::resolve_active_workspace(&mut workspaces, None, None)
            .expect("Should default to first workspace");
        assert_eq!(active.slug, "alpha-corp");
        assert!(active.is_active);
    }

    #[test]
    fn test_resolve_active_workspace_not_found_err() {
        let mut workspaces = sample_workspaces();
        let res =
            WorkspaceManager::resolve_active_workspace(&mut workspaces, Some("non-existent"), None);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_resolve_active_workspace_empty_err() {
        let mut workspaces: Vec<WorkspaceInfo> = Vec::new();
        let res = WorkspaceManager::resolve_active_workspace(&mut workspaces, None, None);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("No accessible workspaces")
        );
    }
}
