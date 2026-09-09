import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type View = "dashboard" | "settings";
export interface Settings {
  closeToTray: boolean;
  showMascot: boolean;
  friendlyMessages: boolean;
  notificationsEnabled: boolean;
  notificationThresholds: [number, number, number];
  hiddenProviderIds: string[];
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
  inputTokens?: number | null;
  outputTokens?: number | null;
  cachedInputTokens?: number | null;
  requestCount: number | null;
  estimatedCostUsd: number | null;
  source: MetricSource;
}
export interface SpendEstimate {
  amount: number;
  currency: string;
  windowDays: number;
}
export interface UsageSnapshot {
  providerId: string;
  displayName: string;
  accountLabel: string | null;
  plan: string | null;
  /** null = unknown or not subscription-based; false = explicitly unsubscribed (hidden) */
  hasSubscription: boolean | null;
  capabilities: ProviderCapabilities;
  authState: string;
  dataKind: DataKind;
  windows: UsageWindow[];
  tokenUsage: TokenUsage | null;
  balance: number | null;
  /** ISO-4217 code for `balance` (e.g. "USD", "CNY"); null when no balance is reported */
  balanceCurrency: string | null;
  /** Ellie's own estimate of spend from balance history, when computable */
  spendEstimate: SpendEstimate | null;
  /** Model in use or dominant alias reported by the provider, when available */
  model: string | null;
  fetchedAt: string;
}
export interface ProviderOverview {
  providerId: string;
  displayName: string;
  snapshot: UsageSnapshot | null;
  error:
    | "invalid_snapshot"
    | "authentication_required"
    | "authentication_expired"
    | "unavailable"
    | null;
  stale?: boolean;
  lastSuccessfulRefresh?: string | null;
  lastAttemptAt?: string | null;
  nextRetryAt?: string | null;
}

export type AnalyticsRange = "today" | "sevenDays" | "thirtyDays" | "ninetyDays";
export type AnalyticsSource =
  | "provider_reported"
  | "locally_calculated"
  | "mixed";
export interface AnalyticsSpend {
  currency: string;
  amount: number;
  source: AnalyticsSource;
}
export interface AnalyticsProvider {
  providerId: string;
  displayName: string;
  model: string | null;
  latestAt: string;
  totalTokens: number | null;
  requestCount: number | null;
  tokenSource: "provider_reported" | "locally_calculated" | null;
}
export interface TokenPoint {
  date: string;
  totalTokens: number;
}
export interface QuotaPoint {
  providerId: string;
  displayName: string;
  windowId: string;
  windowLabel: string;
  usedPercent: number | null;
  remainingPercent: number | null;
  observedAt: string;
}
export interface AnalyticsResponse {
  range: AnalyticsRange;
  startAt: string;
  endAt: string;
  snapshotCount: number;
  providerCount: number;
  latestTotalTokens: number | null;
  latestRequestCount: number | null;
  tokenSource: AnalyticsSource | null;
  estimatedSpend: AnalyticsSpend[];
  providers: AnalyticsProvider[];
  tokenSeries: TokenPoint[];
  quotaWindows: QuotaPoint[];
}

export type ProviderKeySource =
  | "credential_manager"
  | "environment"
  | "none";
export interface RefreshResponse {
  providers: ProviderOverview[];
  refreshed: boolean;
  busy: boolean;
}

export interface ProviderKeyStatus {
  providerId: string;
  source: ProviderKeySource;
}

export const desktop = {
  available: isTauri,
  bootstrap: () => invoke<Bootstrap>("get_bootstrap"),
  getAnalytics: (range: AnalyticsRange) =>
    invoke<AnalyticsResponse>("get_analytics", { range }),
  saveSettings: (settings: Settings) =>
    invoke<Settings>("save_settings", { settings }),
  hide: () => invoke<void>("hide_to_tray"),
  onNavigate: (callback: (view: View) => void) =>
    listen<View>("navigate", (event) => callback(event.payload)),
  onProvidersUpdated: (callback: (providers: ProviderOverview[]) => void) =>
    listen<ProviderOverview[]>("providers-updated", (event) =>
      callback(event.payload),
    ),
  refreshAll: () => invoke<RefreshResponse>("refresh_all"),
  refreshProvider: (providerId: string) =>
    invoke<RefreshResponse>("refresh_provider", { providerId }),
  saveProviderKey: (providerId: string, key: string) =>
    invoke<void>("save_provider_key", { providerId, key }),
  deleteProviderKey: (providerId: string) =>
    invoke<void>("delete_provider_key", { providerId }),
  providerKeyStatus: () =>
    invoke<ProviderKeyStatus[]>("provider_key_status"),
};
