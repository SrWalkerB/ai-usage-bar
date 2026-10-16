//! Codex (ChatGPT plan) — device-code login and rate-limit usage.
//! Mirrors the flow in openai/codex `codex-rs/login/src/device_code_auth.rs`.

use super::{FetchError, Usage, UsageWindow, WindowKind, as_f64, error_from_response, jwt_claims, now_ms};
use crate::store::Credential;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};

const ISSUER: &str = "https://auth.openai.com";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const DEVICE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStart {
    pub user_code: String,
    pub verification_url: String,
    pub device_auth_id: String,
    pub interval: u64,
}

pub async fn start_device(http: &reqwest::Client) -> Result<DeviceStart, String> {
    let resp = http
        .post(format!("{ISSUER}/api/accounts/deviceauth/usercode"))
        .json(&json!({ "client_id": CLIENT_ID }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("device code request failed: HTTP {}", resp.status()));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    let str_field = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    Ok(DeviceStart {
        user_code: str_field("user_code").or_else(|| str_field("usercode")).ok_or("missing user_code")?,
        device_auth_id: str_field("device_auth_id").ok_or("missing device_auth_id")?,
        interval: v.get("interval").and_then(as_f64).unwrap_or(5.0).max(1.0) as u64,
        verification_url: format!("{ISSUER}/codex/device"),
    })
}

/// Polls until the user approves the code, then exchanges it for tokens.
/// `cancelled` is checked between polls so the UI can abort.
pub async fn wait_device(
    http: &reqwest::Client,
    start: &DeviceStart,
    cancelled: impl Fn() -> bool,
) -> Result<Credential, String> {
    let deadline = tokio::time::Instant::now() + DEVICE_TIMEOUT;
    let success: Value = loop {
        if cancelled() {
            return Err("cancelled".into());
        }
        let resp = http
            .post(format!("{ISSUER}/api/accounts/deviceauth/token"))
            .json(&json!({ "device_auth_id": start.device_auth_id, "user_code": start.user_code }))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        if status.is_success() {
            break resp.json().await.map_err(|e| e.to_string())?;
        }
        if status != reqwest::StatusCode::FORBIDDEN && status != reqwest::StatusCode::NOT_FOUND {
            return Err(format!("device auth failed: HTTP {status}"));
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("device auth timed out".into());
        }
        tokio::time::sleep(Duration::from_secs(start.interval)).await;
    };

    let code = success["authorization_code"].as_str().ok_or("missing authorization_code")?;
    let verifier = success["code_verifier"].as_str().ok_or("missing code_verifier")?;
    let redirect_uri = format!("{ISSUER}/deviceauth/callback");
    let resp = http
        .post(format!("{ISSUER}/oauth/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri.as_str()),
            ("client_id", CLIENT_ID),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("token exchange failed: HTTP {}", resp.status()));
    }
    let tokens: Value = resp.json().await.map_err(|e| e.to_string())?;
    credential_from_tokens(&tokens, None).ok_or_else(|| "token response missing fields".into())
}

fn account_id_from(token: &str) -> Option<String> {
    jwt_claims(token)["https://api.openai.com/auth"]["chatgpt_account_id"]
        .as_str()
        .map(str::to_owned)
}

fn credential_from_tokens(v: &Value, prev: Option<&Credential>) -> Option<Credential> {
    let access_token = v["access_token"].as_str()?.to_owned();
    let (prev_refresh, prev_account) = match prev {
        Some(Credential::Oauth { refresh_token, account_id, .. }) => (Some(refresh_token.clone()), account_id.clone()),
        _ => (None, None),
    };
    let refresh_token = v["refresh_token"].as_str().map(str::to_owned).or(prev_refresh)?;
    let account_id = v["id_token"]
        .as_str()
        .and_then(account_id_from)
        .or_else(|| account_id_from(&access_token))
        .or(prev_account);
    let expires_at = jwt_claims(&access_token)["exp"].as_i64().map(|s| s * 1000);
    Some(Credential::Oauth { access_token, refresh_token, expires_at, account_id })
}

async fn refresh(http: &reqwest::Client, cred: &Credential) -> Result<Credential, FetchError> {
    let Credential::Oauth { refresh_token, .. } = cred else {
        return Err(FetchError::Auth("not an oauth credential".into()));
    };
    let resp = http
        .post(format!("{ISSUER}/oauth/token"))
        .json(&json!({
            "client_id": CLIENT_ID,
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "scope": "openid profile email",
        }))
        .send()
        .await?;
    if resp.status().is_client_error() {
        return Err(FetchError::Auth(format!("refresh failed: HTTP {}", resp.status())));
    }
    if !resp.status().is_success() {
        return Err(error_from_response(resp).await);
    }
    let v: Value = resp.json().await?;
    credential_from_tokens(&v, Some(cred)).ok_or_else(|| FetchError::Other("bad refresh response".into()))
}

fn cli_auth_path() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".codex")))
        .map(|d| d.join("auth.json"))
}

