import { useEffect, useState } from "react";
import { Cat } from "./components/Cat";
import { copy } from "./copy";
import {
  desktop,
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
  const [retry, setRetry] = useState(0);
  const [providers, setProviders] = useState<ProviderOverview[]>([]);
  const visibleProviders = providers.filter(
    (provider) =>
      provider.snapshot
        ? provider.snapshot.hasSubscription !== false
        : provider.error !== "authentication_required",
  );
  const native = desktop.available();

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
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
    };
  }, [native, retry]);

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
              </div>
              <div className="providers">
                {providers.length === 0 ? (
                  <ProviderUnavailable />
                ) : (
                  visibleProviders.map((provider, index) => (
                    <ProviderCard
                      key={
                        provider.snapshot?.providerId ?? `error-${index}`
                      }
                      provider={provider}
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
    <fieldset disabled={busy}>
      <legend>Provider credentials</legend>
      <p className="credential-intro">
        Keys are stored in Windows Credential Manager and never shown again.
      </p>
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
          <span>Uses your `codex login` session; Ellie reuses it directly.</span>
        </div>
      </div>
      <p className="credential-message" role="status">
        {message}
      </p>
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
    return "Nothing active right now — hidden providers return when you subscribe or configure them.";
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
    return "Inactive providers are hidden until you subscribe or configure them.";
  }
  return hasLiveData(visible)
    ? "Live data comes from your codex CLI login on this machine; no token is stored."
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

function ProviderCard({ provider }: { provider: ProviderOverview }) {
  if (!provider.snapshot) {
    return (
      <article className="provider">
        <div className="provider-name">
          <h3>Provider unavailable</h3>
          <p>Ellie kept other provider results available.</p>
        </div>
        <span className="unavailable">
          {provider.error?.replaceAll("_", " ")}
        </span>
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
    </article>
  );
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
          : `${formatCount(tokens.requestCount)} requests`}
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
  const time = date.toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
  return dataKind === "mock" ? `Sample reset ${time}` : `Resets ${time}`;
}
