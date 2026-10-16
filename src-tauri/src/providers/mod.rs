pub mod claude;
pub mod codex;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// What kind of limit window this is. Derived from the window length, never from
/// its position in the API response, so plans with only a weekly limit render one bar.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WindowKind {
    Session,
    Daily,
    Weekly,
    Monthly,
    Other,
}

impl WindowKind {
    pub fn from_seconds(secs: i64) -> Self {
        const H: i64 = 3600;
        match secs {
            s if s <= 0 => WindowKind::Other,
            s if s <= 12 * H => WindowKind::Session,
            s if s <= 36 * H => WindowKind::Daily,
            s if s <= 8 * 24 * H => WindowKind::Weekly,
            s if s <= 32 * 24 * H => WindowKind::Monthly,
            _ => WindowKind::Other,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub kind: WindowKind,
    /// Extra qualifier such as a model name ("Opus"); None for the main limits.
    pub label: Option<String>,
    pub used_percent: f64,
    /// Unix milliseconds.
    pub resets_at: Option<i64>,
    pub window_seconds: Option<i64>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub plan: Option<String>,
    pub windows: Vec<UsageWindow>,
}

#[derive(Debug)]
pub enum FetchError {
    /// Credentials are missing/expired/revoked: the user must reconnect.
    Auth(String),
    Other(String),
}

impl From<reqwest::Error> for FetchError {
    fn from(e: reqwest::Error) -> Self {
        FetchError::Other(e.to_string())
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

pub fn random_urlsafe(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

pub fn pkce() -> Pkce {
    let verifier = random_urlsafe(32);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    Pkce { verifier, challenge }
}

/// Decodes a JWT payload without verifying it (we only read our own tokens' claims).
pub fn jwt_claims(token: &str) -> Value {
    token
        .split('.')
        .nth(1)
        .and_then(|p| URL_SAFE_NO_PAD.decode(p.trim_end_matches('=')).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null)
}

/// Reads a number that may be encoded as a JSON number or a numeric string.
pub fn as_f64(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

pub async fn error_from_response(resp: reqwest::Response) -> FetchError {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    let snippet: String = body.chars().take(200).collect();
    let msg = format!("HTTP {status}: {snippet}");
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        FetchError::Auth(msg)
    } else {
        FetchError::Other(msg)
    }
}
