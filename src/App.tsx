import { useEffect, useState } from "react";
import { Cat } from "./components/Cat";
import { copy } from "./copy";
import {
  desktop,
  type AnalyticsRange,
  type AnalyticsResponse,
  type AnalyticsSource,
  type ProviderKeySource,
  type ProviderOverview,
  type Settings,
  type UsageSnapshot,
  type View,
} from "./lib/desktop";

export default function App() {
  const [view, setView] = useState<View>("dashboard");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [saving, setSaving] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshingProvider, setRefreshingProvider] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [providers, setProviders] = useState<ProviderOverview[]>([]);
  const [analyticsRange, setAnalyticsRange] = useState<AnalyticsRange>("sevenDays");
  const [analytics, setAnalytics] = useState<AnalyticsResponse | null>(null);
  const [analyticsLoading, setAnalyticsLoading] = useState(false);
  const [analyticsError, setAnalyticsError] = useState("");
  const [analyticsReload, setAnalyticsReload] = useState(0);
  const visibleProviders = providers.filter(
    (provider) =>
      !(settings?.hiddenProviderIds ?? []).includes(provider.providerId) &&
      (provider.snapshot
        ? provider.snapshot.hasSubscription !== false
        : provider.error !== "authentication_required"),
  );
  const native = desktop.available();

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    let unlistenProviders: (() => void) | undefined;
    if (!native) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError("");
    void (async () => {
      try {
        const stop = await desktop.onNavigate((next) => {
          if (active) setView(next);
        });
        if (!active) {
          stop();
          return;
        }
        unlisten = stop;
        const stopProviders = await desktop.onProvidersUpdated((next) => {
          if (active) {
            setProviders(next);
            setAnalyticsReload((value) => value + 1);
          }
        });
        if (!active) {
          stopProviders();
          return;
        }
        unlistenProviders = stopProviders;
        const result = await desktop.bootstrap();
        if (active) {
          setSettings(result.settings);
          setDraft(result.settings);
          setView(result.view);
          setProviders(result.providers ?? []);
        }
      } catch {
        if (active)
          setError(
            "Ellie could not load local settings. Retry, or restart the app if the problem continues.",
          );
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
      unlisten?.();
      unlistenProviders?.();
    };
  }, [native, retry]);

  useEffect(() => {
    if (!native || view !== "history") {
      setAnalyticsLoading(false);
      return;
    }
    let active = true;
    setAnalyticsLoading(true);
    setAnalyticsError("");
    void desktop
      .getAnalytics(analyticsRange)
      .then((result) => {
        if (active) setAnalytics(result);
      })
      .catch(() => {
        if (active) setAnalyticsError("Ellie could not load local history.");
      })
      .finally(() => {
        if (active) setAnalyticsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [analyticsRange, analyticsReload, native, retry, view]);

  async function refreshAll() {
    if (!native || refreshing || refreshingProvider) return;
    setRefreshing(true);
    setError("");
    setNotice("");
    try {
      const result = await desktop.refreshAll();
      if (result.busy) {
        setNotice("Ellie is already refreshing. The current data is unchanged.");
      } else {
        setProviders(result.providers);
        setNotice("Provider data refreshed.");
      }
    } catch {
      setError("Ellie could not refresh provider data. Existing data is unchanged.");
    } finally {
      setRefreshing(false);
    }
  }

  async function refreshProvider(providerId: string) {
    if (!native || refreshing || refreshingProvider) return;
    setRefreshingProvider(providerId);
    setError("");
    setNotice("");
    try {
      const result = await desktop.refreshProvider(providerId);
      if (result.busy) {
        setNotice("Ellie is already refreshing. The current data is unchanged.");
      } else {
        setProviders(result.providers);
        setNotice("Provider data refreshed.");
      }
    } catch {
      setError("Ellie could not refresh this provider. Existing data is unchanged.");
    } finally {
      setRefreshingProvider(null);
    }
  }

  async function save() {
    if (!draft || saving) return;
    setSaving(true);
    setError("");
    setNotice("");
    try {
      const saved = await desktop.saveSettings(draft);
      setSettings(saved);
      setDraft(saved);
      setNotice("Settings saved on this device.");
    } catch {
      setError(
        "Your settings were not saved. Try again. Your previous settings are still active.",
      );
    } finally {
      setSaving(false);
    }
  }

  async function setProviderVisibility(providerId: string, visible: boolean) {
    if (!settings || saving || !native) return;
    setSaving(true);
    setError("");
    setNotice("");
    const hiddenProviderIds = visible
      ? (settings.hiddenProviderIds ?? []).filter((id) => id !== providerId)
      : [...new Set([...(settings.hiddenProviderIds ?? []), providerId])];
    try {
      const saved = await desktop.saveSettings({ ...settings, hiddenProviderIds });
      setSettings(saved);
      // Do not discard unsaved appearance edits when changing visibility.
      setDraft((current) => current
        ? { ...current, hiddenProviderIds: saved.hiddenProviderIds }
        : saved);
      setNotice(visible
        ? "Provider display enabled. Unconfigured or unsubscribed providers stay hidden."
        : "Provider hidden. Show it again in Settings → Provider visibility.");
    } catch {
      setError("Provider visibility was not saved. Your previous display settings are still active. Try again.");
    } finally {
      setSaving(false);
    }
  }

  async function hide() {
    setError("");
    try {
      await desktop.hide();
    } catch {
      setError("Ellie could not hide the window. Try the minimize button.");
    }
  }

  function navigate(next: View) {
    setView(next);
    setNotice("");
  }
  const showMascot = settings?.showMascot ?? true;
  const friendly = settings?.friendlyMessages ?? true;
  const dirty =
    settings !== null &&
    draft !== null &&
    JSON.stringify(settings) !== JSON.stringify(draft);

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand">
          <Cat small />
          <div>
            <span className="wordmark">
              ellie<span className="pink">.</span>
            </span>
            <span className="brand-caption">Your AI usage companion</span>
          </div>
        </div>
        <span className="version">v0.1 · Preview</span>
      </header>
      <nav aria-label="Main navigation">
        <button
          className={view === "dashboard" ? "nav-active" : ""}
          aria-current={view === "dashboard" ? "page" : undefined}
          onClick={() => navigate("dashboard")}
        >
          Overview
        </button>
        <button
          className={view === "history" ? "nav-active" : ""}
          aria-current={view === "history" ? "page" : undefined}
          onClick={() => navigate("history")}
        >
          History
        </button>
        <button
          className={view === "settings" ? "nav-active" : ""}
          aria-current={view === "settings" ? "page" : undefined}
          onClick={() => navigate("settings")}
        >
          Settings
        </button>
        <span className="local-label">
          <span className="local-dot" /> Local to this device
        </span>
      </nav>
      <main id="main-content">
        {!native && (
          <div className="banner">
            Browser preview · Desktop settings and tray controls are available
            in the Windows app.
          </div>
        )}
        {error && (
          <div className="error" role="alert">
            {error}
            {!settings && native && (
              <button
                onClick={() => setRetry((value) => value + 1)}
                disabled={loading}
              >
                Retry
              </button>
            )}
          </div>
        )}
        {notice && view === "dashboard" && <p role="status">{notice}</p>}
        {loading && <p role="status">Opening your local settings…</p>}
        {view === "dashboard" ? (
          <>
            <section className="welcome" aria-labelledby="welcome-title">
              <div>
                <p className="eyebrow">A quiet place to check in</p>
                <h1 id="welcome-title">Your AI, at a glance.</h1>
                <p className="welcome-copy">
                  {friendly
                    ? copy.greeting
                    : "One place to see how much AI you have left."}
                </p>
              </div>
              {showMascot && <Cat />}
            </section>
            <section className="empty-state" aria-labelledby="usage-title">
              <span className="empty-mark" aria-hidden="true">
                —
              </span>
              <div>
                <h2 id="usage-title">
                  {visibleProviders.length === 0
                    ? "No usage data yet"
                    : "Current usage"}
                </h2>
                <p>
                  {usageDescription(providers, visibleProviders)}
                  <br />
                  Allowances and reset times appear when data is available.
                </p>
              </div>
            </section>
            <section aria-labelledby="providers-heading">
              <div className="section-heading">
                <h2 id="providers-heading">Providers</h2>
                <span>{providersCount(providers, visibleProviders)}</span>
                <button
                  type="button"
                  className="refresh-button"
                  onClick={() => void refreshAll()}
                  disabled={!native || refreshing || refreshingProvider !== null}
                >
                  {refreshing ? "Refreshing…" : "Refresh now"}
                </button>
              </div>
              <div className="providers">
                {providers.length === 0 ? (
                  <ProviderUnavailable />
                ) : (
                  visibleProviders.map((provider) => (
                    <ProviderCard
                      key={provider.providerId}
                      provider={provider}
                      hideDisabled={saving || !settings || !native || !provider.providerId}
                      onHide={() => void setProviderVisibility(provider.providerId, false)}
                      refreshDisabled={refreshing || refreshingProvider !== null || !native}
                      refreshing={refreshingProvider === provider.providerId}
                      onRefresh={() => void refreshProvider(provider.providerId)}
                    />
                  ))
                )}
              </div>
            </section>
            <div className="bottom-note">
              <span>{usageNote(providers, visibleProviders)}</span>
              <button disabled={!native} onClick={() => void hide()}>
                Hide to tray <span aria-hidden="true">↘</span>
              </button>
            </div>
          </>
        ) : view === "history" ? (
          <AnalyticsPanel
            native={native}
            range={analyticsRange}
            analytics={analytics}
            loading={analyticsLoading}
            error={analyticsError}
            onRangeChange={setAnalyticsRange}
          />
        ) : (
          <section className="settings" aria-labelledby="settings-title">
            <p className="eyebrow">Make yourself at home</p>
            <h1 id="settings-title">Settings</h1>
            <p className="welcome-copy">
              Small preferences, saved on this device.
            </p>
            {draft ? (
              <form
                onSubmit={(event) => {
                  event.preventDefault();
                  void save();
                }}
              >
                <fieldset disabled={saving || loading}>
                  <legend>Window & appearance</legend>
                  <Setting
                    label="Close to tray"
                    detail="Keep Ellie running when you close the window. Quit from the tray menu."
                    checked={draft.closeToTray}
                    onChange={(value) =>
                      setDraft({ ...draft, closeToTray: value })
                    }
                  />
                  <Setting
                    label="Show dashboard mascot"
                    detail="A little black-and-white company. Always still, never distracting."
                    checked={draft.showMascot}
                    onChange={(value) =>
                      setDraft({ ...draft, showMascot: value })
                    }
                  />
                  <Setting
                    label="Friendly messages"
                    detail="A few warm words alongside the facts."
                    checked={draft.friendlyMessages}
                    onChange={(value) =>
                      setDraft({ ...draft, friendlyMessages: value })
                    }
                  />
                  <Setting
                    label="Usage notifications"
                    detail="Notify when a provider-reported quota window reaches one of the thresholds below."
                    checked={draft.notificationsEnabled}
                    onChange={(value) =>
                      setDraft({ ...draft, notificationsEnabled: value })
                    }
                  />
                  <NotificationThresholds
                    thresholds={draft.notificationThresholds}
                    onChange={(notificationThresholds) =>
                      setDraft({ ...draft, notificationThresholds })
                    }
                  />
                </fieldset>
                <div className="save-row">
                  <button
                    className="primary"
                    disabled={!dirty || saving || loading}
                    type="submit"
                  >
                    {saving ? "Saving…" : "Save settings"}
                  </button>
                  <span role="status">
                    {notice || (dirty ? "Unsaved changes" : "")}
                  </span>
                </div>
              </form>
            ) : (
              !loading && (
                <p>Open the desktop app to manage your local preferences.</p>
              )
            )}
            {settings && (
              <fieldset className="provider-visibility" disabled={saving || !native}>
                <legend>Provider visibility</legend>
                <p>
                  Changes save immediately. Hidden cards keep their credentials,
                  history, and data fetching. Unconfigured or unsubscribed providers
                  remain hidden until active.
                </p>
                {providers.map((provider) => (
                  <Setting
                    key={provider.providerId}
                    label={`Show ${provider.displayName} on dashboard`}
                    detail={provider.snapshot?.dataKind === "mock"
                      ? "Demo provider · illustrative data, not a connected account."
                      : "Display preference only; this does not disconnect your account."}
                    checked={!(settings.hiddenProviderIds ?? []).includes(provider.providerId)}
                    onChange={(visible) => void setProviderVisibility(provider.providerId, visible)}
                  />
                ))}
              </fieldset>
            )}
            <ProviderCredentials />
            <div className="settings-note">
              <h2>Dark mode, by default.</h2>
              <p>
                More preferences will appear as new features become available.
              </p>
            </div>
          </section>
        )}
      </main>
      <footer>
        <span>
          {friendly ? copy.quiet : "Ellie · Local-first AI usage monitor"}
        </span>
        <span>Trust the number.</span>
      </footer>
    </div>
  );
}

function ProviderCredentials() {
  const [status, setStatus] = useState<Map<string, ProviderKeySource>>(
    new Map(),
  );
  const [openAiAdmin, setOpenAiAdmin] = useState("");
  const [anthropic, setAnthropic] = useState("");
  const [deepseek, setDeepseek] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");

  const refresh = () => {
    void desktop.providerKeyStatus().then((rows) => {
      setStatus(new Map(rows.map((row) => [row.providerId, row.source])));
    });
  };
  useEffect(refresh, []);

  const saveKey = async (providerId: string, key: string, clear: () => void) => {
    if (!key.trim()) return;
    setBusy(true);
    try {
      await desktop.saveProviderKey(providerId, key);
      clear();
      setMessage("Key saved to Windows Credential Manager.");
      refresh();
    } catch {
      setMessage("The key could not be saved.");
    } finally {
      setBusy(false);
    }
  };
  const removeKey = async (providerId: string) => {
    setBusy(true);
    try {
      await desktop.deleteProviderKey(providerId);
      setMessage("Key removed.");
      refresh();
    } catch {
      setMessage("The key could not be removed.");
    } finally {
      setBusy(false);
    }
  };
  const sourceLabel = (source: ProviderKeySource | undefined) =>
    source === "environment"
      ? "From environment"
      : source === "credential_manager"
        ? "Saved on this device"
        : "Not set";

  return (
    <fieldset disabled={busy} className="credentials">
      <legend>Provider credentials</legend>
      <p className="credential-intro">
        Keys are stored in Windows Credential Manager and never shown again.
      </p>
      <div className="credential-row">
        <div className="credential-info">
          <strong>OpenAI API</strong>
          <span>
            Admin API key for separately billed API token usage · {" "}
            {sourceLabel(status.get("openai-api"))}
          </span>
        </div>
        <input
          type="password"
          value={openAiAdmin}
          placeholder="sk-admin-…"
          onChange={(event) => setOpenAiAdmin(event.target.value)}
          aria-label="OpenAI Admin API key"
        />
        <button
          aria-label="Save OpenAI Admin API key"
          onClick={() =>
            void saveKey("openai-api", openAiAdmin, () => setOpenAiAdmin(""))
          }
        >
          Save
        </button>
        {status.get("openai-api") === "credential_manager" && (
          <button onClick={() => void removeKey("openai-api")}>Remove</button>
        )}
      </div>
      <div className="credential-row">
        <div className="credential-info">
          <strong>Anthropic / Claude</strong>
          <span>
            Admin key (sk-ant-admin) for usage and cost reports ·{" "}
            {sourceLabel(status.get("anthropic-claude"))}
          </span>
        </div>
        <input
          type="password"
          value={anthropic}
          placeholder="sk-ant-admin-…"
          onChange={(event) => setAnthropic(event.target.value)}
          aria-label="Anthropic API key"
        />
        <button
          aria-label="Save Anthropic API key"
          onClick={() =>
            void saveKey("anthropic-claude", anthropic, () => setAnthropic(""))
          }
        >
          Save
        </button>
        {status.get("anthropic-claude") === "credential_manager" && (
          <button onClick={() => void removeKey("anthropic-claude")}>
            Remove
          </button>
        )}
      </div>
      <div className="credential-row">
        <div className="credential-info">
          <strong>DeepSeek</strong>
          <span>
            API key from platform.deepseek.com · {sourceLabel(status.get("deepseek"))}
          </span>
        </div>
        <input
          type="password"
          value={deepseek}
          placeholder="sk-…"
          onChange={(event) => setDeepseek(event.target.value)}
          aria-label="DeepSeek API key"
        />
        <button
          aria-label="Save DeepSeek API key"
          onClick={() => void saveKey("deepseek", deepseek, () => setDeepseek(""))}
        >
          Save
        </button>
        {status.get("deepseek") === "credential_manager" && (
          <button onClick={() => void removeKey("deepseek")}>Remove</button>
        )}
      </div>
      <div className="credential-row">
        <div className="credential-info">
          <strong>OpenAI / Codex</strong>
          <span>Uses your `codex login` session; Ellie reuses it directly. API billing is configured separately above.</span>
        </div>
      </div>
      <p className="credential-message" role="status">
        {message}
      </p>
    </fieldset>
  );
}

function NotificationThresholds({
  thresholds,
  onChange,
}: {
  thresholds: [number, number, number];
  onChange: (thresholds: [number, number, number]) => void;
}) {
  const labels = ["First warning", "Second warning", "Final warning"];
  return (
    <fieldset className="notification-thresholds">
      <legend>Warning thresholds</legend>
      <p>Adjust the percentage used at which each notification is sent.</p>
      {thresholds.map((value, index) => {
        const minimum = index === 0 ? 1 : thresholds[index - 1]! + 1;
        const maximum = index === thresholds.length - 1 ? 100 : thresholds[index + 1]! - 1;
        return (
          <label className="threshold-row" key={labels[index]!}>
            <span>
              <strong>{labels[index]!}</strong>
              <span className="setting-detail">{value}% used</span>
            </span>
            <input
              type="range"
              min={minimum}
              max={maximum}
              value={value}
              aria-label={`${labels[index]!} threshold`}
              onChange={(event) => {
                const next = [...thresholds] as [number, number, number];
                next[index] = Number(event.target.value);
                onChange(next);
              }}
            />
            <output>{value}%</output>
          </label>
        );
      })}
    </fieldset>
  );
}

function Setting({
  label,
  detail,
  checked,
  onChange,
}: {
  label: string;
  detail: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className="setting">
      <span>
        <strong>{label}</strong>
        <span className="setting-detail">{detail}</span>
      </span>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  );
}

function hasLiveData(providers: ProviderOverview[]) {
  return providers.some(
    (provider) => provider.snapshot?.dataKind === "live",
  );
}

function providersCount(
  providers: ProviderOverview[],
  visible: ProviderOverview[],
) {
  if (providers.length === 0) return "0 connected";
  const hidden = providers.length - visible.length;
  return hidden > 0
    ? `${visible.length} shown · ${hidden} hidden`
    : `${visible.length} shown`;
}

function usageDescription(
  providers: ProviderOverview[],
  visible: ProviderOverview[],
) {
  if (providers.length === 0) {
    return "Provider connections are not available in this build.";
  }
  if (visible.length === 0) {
    return "No visible providers. Check Provider visibility in Settings; unconfigured or unsubscribed providers also stay hidden.";
  }
  return hasLiveData(visible)
    ? "Cards show live quota from your configured logins; demo cards stay labeled."
    : "Demo providers exercise Ellie's display and are not account data.";
}

function usageNote(providers: ProviderOverview[], visible: ProviderOverview[]) {
  if (providers.length === 0) {
    return "No provider requests.";
  }
  if (visible.length === 0) {
    return "Show hidden cards in Settings → Provider visibility. No credentials or history are deleted.";
  }
  return hasLiveData(visible)
    ? "Live data comes from configured provider connections on this device. Keys remain local."
    : "No provider requests. Demo data is illustrative only.";
}

function ProviderUnavailable() {
  return (
    <article className="provider">
      <span className="provider-symbol" aria-hidden="true">
        —
      </span>
      <div className="provider-name">
        <h3>No provider data</h3>
        <p>Connections will be added in a later milestone.</p>
      </div>
      <span className="unavailable">Not available yet</span>
    </article>
  );
}

const ANALYTICS_RANGES: Array<{ value: AnalyticsRange; label: string }> = [
  { value: "today", label: "Today" },
  { value: "sevenDays", label: "7 days" },
  { value: "thirtyDays", label: "30 days" },
  { value: "ninetyDays", label: "90 days" },
];

function AnalyticsPanel({
  native,
  range,
  analytics,
  loading,
  error,
  onRangeChange,
}: {
  native: boolean;
  range: AnalyticsRange;
  analytics: AnalyticsResponse | null;
  loading: boolean;
  error: string;
  onRangeChange: (range: AnalyticsRange) => void;
}) {
  return (
    <section className="analytics" aria-labelledby="analytics-heading">
      <div className="section-heading analytics-heading">
        <div>
          <h2 id="analytics-heading">History & insights</h2>
          <span>Local history, kept on this device</span>
        </div>
        <div className="analytics-ranges" role="group" aria-label="Analytics range">
          {ANALYTICS_RANGES.map((option) => (
            <button
              key={option.value}
              type="button"
              className={range === option.value ? "range-active" : ""}
              aria-pressed={range === option.value}
              disabled={!native || loading}
              onClick={() => onRangeChange(option.value)}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>
      {!native ? (
        <p className="analytics-empty">Open the desktop app to view local history.</p>
      ) : loading && !analytics ? (
        <p className="analytics-empty" role="status">Loading local history…</p>
      ) : error ? (
        <p className="analytics-empty" role="alert">{error}</p>
      ) : !analytics || analytics.snapshotCount === 0 ? (
        <p className="analytics-empty">
          No live history in this range yet. Keep Ellie running to build a local picture.
        </p>
      ) : (
        <>
          <div className="analytics-summary">
            <AnalyticsMetric
              label="Latest tracked tokens"
              value={formatCount(analytics.latestTotalTokens)}
              detail={`Token metrics · ${analyticsSourceLabel(analytics.tokenSource)}`}
            />
            <AnalyticsMetric
              label="Latest requests"
              value={formatCount(analytics.latestRequestCount)}
              detail={`Request metrics · ${analyticsSourceLabel(analytics.tokenSource)}`}
            />
            <AnalyticsMetric
              label="Estimated spend"
              value={
                analytics.estimatedSpend.length === 0
                  ? "—"
                  : analytics.estimatedSpend
                      .map((spend) => formatBalance(spend.amount, spend.currency))
                      .join(" · ")
              }
              detail="Ellie estimate, not provider billing"
            />
            <AnalyticsMetric
              label="Observed history"
              value={formatCount(analytics.snapshotCount)}
              detail={`${formatCount(analytics.providerCount)} live provider${analytics.providerCount === 1 ? "" : "s"}`}
            />
          </div>
          <div className="analytics-grid">
            <div className="analytics-card">
              <h3>Token activity over time</h3>
              <p className="analytics-card-note">
                Daily latest observations · {analyticsSourceLabel(analytics.tokenSource).toLowerCase()}.
              </p>
              {analytics.tokenSeries.length === 0 ? (
                <p className="analytics-muted">No token activity reported.</p>
              ) : (
                <div className="token-chart" aria-label="Token activity over time">
                  {analytics.tokenSeries.map((point) => {
                    const maximum = Math.max(
                      1,
                      ...analytics.tokenSeries.map((item) => item.totalTokens),
                    );
                    return (
                      <div className="token-chart-row" key={point.date}>
                        <span>{formatAnalyticsDate(point.date)}</span>
                        <div className="token-chart-track">
                          <span style={{ width: `${(point.totalTokens / maximum) * 100}%` }} />
                        </div>
                        <strong>{formatCount(point.totalTokens)}</strong>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
            <div className="analytics-card">
              <h3>Quota utilization</h3>
              <p className="analytics-card-note">Latest provider-reported observation in range.</p>
              {analytics.quotaWindows.length === 0 ? (
                <p className="analytics-muted">No quota windows reported.</p>
              ) : (
                <div className="quota-analytics-list">
                  {analytics.quotaWindows.map((window) => {
                    const used = window.usedPercent == null
                      ? null
                      : Math.max(0, Math.min(100, window.usedPercent));
                    return (
                      <div className="quota-analytics-row" key={`${window.providerId}-${window.windowId}`}>
                        <div>
                          <strong>{window.displayName}</strong>
                          <span>{window.windowLabel}</span>
                        </div>
                        <strong>{used == null ? "—" : `${Math.round(used)}% used`}</strong>
                        <div
                          className="quota-analytics-track"
                          role="progressbar"
                          aria-label={`${window.displayName} ${window.windowLabel}`}
                          aria-valuemin={0}
                          aria-valuemax={100}
                          aria-valuenow={used ?? undefined}
                        >
                          <span style={{ width: `${used ?? 0}%` }} />
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
          <p className="analytics-footnote">
            Analytics are calculated from fresh live snapshots. Repeated refreshes are not added together;
            estimates and provider-reported windows retain their original meaning.
          </p>
        </>
      )}
    </section>
  );
}

function AnalyticsMetric({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail: string;
}) {
  return (
    <div className="analytics-metric">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </div>
  );
}

function analyticsSourceLabel(source: AnalyticsSource | null) {
  switch (source) {
    case "provider_reported":
      return "provider-reported";
    case "locally_calculated":
      return "Ellie-calculated estimate";
    case "mixed":
      return "mixed sources";
    default:
      return "source unavailable";
  }
}

function formatAnalyticsDate(value: string) {
  const date = new Date(`${value}T00:00:00Z`);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString([], { month: "short", day: "numeric" });
}

function ProviderCard({
  provider,
  onHide,
  hideDisabled,
  onRefresh,
  refreshDisabled,
  refreshing,
}: {
  provider: ProviderOverview;
  onHide: () => void;
  hideDisabled: boolean;
  onRefresh: () => void;
  refreshDisabled: boolean;
  refreshing: boolean;
}) {
  const hideButton = (
    <button type="button" className="provider-hide" disabled={hideDisabled}
      aria-label={`Hide ${provider.displayName}`} onClick={onHide}>
      Hide
    </button>
  );
  const refreshButton = (
    <button type="button" className="provider-refresh" disabled={refreshDisabled}
      aria-label={`Refresh ${provider.displayName}`} onClick={onRefresh}>
      {refreshing ? "Refreshing…" : "Refresh"}
    </button>
  );
  if (!provider.snapshot) {
    return (
      <article className="provider">
        <div className="provider-name">
          <h3>{provider.displayName}</h3>
          <p>Provider unavailable</p>
          <p>Ellie kept other provider results available.</p>
        </div>
        <span className="unavailable">
          {provider.error?.replaceAll("_", " ")}
        </span>
        <div className="provider-actions">
          {refreshButton}
          {hideButton}
        </div>
      </article>
    );
  }
  const { snapshot } = provider;
  return (
    <article className="provider provider-card">
      <div className="provider-card-header">
        <span className="provider-symbol" aria-hidden="true">
          E
        </span>
        <div className="provider-name">
          <h3>{snapshot.displayName}</h3>
          {headerDetail(snapshot) && <p>{headerDetail(snapshot)}</p>}
          {snapshot.model && <p className="provider-model">Model: {snapshot.model}</p>}
          <RefreshStatus provider={provider} />
        </div>
        {snapshot.dataKind === "mock" && (
          <span className="mock-badge">Mock data</span>
        )}
      </div>
      {snapshot.capabilities.quotaWindows &&
        snapshot.windows.map((window) => (
          <UsageWindowCard
            key={window.id}
            window={window}
            dataKind={snapshot.dataKind}
          />
        ))}
      {snapshot.balance !== null && (
        <div className="balance-summary">
          <span>Account balance</span>
          <span>{formatBalance(snapshot.balance, snapshot.balanceCurrency)}</span>
          {snapshot.spendEstimate && (
            <>
              <span>≈ spent (last {snapshot.spendEstimate.windowDays} days)</span>
              <span>
                {formatBalance(
                  snapshot.spendEstimate.amount,
                  snapshot.spendEstimate.currency,
                )}
              </span>
              <small>
                Estimated by Ellie from reported balance changes; top-ups can
                skew it
              </small>
            </>
          )}
        </div>
      )}
      {snapshot.capabilities.tokenUsage && snapshot.tokenUsage && (
        <TokenSummaryCard snapshot={snapshot} />
      )}
      <div className="provider-actions">
        {refreshButton}
        {hideButton}
      </div>
    </article>
  );
}

function RefreshStatus({ provider }: { provider: ProviderOverview }) {
  if (provider.stale && provider.lastSuccessfulRefresh) {
    return (
      <span className="provider-stale" role="status">
        Showing data from {formatAge(provider.lastSuccessfulRefresh)}
        {provider.error ? " · refresh failed" : ""}
      </span>
    );
  }
  if (provider.error && !provider.snapshot) {
    return <span className="provider-stale">Refresh unavailable</span>;
  }
  if (provider.snapshot) {
    return <span className="provider-fresh">Updated {formatAge(provider.snapshot.fetchedAt)} ago</span>;
  }
  return null;
}

function TokenSummaryCard({ snapshot }: { snapshot: UsageSnapshot }) {
  const tokens = snapshot.tokenUsage!;
  const mock = snapshot.dataKind === "mock";
  const windowLabel = mock
    ? "Sample token activity"
    : "Token activity (last 30 days)";
  const breaksDown =
    tokens.inputTokens !== undefined && tokens.outputTokens !== undefined;
  return (
    <div className="token-summary">
      <span>{windowLabel}</span>
      <span>
        {formatCount(tokens.totalTokens)} tokens ·{" "}
        {breaksDown
          ? `${formatCount(tokens.inputTokens)} in / ${formatCount(tokens.outputTokens)} out`
          : tokens.requestCount != null
            ? `${formatCount(tokens.requestCount)} requests`
            : "daily activity"}
      </span>
      <small>
        {tokens.estimatedCostUsd !== null &&
          `${formatBalance(tokens.estimatedCostUsd, "USD")} · `}
        {tokens.cachedInputTokens !== undefined &&
          `${formatCount(tokens.cachedInputTokens)} cached input · `}
        {mock
          ? "Locally calculated sample · illustrative only"
          : tokens.source === "locally_calculated"
            ? "Locally calculated by Ellie (window chosen and summed from provider buckets)"
            : "Provider-reported"}
      </small>
    </div>
  );
}

function UsageWindowCard({
  window,
  dataKind,
}: {
  window: UsageSnapshot["windows"][number];
  dataKind: UsageSnapshot["dataKind"];
}) {
  const used = window.usedPercent;
  const remaining = window.remainingPercent;
  return (
    <div className="usage-window">
      <div className="usage-window-title">
        <strong>{window.label}</strong>
        <span>
          {dataKind === "mock"
            ? "Demo data"
            : window.source === "provider_reported"
              ? "Provider-reported"
              : "Estimated"}
        </span>
      </div>
      {used !== null && (
        <>
          <div
            className="usage-track"
            role="progressbar"
            aria-label={`${window.label}: ${used}% used, ${dataKind === "mock" ? "illustrative data" : "provider data"}`}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={used}
          >
            <span style={{ width: `${used}%` }} />
          </div>
          <div className="usage-summary">
            <span>{used}% used</span>
            <span>
              {remaining ?? "—"}% remaining · {formatReset(window.resetAt, dataKind)}
            </span>
          </div>
        </>
      )}
      <small>
        {dataKind === "mock"
          ? "Illustrative provider-reported sample · no connected account"
          : "Provider-reported quota · remaining and reset are derived by Ellie"}
      </small>
    </div>
  );
}

function formatCount(value: number | null | undefined) {
  return value == null ? "—" : new Intl.NumberFormat().format(value);
}
function formatAge(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "an unknown time";
  const elapsed = Math.max(0, Date.now() - date.getTime());
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} minute${minutes === 1 ? "" : "s"}`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} hour${hours === 1 ? "" : "s"}`;
  const days = Math.floor(hours / 24);
  return `${days} day${days === 1 ? "" : "s"}`;
}
function headerDetail(snapshot: UsageSnapshot) {
  return [snapshot.accountLabel, snapshot.plan].filter(Boolean).join(" · ");
}
function formatBalance(value: number, currency: string | null) {
  const amount = value.toLocaleString(undefined, { maximumFractionDigits: 2 });
  if (!currency) return amount;
  try {
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency,
    }).format(value);
  } catch {
    return `${currency} ${amount}`;
  }
}
function formatReset(
  value: string | null,
  dataKind: UsageSnapshot["dataKind"],
) {
  if (!value) return "Reset unavailable";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Reset unavailable";
  const localDateTime = date.toLocaleString([], {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
  return dataKind === "mock"
    ? `Sample reset ${localDateTime}`
    : `Resets ${localDateTime}`;
}
