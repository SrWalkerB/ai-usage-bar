//! Persistent app data: settings + connected accounts, kept as JSON in the app data dir.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Codex,
    Claude,
    #[serde(rename = "opencodego")]
    OpenCodeGo,
}

impl ProviderId {
    pub const ALL: [ProviderId; 3] = [ProviderId::Codex, ProviderId::Claude, ProviderId::OpenCodeGo];

    pub fn display_name(self) -> &'static str {
        match self {
            ProviderId::Codex => "Codex",
            ProviderId::Claude => "Claude",
            ProviderId::OpenCodeGo => "OpenCode Go",
        }
    }
}

/// How the app authenticates against a provider.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Credential {
    /// Tokens obtained by this app's own login flow; we refresh them ourselves.
    Oauth {
        access_token: String,
        refresh_token: String,
        #[serde(default)]
        expires_at: Option<i64>,
        #[serde(default)]
        account_id: Option<String>,
    },
    /// Tokens read live from the official CLI's credential file. Never refreshed here,
    /// because refresh tokens rotate and refreshing would log the CLI out.
    Cli,
}

impl Credential {
    pub fn source(&self) -> &'static str {
        match self {
            Credential::Oauth { .. } => "oauth",
            Credential::Cli => "cli",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "system", "en" or "pt-BR".
    pub language: String,
    pub refresh_minutes: u32,
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { language: "system".into(), refresh_minutes: 5, autostart: false }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Store {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub accounts: BTreeMap<ProviderId, Credential>,
    #[serde(skip)]
    path: PathBuf,
}

impl Store {
    pub fn load(path: PathBuf) -> Self {
        let mut store: Store = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        store.path = path;
        store
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}
