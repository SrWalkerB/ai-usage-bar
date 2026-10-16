mod providers;
mod store;

use providers::{FetchError, Usage, claude, codex, now_ms};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use store::{Credential, ProviderId, Settings, Store};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

const WINDOW_WIDTH: f64 = 420.0;
const TRAY_ID: &str = "main";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ErrorView {
    /// "auth" means the user needs to reconnect; "other" is transient.
    kind: &'static str,
    message: String,
}

#[derive(Default, Clone)]
struct ProviderResult {
    usage: Option<Usage>,
    error: Option<ErrorView>,
    fetched_at: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderView {
    id: ProviderId,
    name: &'static str,
    connected: bool,
    source: Option<&'static str>,
    cli_available: bool,
    usage: Option<Usage>,
    error: Option<ErrorView>,
    fetched_at: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppView {
    providers: Vec<ProviderView>,
    last_sync: Option<i64>,
    settings: Settings,
}

struct AppState {
    store: Mutex<Store>,
    results: Mutex<BTreeMap<ProviderId, ProviderResult>>,
    last_sync: Mutex<Option<i64>>,
    http: reqwest::Client,
    claude_pending: Mutex<Option<claude::PendingAuth>>,
    codex_attempt: AtomicU64,
    /// Tray click position (physical px), used to anchor the popup.
    tray_anchor: Mutex<Option<(i32, i32)>>,
    /// Logical height the UI last asked for; used for positioning because the
    /// native size may not be updated yet right after a resize.
    panel_height: Mutex<f64>,
    last_hidden: Mutex<Instant>,
    refresh_lock: tokio::sync::Mutex<()>,
    schedule_changed: tokio::sync::Notify,
}

fn build_view(state: &AppState) -> AppView {
    let store = state.store.lock().unwrap();
    let results = state.results.lock().unwrap();
    let providers = ProviderId::ALL
        .iter()
        .map(|&id| {
            let cred = store.accounts.get(&id);
            let result = results.get(&id).cloned().unwrap_or_default();
            ProviderView {
                id,
                name: id.display_name(),
                connected: cred.is_some(),
                source: cred.map(Credential::source),
                cli_available: match id {
                    ProviderId::Codex => codex::cli_available(),
                    ProviderId::Claude => claude::cli_available(),
                },
                usage: result.usage,
                error: result.error,
                fetched_at: result.fetched_at,
            }
        })
        .collect();
    AppView { providers, last_sync: *state.last_sync.lock().unwrap(), settings: store.settings.clone() }
}

fn emit_view(app: &AppHandle) -> AppView {
    let view = build_view(&app.state::<AppState>());
    let _ = app.emit("state-changed", &view);
    view
}

fn update_tray_tooltip(app: &AppHandle) {
    let state = app.state::<AppState>();
    let parts: Vec<String> = {
        let store = state.store.lock().unwrap();
        let results = state.results.lock().unwrap();
        store
            .accounts
            .keys()
            .filter_map(|id| {
                let usage = results.get(id)?.usage.as_ref()?;
                let max = usage.windows.iter().map(|w| w.used_percent).fold(f64::NAN, f64::max);
                (!max.is_nan()).then(|| format!("{} {:.0}%", id.display_name(), max))
            })
            .collect()
    };
    let text = if parts.is_empty() { "AI Usage Monitor".to_owned() } else { format!("AI Usage — {}", parts.join(" · ")) };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}

async fn refresh_all(app: &AppHandle) -> AppView {
    let state = app.state::<AppState>();
    let _guard = state.refresh_lock.lock().await;
    let accounts: Vec<(ProviderId, Credential)> =
        state.store.lock().unwrap().accounts.iter().map(|(k, v)| (*k, v.clone())).collect();

    let http = &state.http;
    let results = futures::future::join_all(accounts.iter().map(|(id, cred)| async move {
        let r = match id {
            ProviderId::Codex => codex::fetch(http, cred).await,
            ProviderId::Claude => claude::fetch(http, cred).await,
        };
        (*id, r)
    }))
    .await;

    let now = now_ms();
    {
        let mut store = state.store.lock().unwrap();
        let mut stored = state.results.lock().unwrap();
        let mut changed = false;
        for (id, r) in results {
            let entry = stored.entry(id).or_default();
            match r {
                Ok((usage, new_cred)) => {
                    if let Some(c) = new_cred {
                        // Skip if the user disconnected while we were fetching.
                        if store.accounts.contains_key(&id) {
                            store.accounts.insert(id, c);
                            changed = true;
                        }
                    }
                    *entry = ProviderResult { usage: Some(usage), error: None, fetched_at: Some(now) };
                }
                Err(e) => {
                    entry.error = Some(match e {
                        FetchError::Auth(message) => ErrorView { kind: "auth", message },
                        FetchError::Other(message) => ErrorView { kind: "other", message },
                    });
                }
            }
        }
        if changed {
            let _ = store.save();
        }
        *state.last_sync.lock().unwrap() = Some(now);
    }
    update_tray_tooltip(app);
    emit_view(app)
}

fn spawn_refresh(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        refresh_all(&app).await;
    });
}

