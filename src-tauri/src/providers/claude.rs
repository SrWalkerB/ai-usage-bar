//! Claude (Pro/Max plan) — OAuth PKCE login with pasted code, and plan usage.
//! Endpoints match the ones the Claude Code CLI uses.

use super::{FetchError, Usage, UsageWindow, WindowKind, as_f64, error_from_response, now_ms, pkce, random_urlsafe};
use crate::store::Credential;
use serde_json::{Value, json};
use std::path::PathBuf;

const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const AUTHORIZE_URL: &str = "https://claude.com/cai/oauth/authorize";
const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const REDIRECT_URI: &str = "https://platform.claude.com/oauth/code/callback";
const SCOPES: &str = "user:profile user:inference";
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const OAUTH_BETA: &str = "oauth-2025-04-20";

pub struct PendingAuth {
    verifier: String,
    state: String,
}

pub fn start_auth() -> (String, PendingAuth) {
    let p = pkce();
    let state = random_urlsafe(32);
    let url = reqwest::Url::parse_with_params(
        AUTHORIZE_URL,
        &[
            ("code", "true"),
            ("client_id", CLIENT_ID),
            ("response_type", "code"),
            ("redirect_uri", REDIRECT_URI),
            ("scope", SCOPES),
            ("code_challenge", p.challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", state.as_str()),
        ],
    )
    .expect("static url")
    .to_string();
    (url, PendingAuth { verifier: p.verifier, state })
}

/// Accepts what the callback page shows (`code#state`), a bare code, or the full callback URL.
fn parse_pasted(input: &str) -> (String, Option<String>) {
    let input = input.trim();
    if let Ok(url) = reqwest::Url::parse(input) {
        let get = |k: &str| url.query_pairs().find(|(key, _)| key == k).map(|(_, v)| v.into_owned());
        if let Some(code) = get("code") {
            return (code, get("state"));
        }
    }
    match input.split_once('#') {
        Some((code, state)) => (code.to_owned(), Some(state.to_owned())),
        None => (input.to_owned(), None),
    }
}

pub async fn finish_auth(http: &reqwest::Client, pending: &PendingAuth, pasted: &str) -> Result<Credential, String> {
    let (code, state) = parse_pasted(pasted);
    if code.is_empty() {
        return Err("empty code".into());
    }
    if state.as_deref().is_some_and(|s| s != pending.state) {
        return Err("state mismatch — start the login again".into());
    }
    let resp = http
        .post(TOKEN_URL)
        .json(&json!({
            "grant_type": "authorization_code",
            "code": code,
            "state": pending.state,
            "client_id": CLIENT_ID,
            "redirect_uri": REDIRECT_URI,
            "code_verifier": pending.verifier,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body: String = resp.text().await.unwrap_or_default().chars().take(200).collect();
        return Err(format!("token exchange failed: HTTP {status} {body}"));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    credential_from_tokens(&v, None).ok_or_else(|| "token response missing fields".into())
}

fn credential_from_tokens(v: &Value, prev_refresh: Option<&str>) -> Option<Credential> {
    let access_token = v["access_token"].as_str()?.to_owned();
    let refresh_token = v["refresh_token"].as_str().or(prev_refresh)?.to_owned();
    let expires_at = v["expires_in"].as_i64().map(|s| now_ms() + s * 1000);
    Some(Credential::Oauth { access_token, refresh_token, expires_at, account_id: None })
}

async fn refresh(http: &reqwest::Client, refresh_token: &str) -> Result<Credential, FetchError> {
    let resp = http
        .post(TOKEN_URL)
        .json(&json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "client_id": CLIENT_ID,
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
    credential_from_tokens(&v, Some(refresh_token)).ok_or_else(|| FetchError::Other("bad refresh response".into()))
}

fn cli_credentials_path() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".claude")))
        .map(|d| d.join(".credentials.json"))
}

/// Returns (access token, subscription type).
fn read_cli() -> Result<(String, Option<String>), FetchError> {
    let path = cli_credentials_path().ok_or_else(|| FetchError::Auth("no home dir".into()))?;
    let raw = std::fs::read_to_string(&path).map_err(|_| FetchError::Auth("Claude Code is not logged in".into()))?;
    let v: Value = serde_json::from_str(&raw).map_err(|e| FetchError::Other(e.to_string()))?;
    let oauth = &v["claudeAiOauth"];
    let token = oauth["accessToken"]
        .as_str()
        .ok_or_else(|| FetchError::Auth("Claude Code is not logged in with a Claude account".into()))?;
    if oauth["expiresAt"].as_i64().is_some_and(|t| t < now_ms()) {
        return Err(FetchError::Auth("Claude Code token expired — open Claude Code to renew it".into()));
    }
    Ok((token.to_owned(), oauth["subscriptionType"].as_str().map(str::to_owned)))
}

pub fn cli_available() -> bool {
    cli_credentials_path().is_some_and(|p| {
        std::fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .is_some_and(|v| v["claudeAiOauth"]["accessToken"].is_string())
    })
}

pub async fn fetch(http: &reqwest::Client, cred: &Credential) -> Result<(Usage, Option<Credential>), FetchError> {
    match cred {
        Credential::Cli => {
            let (token, plan) = read_cli()?;
            let mut usage = call_usage(http, &token).await?;
            usage.plan = plan;
            Ok((usage, None))
        }
        Credential::Oauth { access_token, refresh_token, expires_at, .. } => {
            let mut updated = None;
            if expires_at.is_some_and(|t| t - now_ms() < 60_000) {
                updated = Some(refresh(http, refresh_token).await?);
            }
            let token = match &updated {
                Some(Credential::Oauth { access_token, .. }) => access_token.as_str(),
                _ => access_token.as_str(),
            };
            match call_usage(http, token).await {
                Err(FetchError::Auth(_)) if updated.is_none() => {
                    let fresh = refresh(http, refresh_token).await?;
                    let Credential::Oauth { access_token, .. } = &fresh else { unreachable!() };
                    let usage = call_usage(http, access_token).await?;
                    Ok((usage, Some(fresh)))
                }
                other => Ok((other?, updated)),
            }
        }
    }
}

async fn call_usage(http: &reqwest::Client, token: &str) -> Result<Usage, FetchError> {
    let resp = http
        .get(USAGE_URL)
        .bearer_auth(token)
        .header("anthropic-beta", OAUTH_BETA)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(error_from_response(resp).await);
    }
    let v: Value = resp.json().await?;
    Ok(parse_usage(&v))
}

