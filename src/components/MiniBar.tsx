import { useEffect, useState, type CSSProperties, type PointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  desktop,
  type ProviderOverview,
  type Settings,
} from "../lib/desktop";
import { selectMiniQuotaMetrics } from "../lib/miniQuota";

export default function MiniBar() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [providers, setProviders] = useState<ProviderOverview[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const native = desktop.available();

  useEffect(() => {
    let active = true;
    let stopProviders: (() => void) | undefined;
    let stopSettings: (() => void) | undefined;
    if (!native) {
      setLoading(false);
      setError("Quota unavailable");
      return;
    }
    void (async () => {
      let providersAdvanced = false;
      let settingsAdvanced = false;
      try {
        const unlistenProviders = await desktop.onProvidersUpdated((next) => {
          providersAdvanced = true;
          if (active) setProviders(next);
        });
        if (!active) {
          unlistenProviders();
          return;
        }
        stopProviders = unlistenProviders;
        const unlistenSettings = await desktop.onMiniSettingsUpdated((next) => {
          settingsAdvanced = true;
          if (active) setSettings(next);
        });
        if (!active) {
          unlistenSettings();
          return;
        }
        stopSettings = unlistenSettings;
        const bootstrap = await desktop.miniBootstrap();
        if (active) {
          if (!settingsAdvanced) setSettings(bootstrap.settings);
          if (!providersAdvanced) setProviders(bootstrap.providers);
        }
      } catch {
        if (active) setError("Quota unavailable");
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
      stopProviders?.();
      stopSettings?.();
    };
  }, [native]);

  const metrics = selectMiniQuotaMetrics(
    providers,
    settings?.hiddenProviderIds ?? [],
  );
  const emptyState = miniEmptyState(providers, settings?.hiddenProviderIds ?? []);
  const style = {
    "--mini-opacity": settings?.miniBarOpacity ?? 0.9,
  } as CSSProperties;
  const accessibleStatus = miniAccessibleStatus(
    loading,
    error,
    metrics,
    emptyState,
  );

  async function openMain() {
    setError("");
    try {
      await desktop.openMainWindow();
    } catch {
      setError("Ellie could not open the dashboard");
    }
  }

  function startDragging(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    event.preventDefault();
    void getCurrentWindow().startDragging().catch(() => {
      setError("Mini bar could not be moved");
    });
  }

  return (
    <main className="mini-bar" style={style} aria-label="Ellie quota mini bar">
      <span
        className="visually-hidden"
        role={error ? "alert" : "status"}
        aria-live={error ? "assertive" : "polite"}
      >
        Mini bar status: {accessibleStatus}
      </span>
      <div
        className="mini-drag-handle"
        data-tauri-drag-region
        title="Drag mini bar"
        onPointerDown={startDragging}
      >
        <span aria-hidden="true">⠿</span>
      </div>
      <button
        type="button"
        className="mini-open"
        onClick={() => void openMain()}
        aria-label={`${accessibleStatus}. Open Ellie dashboard`}
      >
        <span className="mini-wordmark" aria-hidden="true">ellie<span>.</span></span>
        <span className="mini-content">
          {loading ? (
            <span className="mini-state">Loading quota…</span>
          ) : error ? (
            <span className="mini-state mini-error">{error}</span>
          ) : metrics.length === 0 ? (
            <span className="mini-state">{emptyState}</span>
          ) : (
            metrics.map((metric) => (
              <span
                className="mini-metric"
                key={`${metric.providerId}:${metric.windowId}`}
              >
                <span className="mini-provider">{metric.providerName}</span>
                <span>{metric.windowLabel}</span>
                <strong>{formatPercent(metric.remainingPercent)} remaining</strong>
                {metric.stale && (
                  <small>
                    Showing data from {formatAge(metric.lastSuccessfulRefresh)} · refresh failed
                  </small>
                )}
              </span>
            ))
          )}
        </span>
      </button>
    </main>
  );
}

function miniEmptyState(
  providers: ProviderOverview[],
  hiddenProviderIds: string[],
) {
  const visible = providers.filter(
    (provider) => !hiddenProviderIds.includes(provider.providerId),
  );
  if (providers.length === 0) return "Waiting for quota data";
  if (visible.length === 0) return "No providers selected";
  if (visible.some((provider) => provider.error !== null)) {
    return "Quota unavailable";
  }
  if (visible.every((provider) => provider.snapshot === null)) {
    return "Waiting for quota data";
  }
  const eligible = visible.filter((provider) => {
    const snapshot = provider.snapshot;
    return snapshot?.dataKind === "live"
      && snapshot.hasSubscription !== false
      && snapshot.capabilities.quotaWindows;
  });
  if (eligible.some((provider) => provider.snapshot?.windows.some(
    (window) => window.source === "provider_reported" && window.remainingPercent === null,
  ))) {
    return "Quota remaining not reported";
  }
  return "No provider-reported quota windows";
}

function miniAccessibleStatus(
  loading: boolean,
  error: string,
  metrics: ReturnType<typeof selectMiniQuotaMetrics>,
  emptyState: string,
) {
  if (loading) return "Loading quota";
  if (error) return error;
  if (metrics.length === 0) return emptyState;
  return metrics.map((metric) => {
    const stale = metric.stale
      ? `, showing data from ${formatAge(metric.lastSuccessfulRefresh)}, refresh failed`
      : "";
    return `${metric.providerName}, ${metric.windowLabel}, ${formatPercent(metric.remainingPercent)} remaining${stale}`;
  }).join(". ");
}

function formatPercent(value: number) {
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(value)}%`;
}

function formatAge(value: string | null) {
  if (!value) return "an earlier refresh";
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return "an earlier refresh";
  const seconds = Math.max(0, Math.floor((Date.now() - timestamp) / 1_000));
  if (seconds < 60) return `${seconds} seconds ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} minute${minutes === 1 ? "" : "s"} ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours} hour${hours === 1 ? "" : "s"} ago`;
}