fn save_store(state: &AppState) -> Result<(), String> {
    state.store.lock().unwrap().save()
}

// ---------- commands ----------

#[tauri::command]
fn get_state(app: AppHandle) -> AppView {
    build_view(&app.state::<AppState>())
}

#[tauri::command]
async fn refresh(app: AppHandle) -> Result<AppView, String> {
    Ok(refresh_all(&app).await)
}

#[tauri::command]
async fn codex_start_device(app: AppHandle) -> Result<codex::DeviceStart, String> {
    codex::start_device(&app.state::<AppState>().http).await
}

#[tauri::command]
async fn codex_wait_device(app: AppHandle, start: codex::DeviceStart) -> Result<AppView, String> {
    let state = app.state::<AppState>();
    let attempt = state.codex_attempt.fetch_add(1, Ordering::SeqCst) + 1;
    let cred = codex::wait_device(&state.http, &start, || state.codex_attempt.load(Ordering::SeqCst) != attempt).await?;
    state.store.lock().unwrap().accounts.insert(ProviderId::Codex, cred);
    save_store(&state)?;
    Ok(refresh_all(&app).await)
}

#[tauri::command]
fn codex_cancel_device(app: AppHandle) {
    app.state::<AppState>().codex_attempt.fetch_add(1, Ordering::SeqCst);
}

#[tauri::command]
fn claude_start_auth(app: AppHandle) -> String {
    let (url, pending) = claude::start_auth();
    *app.state::<AppState>().claude_pending.lock().unwrap() = Some(pending);
    url
}

#[tauri::command]
async fn claude_finish_auth(app: AppHandle, code: String) -> Result<AppView, String> {
    let state = app.state::<AppState>();
    let pending = state.claude_pending.lock().unwrap().take().ok_or("start the login first")?;
    let cred = match claude::finish_auth(&state.http, &pending, &code).await {
        Ok(c) => c,
        Err(e) => {
            // Keep the pending login so the user can retry pasting.
            *state.claude_pending.lock().unwrap() = Some(pending);
            return Err(e);
        }
    };
    state.store.lock().unwrap().accounts.insert(ProviderId::Claude, cred);
    save_store(&state)?;
    Ok(refresh_all(&app).await)
}

#[tauri::command]
async fn import_cli(app: AppHandle, provider: ProviderId) -> Result<AppView, String> {
    let available = match provider {
        ProviderId::Codex => codex::cli_available(),
        ProviderId::Claude => claude::cli_available(),
    };
    if !available {
        return Err("cli-not-found".into());
    }
    let state = app.state::<AppState>();
    state.store.lock().unwrap().accounts.insert(provider, Credential::Cli);
    save_store(&state)?;
    Ok(refresh_all(&app).await)
}

#[tauri::command]
fn disconnect(app: AppHandle, provider: ProviderId) -> Result<AppView, String> {
    let state = app.state::<AppState>();
    state.store.lock().unwrap().accounts.remove(&provider);
    state.results.lock().unwrap().remove(&provider);
    save_store(&state)?;
    update_tray_tooltip(&app);
    Ok(emit_view(&app))
}

#[tauri::command]
fn set_settings(app: AppHandle, mut settings: Settings) -> Result<AppView, String> {
    settings.refresh_minutes = settings.refresh_minutes.clamp(1, 60);
    let state = app.state::<AppState>();
    let autolaunch = app.autolaunch();
    if autolaunch.is_enabled().unwrap_or(false) != settings.autostart {
        let r = if settings.autostart { autolaunch.enable() } else { autolaunch.disable() };
        r.map_err(|e| e.to_string())?;
    }
    state.store.lock().unwrap().settings = settings;
    save_store(&state)?;
    state.schedule_changed.notify_one();
    Ok(emit_view(&app))
}