fn title_case(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The response is an object of named windows (`five_hour`, `seven_day`, `seven_day_opus`, ...),
/// each `{ utilization, resets_at }` or null. Only non-null windows become bars.
pub fn parse_usage(v: &Value) -> Usage {
    let mut windows = Vec::new();
    if let Some(obj) = v.as_object() {
        for (key, w) in obj {
            let Some(used_percent) = w.get("utilization").and_then(as_f64) else { continue };
            // The response also carries internal/codename windows; only show the known ones.
            let (secs, rest) = match key.as_str() {
                "five_hour" => (5 * 3600, ""),
                "seven_day" | "seven_day_opus" | "seven_day_sonnet" => (7 * 24 * 3600, &key["seven_day".len()..]),
                _ => continue,
            };
            let label = Some(title_case(rest)).filter(|s| !s.is_empty());
            let resets_at = w["resets_at"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp_millis());
            windows.push(UsageWindow {
                kind: WindowKind::from_seconds(secs),
                label,
                used_percent,
                resets_at,
                window_seconds: Some(secs),
            });
        }
    }
    windows.sort_by_key(|w| (w.label.is_some(), w.window_seconds, w.label.clone()));
    Usage { plan: None, windows }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_windows_and_skips_nulls() {
        let v = json!({
            "five_hour": { "utilization": 72.0, "resets_at": "2026-10-16T20:00:00Z" },
            "seven_day": { "utilization": 34.0, "resets_at": "2026-10-28T00:00:00+00:00" },
            "seven_day_opus": null,
            "seven_day_sonnet": { "utilization": 5, "resets_at": null },
            "extra_usage": { "is_enabled": false }
        });
        let u = parse_usage(&v);
        let kinds: Vec<_> = u.windows.iter().map(|w| (w.kind, w.label.clone())).collect();
        assert_eq!(
            kinds,
            vec![(WindowKind::Session, None), (WindowKind::Weekly, None), (WindowKind::Weekly, Some("Sonnet".into()))]
        );
    }

    #[test]
    fn pasted_code_formats() {
        assert_eq!(parse_pasted(" abc#xyz "), ("abc".into(), Some("xyz".into())));
        assert_eq!(parse_pasted("abc"), ("abc".into(), None));
        assert_eq!(
            parse_pasted("https://platform.claude.com/oauth/code/callback?code=abc&state=xyz"),
            ("abc".into(), Some("xyz".into()))
        );
    }
}
