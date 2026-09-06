import { useEffect, useState } from "react";
import { Cat } from "./components/Cat";
import { copy } from "./copy";
import { desktop, type Settings, type View } from "./lib/desktop";

const plannedProviders = [
  { name: "OpenAI / Codex", symbol: "O" },
  { name: "Anthropic / Claude", symbol: "A" },
  { name: "DeepSeek", symbol: "D" },
];

export default function App() {
  const [view, setView] = useState<View>("dashboard");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [saving, setSaving] = useState(false);
  const [retry, setRetry] = useState(0);
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
                <h2 id="usage-title">No usage data yet</h2>
                <p>
                  Provider connections are not available in this build.
                  <br />
                  Allowances and reset times will appear when data is available.
                </p>
              </div>
            </section>
            <section aria-labelledby="providers-heading">
              <div className="section-heading">
                <h2 id="providers-heading">Providers</h2>
                <span>0 connected</span>
              </div>
              <div className="providers">
                {plannedProviders.map((provider) => (
                  <article className="provider" key={provider.name}>
                    <span className="provider-symbol" aria-hidden="true">
                      {provider.symbol}
                    </span>
                    <div className="provider-name">
                      <h3>{provider.name}</h3>
                      <p>Connection support is planned</p>
                    </div>
                    <span className="unavailable">Not available yet</span>
                  </article>
                ))}
              </div>
            </section>
            <div className="bottom-note">
              <span>No provider requests. No usage estimates.</span>
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