#[tauri::command]
fn fit_window(app: AppHandle, height: f64) {
    let height = height.clamp(120.0, 900.0);
    *app.state::<AppState>().panel_height.lock().unwrap() = height;
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_size(LogicalSize::new(WINDOW_WIDTH, height));
        position_window(&app, &win);
    }
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ---------- window / tray ----------

/// Places the popup next to the tray icon, inside the monitor's work area
/// (above the taskbar when it is at the bottom, below it when at the top).
fn position_window(app: &AppHandle, win: &WebviewWindow) {
    let anchor = *app.state::<AppState>().tray_anchor.lock().unwrap();
    let monitor = anchor
        .and_then(|(x, y)| win.monitor_from_point(x as f64, y as f64).ok().flatten())
        .or_else(|| win.primary_monitor().ok().flatten());
    let Some(m) = monitor else { return };
    let scale = m.scale_factor();
    let height = *app.state::<AppState>().panel_height.lock().unwrap();
    let wa = m.work_area();
    let margin = (12.0 * scale) as i32;
    let (left, top) = (wa.position.x, wa.position.y);
    let (right, bottom) = (left + wa.size.width as i32, top + wa.size.height as i32);
    let (w, h) = ((WINDOW_WIDTH * scale).round() as i32, (height * scale).round() as i32);
    let (cx, cy) = anchor.unwrap_or((right, bottom));
    let x = (cx - w / 2).clamp(left + margin, (right - w - margin).max(left + margin));
    let y = if cy > (top + bottom) / 2 { bottom - h - margin } else { top + margin };
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

fn show_window(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else { return };
    position_window(app, &win);
    let _ = win.show();
    let _ = win.unminimize();
    let _ = win.set_focus();
    let _ = app.emit("window-shown", ());
    let stale = app.state::<AppState>().last_sync.lock().unwrap().is_none_or(|t| now_ms() - t > 60_000);
    if stale {
        spawn_refresh(app);
    }
}

fn toggle_window(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else { return };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else if app.state::<AppState>().last_hidden.lock().unwrap().elapsed() > Duration::from_millis(300) {
        // Clicking the tray while open first blurs (hides) the window; don't reopen it immediately.
        show_window(app);
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open / Abrir", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh / Atualizar", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit / Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &refresh, &PredefinedMenuItem::separator(app)?, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("AI Usage Monitor")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app),
            "refresh" => spawn_refresh(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, position, .. } =
                event
            {
                let app = tray.app_handle();
                *app.state::<AppState>().tray_anchor.lock().unwrap() = Some((position.x as i32, position.y as i32));
                toggle_window(app);
            }
        })
        .build(app)?;
    Ok(())
}

fn spawn_scheduler(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        refresh_all(&app).await;
        let state = app.state::<AppState>();
        loop {
            let minutes = state.store.lock().unwrap().settings.refresh_minutes.max(1) as u64;
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(minutes * 60)) => { refresh_all(&app).await; }
                _ = state.schedule_changed.notified() => {}
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            let path = app.path().app_data_dir()?.join("data.json");
            let http = reqwest::Client::builder()
                .user_agent(concat!("ai-usage-monitor/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(20))
                .build()?;
            app.manage(AppState {
                store: Mutex::new(Store::load(path)),
                results: Mutex::default(),
                last_sync: Mutex::default(),
                http,
                claude_pending: Mutex::default(),
                codex_attempt: AtomicU64::new(0),
                tray_anchor: Mutex::default(),
                panel_height: Mutex::new(360.0),
                last_hidden: Mutex::new(Instant::now() - Duration::from_secs(1)),
                refresh_lock: tokio::sync::Mutex::new(()),
                schedule_changed: tokio::sync::Notify::new(),
            });
            setup_tray(app.handle())?;
            spawn_scheduler(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Focused(false) => {
                *window.state::<AppState>().last_hidden.lock().unwrap() = Instant::now();
                let _ = window.hide();
            }
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            refresh,
            codex_start_device,
            codex_wait_device,
            codex_cancel_device,
            claude_start_auth,
            claude_finish_auth,
            import_cli,
            disconnect,
            set_settings,
            fit_window,
            quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
