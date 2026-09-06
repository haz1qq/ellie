import { useEffect, useState } from "react";
import { Cat } from "./components/Cat";
import { copy } from "./copy";
import {
  desktop,
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
                  {providers.length === 0 ? "No usage data yet" : "Current usage"}
                </h2>
                <p>
                  {usageDescription(providers)}
                  <br />
                  Allowances and reset times appear when data is available.
                </p>
              </div>
            </section>
            <section aria-labelledby="providers-heading">
              <div className="section-heading">
                <h2 id="providers-heading">Providers</h2>
                <span>
                  {providers.length === 0 ? "0 connected" : "1 demo provider"}
                </span>
              </div>
              <div className="providers">
                {providers.length === 0 ? (
                  <ProviderUnavailable />
                ) : (
                  providers.map((provider, index) => (
                    <ProviderCard
                      key={provider.snapshot?.providerId ?? `error-${index}`}
                      provider={provider}
                    />
                  ))
                )}
              </div>
            </section>
            <div className="bottom-note">
              <span>{usageNote(providers)}</span>
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

function usageDescription(providers: ProviderOverview[]) {
  if (providers.length === 0) {
    return "Provider connections are not available in this build.";
  }
  return hasLiveData(providers)
    ? "Cards show live quota from your configured logins; demo cards stay labeled."
    : "Demo providers exercise Ellie's display and are not account data.";
}

function usageNote(providers: ProviderOverview[]) {
  if (providers.length === 0) {
    return "No provider requests.";
  }
  return hasLiveData(providers)
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
          <p>
            {snapshot.accountLabel} · {snapshot.plan}
          </p>
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
      {snapshot.capabilities.tokenUsage && snapshot.tokenUsage && (
        <div className="token-summary">
          <span>Sample token activity</span>
          <span>
            {formatCount(snapshot.tokenUsage.totalTokens)} tokens ·{" "}
            {formatCount(snapshot.tokenUsage.requestCount)} requests
          </span>
          <small>Locally calculated sample · illustrative only</small>
        </div>
      )}
    </article>
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

function formatCount(value: number | null) {
  return value === null ? "—" : new Intl.NumberFormat().format(value);
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
