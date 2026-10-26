//! OpenCode Go: there is no usage API, so spend is summed from the OpenCode CLI's local
//! database (`opencode.db`) and compared with the plan's rolling dollar limits.
//! This only sees usage made on this machine.

use super::{FetchError, Usage, UsageWindow, WindowKind, now_ms};
use crate::store::Credential;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 24 * HOUR_MS;

/// Go plan allowances in dollars: (rolling 5h, rolling week, rolling month).
const LIMITS: [(i64, f64); 3] = [(5 * HOUR_MS, 12.0), (7 * DAY_MS, 30.0), (30 * DAY_MS, 60.0)];

fn db_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))?;
    Some(base.join("opencode").join("opencode.db"))
}

pub fn cli_available() -> bool {
    db_path().is_some_and(|p| p.is_file())
}

/// Costs (dollars) of OpenCode Go assistant messages since `since_ms`, as (created_ms, cost).
fn read_costs(since_ms: i64) -> Result<Vec<(i64, f64)>, FetchError> {
    let path = db_path().filter(|p| p.is_file()).ok_or_else(|| FetchError::Auth("opencode.db not found".into()))?;
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| FetchError::Other(e.to_string()))?;
    let mut stmt = conn
        .prepare(
            "SELECT time_created, COALESCE(json_extract(data, '$.cost'), 0) FROM session_message \
             WHERE type = 'assistant' AND json_extract(data, '$.model.providerID') = 'opencode-go' \
             AND time_created >= ?1 ORDER BY time_created",
        )
        .map_err(|e| FetchError::Other(e.to_string()))?;
    let rows = stmt
        .query_map([since_ms], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?)))
        .map_err(|e| FetchError::Other(e.to_string()))?;
    rows.collect::<Result<_, _>>().map_err(|e| FetchError::Other(e.to_string()))
}

/// Pure: turns message costs into rolling windows. A rolling window frees up
/// when its oldest message ages out, which is when we report it as resetting.
pub fn parse_usage(costs: &[(i64, f64)], now: i64) -> Usage {
    let windows = LIMITS
        .iter()
        .map(|&(len, limit)| {
            let inside: Vec<_> = costs.iter().filter(|(t, _)| *t > now - len).collect();
            let spent: f64 = inside.iter().map(|(_, c)| c).sum();
            UsageWindow {
                kind: WindowKind::from_seconds(len / 1000),
                label: None,
                used_percent: (spent / limit * 100.0).clamp(0.0, 100.0),
                resets_at: inside.first().map(|(t, _)| t + len),
                window_seconds: Some(len / 1000),
            }
        })
        .collect();
    Usage { plan: Some("go".into()), windows }
}

pub async fn fetch(_http: &reqwest::Client, _cred: &Credential) -> Result<(Usage, Option<Credential>), FetchError> {
    let now = now_ms();
    let max_len = LIMITS.iter().map(|l| l.0).max().unwrap_or(0);
    let costs = tokio::task::spawn_blocking(move || read_costs(now - max_len))
        .await
        .map_err(|e| FetchError::Other(e.to_string()))??;
    Ok((parse_usage(&costs, now), None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_rolling_windows() {
        let now = 100 * DAY_MS;
        let costs = [(now - 6 * HOUR_MS, 3.0), (now - HOUR_MS, 6.0), (now - 2 * DAY_MS, 9.0)];
        let u = parse_usage(&costs, now);
        assert_eq!(u.windows.len(), 3);
        assert_eq!(u.windows[0].kind, WindowKind::Session);
        assert!((u.windows[0].used_percent - 50.0).abs() < 1e-9); // only the 1h-old message
        assert_eq!(u.windows[0].resets_at, Some(now - HOUR_MS + 5 * HOUR_MS));
        assert!((u.windows[1].used_percent - 60.0).abs() < 1e-9); // 18 / 30
        assert!((u.windows[2].used_percent - 30.0).abs() < 1e-9); // 18 / 60
    }

    #[test]
    fn empty_usage_has_no_reset() {
        let u = parse_usage(&[], 1_000_000_000);
        assert!(u.windows.iter().all(|w| w.used_percent == 0.0 && w.resets_at.is_none()));
    }
}
