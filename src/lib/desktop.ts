import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type View = "dashboard" | "settings";
export interface Settings {
  closeToTray: boolean;
  showMascot: boolean;
  friendlyMessages: boolean;
}
export interface Bootstrap {
  settings: Settings;
  view: View;
  providers: ProviderOverview[];
}

export type MetricSource = "provider_reported" | "locally_calculated";
export type DataKind = "live" | "mock";
export interface ProviderCapabilities {
  quotaWindows: boolean;
  tokenUsage: boolean;
  accountBalance: boolean;
  credits: boolean;
  costTracking: boolean;
  localHistory: boolean;
}
export interface UsageWindow {
  id: string;
  label: string;
  usedPercent: number | null;
  remainingPercent: number | null;
  resetAt: string | null;
  source: MetricSource;
}
export interface TokenUsage {
  totalTokens: number | null;
  requestCount: number | null;
  estimatedCostUsd: number | null;
  source: MetricSource;
}
export interface UsageSnapshot {
  providerId: string;
  displayName: string;
  accountLabel: string | null;
  plan: string | null;
  capabilities: ProviderCapabilities;
  authState: string;
  dataKind: DataKind;
  windows: UsageWindow[];
  tokenUsage: TokenUsage | null;
  fetchedAt: string;
}
export interface ProviderOverview {
  snapshot: UsageSnapshot | null;
  error:
    | "invalid_snapshot"
    | "authentication_required"
    | "authentication_expired"
    | "unavailable"
    | null;
}

export const desktop = {
  available: isTauri,
  bootstrap: () => invoke<Bootstrap>("get_bootstrap"),
  saveSettings: (settings: Settings) =>
    invoke<Settings>("save_settings", { settings }),
  hide: () => invoke<void>("hide_to_tray"),
  onNavigate: (callback: (view: View) => void) =>
    listen<View>("navigate", (event) => callback(event.payload)),
};
