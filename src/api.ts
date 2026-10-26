import { invoke } from "@tauri-apps/api/core";

export type ProviderId = "codex" | "claude" | "opencodego";
export type WindowKind = "session" | "daily" | "weekly" | "monthly" | "other";

export interface UsageWindow {
  kind: WindowKind;
  label: string | null;
  usedPercent: number;
  resetsAt: number | null;
  windowSeconds: number | null;
}

export interface ProviderView {
  id: ProviderId;
  name: string;
  connected: boolean;
  source: "oauth" | "cli" | null;
  cliAvailable: boolean;
  usage: { plan: string | null; windows: UsageWindow[] } | null;
  error: { kind: "auth" | "other"; message: string } | null;
  fetchedAt: number | null;
}

export interface Settings {
  language: string;
  refreshMinutes: number;
  autostart: boolean;
}

export interface AppView {
  providers: ProviderView[];
  lastSync: number | null;
  settings: Settings;
}

export interface DeviceStart {
  userCode: string;
  verificationUrl: string;
  deviceAuthId: string;
  interval: number;
}

export const api = {
  getState: () => invoke<AppView>("get_state"),
  refresh: () => invoke<AppView>("refresh"),
  codexStartDevice: () => invoke<DeviceStart>("codex_start_device"),
  codexWaitDevice: (start: DeviceStart) => invoke<AppView>("codex_wait_device", { start }),
  codexCancelDevice: () => invoke<void>("codex_cancel_device"),
  claudeStartAuth: () => invoke<string>("claude_start_auth"),
  claudeFinishAuth: (code: string) => invoke<AppView>("claude_finish_auth", { code }),
  importCli: (provider: ProviderId) => invoke<AppView>("import_cli", { provider }),
  disconnect: (provider: ProviderId) => invoke<AppView>("disconnect", { provider }),
  setSettings: (settings: Settings) => invoke<AppView>("set_settings", { settings }),
  fitWindow: (height: number) => invoke<void>("fit_window", { height }),
  quit: () => invoke<void>("quit_app"),
};