fn read_cli() -> Result<(String, Option<String>), FetchError> {
    let path = cli_auth_path().ok_or_else(|| FetchError::Auth("no home dir".into()))?;
    let raw = std::fs::read_to_string(&path).map_err(|_| FetchError::Auth("Codex CLI is not logged in".into()))?;
    let v: Value = serde_json::from_str(&raw).map_err(|e| FetchError::Other(e.to_string()))?;
    let token = v["tokens"]["access_token"]
        .as_str()
        .ok_or_else(|| FetchError::Auth("Codex CLI is not logged in with ChatGPT".into()))?;
    let account = v["tokens"]["account_id"].as_str().map(str::to_owned).or_else(|| account_id_from(token));
    Ok((token.to_owned(), account))
}

pub fn cli_available() -> bool {
    read_cli().is_ok()
}

/// Fetches usage. Returns an updated credential when tokens were refreshed.
pub async fn fetch(http: &reqwest::Client, cred: &Credential) -> Result<(Usage, Option<Credential>), FetchError> {
    match cred {
        Credential::Cli => {
            let (token, account) = read_cli()?;
            Ok((call_usage(http, &token, account.as_deref()).await?, None))
        }
        Credential::Oauth { expires_at, .. } => {
            let mut current = cred.clone();
            let mut refreshed = false;
            if expires_at.is_some_and(|t| t - now_ms() < 60_000) {
                current = refresh(http, &current).await?;
                refreshed = true;
            }
            loop {
                let Credential::Oauth { access_token, account_id, .. } = &current else { unreachable!() };
                match call_usage(http, access_token, account_id.as_deref()).await {
                    Err(FetchError::Auth(_)) if !refreshed => {
                        current = refresh(http, &current).await?;
                        refreshed = true;
                    }
                    Err(e) => return Err(e),
                    Ok(usage) => return Ok((usage, refreshed.then_some(current))),
                }
            }
        }
    }
}

async fn call_usage(http: &reqwest::Client, token: &str, account: Option<&str>) -> Result<Usage, FetchError> {
    let mut req = http.get(USAGE_URL).bearer_auth(token).header("User-Agent", "codex-cli");
    if let Some(acc) = account {
        req = req.header("ChatGPT-Account-Id", acc);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        return Err(error_from_response(resp).await);
    }
    let v: Value = resp.json().await?;
    Ok(parse_usage(&v))
}

fn parse_window(v: &Value, label: Option<String>) -> Option<UsageWindow> {
    let used_percent = as_f64(v.get("used_percent")?)?;
    let window_seconds = v.get("limit_window_seconds").and_then(as_f64).map(|s| s as i64);
    let resets_at = v
        .get("reset_at")
        .and_then(as_f64)
        .map(|s| (s * 1000.0) as i64)
        .or_else(|| v.get("reset_after_seconds").and_then(as_f64).map(|s| now_ms() + (s * 1000.0) as i64));
    Some(UsageWindow {
        kind: window_seconds.map(WindowKind::from_seconds).unwrap_or(WindowKind::Other),
        label,
        used_percent,
        resets_at,
        window_seconds,
    })
}

fn push_windows(out: &mut Vec<UsageWindow>, rate_limit: &Value, label: Option<String>) {
    for key in ["primary_window", "secondary_window"] {
        if let Some(w) = rate_limit.get(key).filter(|w| w.is_object()).and_then(|w| parse_window(w, label.clone())) {
            out.push(w);
        }
    }
}

pub fn parse_usage(v: &Value) -> Usage {
    let mut windows = Vec::new();
    push_windows(&mut windows, &v["rate_limit"], None);
    if let Some(extra) = v["additional_rate_limits"].as_array() {
        for item in extra {
            let label = item["limit_name"].as_str().or(item["metered_feature"].as_str()).map(str::to_owned);
            push_windows(&mut windows, &item["rate_limit"], label);
        }
    }
    windows.sort_by_key(|w| (w.label.is_some(), w.window_seconds.unwrap_or(i64::MAX)));
    Usage { plan: v["plan_type"].as_str().map(str::to_owned), windows }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_only_plan_yields_single_weekly_bar() {
        let v = json!({
            "plan_type": "team",
            "rate_limit": {
                "primary_window": { "used_percent": 68, "limit_window_seconds": 604800, "reset_at": 1_800_000_000 },
                "secondary_window": null
            }
        });
        let u = parse_usage(&v);
        assert_eq!(u.windows.len(), 1);
        assert_eq!(u.windows[0].kind, WindowKind::Weekly);
        assert_eq!(u.windows[0].resets_at, Some(1_800_000_000_000));
    }

    #[test]
    fn session_and_weekly_sorted_by_length() {
        let v = json!({
            "rate_limit": {
                "primary_window": { "used_percent": 10, "limit_window_seconds": 604800 },
                "secondary_window": { "used_percent": 50, "limit_window_seconds": 18000 }
            }
        });
        let u = parse_usage(&v);
        assert_eq!(u.windows.iter().map(|w| w.kind).collect::<Vec<_>>(), vec![WindowKind::Session, WindowKind::Weekly]);
    }
}
