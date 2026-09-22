import { useEffect, useId, useState } from "react";
import { CheckCircle2, KeyRound, Loader2 } from "lucide-react";
import { toast } from "sonner";
import type { ProviderKeySource, ProviderOverview, Settings } from "../../lib/desktop";
import { desktop } from "../../lib/desktop";
import { GitHubSettings } from "../GitHubSettings";
import { LocalApiSettings } from "../LocalApiSettings";
import { Button } from "../ui/Button";
import { Checkbox } from "../ui/Checkbox";

export interface SettingsPageProps {
  native: boolean;
  settings: Settings | null;
  loading: boolean;
  providers: ProviderOverview[];
  /** Persists the saved settings object to the App-level state. */
  onChangeSettings: (settings: Settings) => void;
  onRefreshAll: () => void;
  refreshing: boolean;
}

export function SettingsPage({
  native,
  settings,
  loading,
  providers,
  onChangeSettings,
  onRefreshAll,
  refreshing,
}: SettingsPageProps) {
  const [draft, setDraft] = useState<Settings | null>(settings);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => setDraft(settings), [settings]);

  const dirty =
    settings !== null && draft !== null && JSON.stringify(settings) !== JSON.stringify(draft);

  async function save() {
    if (!draft || saving) return;
    setSaving(true);
    setSaved(false);
    try {
      const savedSettings = await desktop.saveSettings(draft);
      onChangeSettings(savedSettings);
      setSaved(true);
      toast.success("Settings saved on this device");
    } catch {
      toast.error("Your settings were not saved. Previous settings are still active. Try again.");
    } finally {
      setSaving(false);
    }
  }

  async function setProviderVisibility(providerId: string, visible: boolean) {
    if (!settings || saving || !native) return;
    setSaving(true);
    const hiddenProviderIds = visible
      ? (settings.hiddenProviderIds ?? []).filter((id) => id !== providerId)
      : [...new Set([...(settings.hiddenProviderIds ?? []), providerId])];
    try {
      const savedSettings = await desktop.saveSettings({ ...settings, hiddenProviderIds });
      onChangeSettings(savedSettings);
      setDraft((current) =>
        current ? { ...current, hiddenProviderIds: savedSettings.hiddenProviderIds } : savedSettings,
      );
      toast.success(visible ? "Provider display enabled." : "Provider hidden.");
    } catch {
      toast.error("Provider visibility was not saved. Try again.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="settings-page" aria-labelledby="settings-title">
      <div className="todo-toolbar">
        <div className="todo-toolbar-title">
          <p className="eyebrow">Local controls</p>
          <h1 className="page-title" id="settings-title">
            Settings
          </h1>
          <p className="page-sub">Small preferences, saved on this device.</p>
        </div>
        <div className="todo-toolbar-actions">
          <Button
            variant="ghost"
            onClick={onRefreshAll}
            disabled={!native || refreshing}
          >
            <Loader2 size={14} className={refreshing ? "spin" : ""} />
            {refreshing ? "Refreshing…" : "Refresh providers"}
          </Button>
        </div>
      </div>

      {!native && !loading && settings === null && (
        <p className="panel-muted">
          Open the desktop app to manage your local preferences.
        </p>
      )}

      {draft && (
        <>
          <section className="panel">
            <header className="panel-header">
              <div className="panel-title">
                <span className="panel-kicker">Window & appearance</span>
                <h2 className="panel-heading">Preferences</h2>
              </div>
              {saved && !dirty && (
                <span className="saved-chip">
                  <CheckCircle2 size={12} /> Saved
                </span>
              )}
            </header>
            <form
              className="settings-form"
              onSubmit={(event) => {
                event.preventDefault();
                void save();
              }}
            >
              <fieldset disabled={saving || loading}>
                <Setting
                  label="Close to tray"
                  detail="Keep Ellie running when you close the window. Quit from the tray menu."
                  checked={draft.closeToTray}
                  onChange={(value) => setDraft({ ...draft, closeToTray: value })}
                />
                <Setting
                  label="Mini floating bar"
                  detail="Keep provider-reported quota remaining visible in a separate always-on-top bar."
                  checked={draft.miniBarEnabled}
                  onChange={(value) => setDraft({ ...draft, miniBarEnabled: value })}
                />
                <label className="setting setting-slider">
                  <span>
                    <strong>Mini bar opacity</strong>
                    <span className="setting-detail">
                      Adjust the mini bar surface from 50% to fully opaque.
                    </span>
                  </span>
                  <span className="opacity-control">
                    <input
                      type="range"
                      min="0.5"
                      max="1"
                      step="0.05"
                      value={draft.miniBarOpacity}
                      aria-label="Mini bar opacity"
                      onChange={(event) =>
                        setDraft({ ...draft, miniBarOpacity: Number(event.target.value) })
                      }
                    />
                    <output>{Math.round(draft.miniBarOpacity * 100)}%</output>
                  </span>
                </label>
                <Setting
                  label="Show current task in mini bar"
                  detail="Adds the pinned task title to the mini bar. Task content can appear in screen sharing; there is no capture-exclusion guarantee."
                  checked={draft.miniBarShowTask}
                  onChange={(value) => setDraft({ ...draft, miniBarShowTask: value })}
                />
                <Setting
                  label="Show GitHub activity in mini bar"
                  detail="Adds the loaded commit count and freshness to the mini bar using data Ellie already fetched; the mini bar never polls GitHub itself."
                  checked={draft.miniBarShowGitHub}
                  onChange={(value) => setDraft({ ...draft, miniBarShowGitHub: value })}
                />
                <Setting
                  label="Show dashboard mascot"
                  detail="A little black-and-white company. Always still, never distracting."
                  checked={draft.showMascot}
                  onChange={(value) => setDraft({ ...draft, showMascot: value })}
                />
                <Setting
                  label="Friendly messages"
                  detail="A few warm words alongside the facts."
                  checked={draft.friendlyMessages}
                  onChange={(value) => setDraft({ ...draft, friendlyMessages: value })}
                />
                <Setting
                  label="Usage notifications"
                  detail="Notify when a provider-reported quota window reaches one of the thresholds below."
                  checked={draft.notificationsEnabled}
                  onChange={(value) => setDraft({ ...draft, notificationsEnabled: value })}
                />
                <NotificationThresholds
                  thresholds={draft.notificationThresholds}
                  onChange={(notificationThresholds) =>
                    setDraft({ ...draft, notificationThresholds })
                  }
                />
              </fieldset>
              <div className="save-row">
                <Button type="submit" variant="primary" disabled={!dirty || saving || loading}>
                  {saving ? "Saving…" : "Save settings"}
                </Button>
                <span className="save-hint" role="status">
                  {dirty ? "Unsaved changes" : ""}
                </span>
              </div>
            </form>
          </section>

          <section className="panel">
            <header className="panel-header">
              <div className="panel-title">
                <span className="panel-kicker">Display</span>
                <h2 className="panel-heading">Provider visibility</h2>
              </div>
            </header>
            <p className="panel-description">
              Changes save immediately. Hidden cards keep their credentials, history,
              and fetching. Unconfigured or unsubscribed providers remain hidden.
            </p>
            <div className="provider-visibility-list">
              {providers.map((provider) => (
                <Setting
                  key={provider.providerId}
                  label={`Show ${provider.displayName} on dashboard`}
                  detail={
                    provider.snapshot?.dataKind === "mock"
                      ? "Demo provider · illustrative data, not a connected account."
                      : "Display preference only; this does not disconnect your account."
                  }
                  checked={!(settings?.hiddenProviderIds ?? []).includes(provider.providerId)}
                  onChange={(visible) => void setProviderVisibility(provider.providerId, visible)}
                />
              ))}
            </div>
          </section>
        </>
      )}

      <LocalApiSettings native={native} />

      <section className="panel">
        <header className="panel-header">
          <div className="panel-title">
            <span className="panel-kicker">Connected work</span>
            <h2 className="panel-heading">GitHub</h2>
          </div>
        </header>
        <GitHubSettings native={native} />
      </section>

      <ProviderCredentials />

      <div className="settings-note">
        <h2>Dark mode, by default.</h2>
        <p>More preferences will appear as new features become available.</p>
      </div>
    </section>
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
  const id = useId();
  return (
    <div className="setting">
      <label htmlFor={id}>
        <strong>{label}</strong>
        <span className="setting-detail">{detail}</span>
      </label>
      <Checkbox
        id={id}
        checked={checked}
        onCheckedChange={onChange}
        label={label}
      />
    </div>
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

function ProviderCredentials() {
  const [status, setStatus] = useState<Map<string, ProviderKeySource>>(new Map());
  const [openAiAdmin, setOpenAiAdmin] = useState("");
  const [anthropic, setAnthropic] = useState("");
  const [deepseek, setDeepseek] = useState("");
  const [busy, setBusy] = useState(false);

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
      toast.success("Key saved to Windows Credential Manager.");
      refresh();
    } catch {
      toast.error("The key could not be saved.");
    } finally {
      setBusy(false);
    }
  };
  const removeKey = async (providerId: string) => {
    setBusy(true);
    try {
      await desktop.deleteProviderKey(providerId);
      toast.success("Key removed.");
      refresh();
    } catch {
      toast.error("The key could not be removed.");
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
    <section className="panel" aria-label="Provider credentials">
      <header className="panel-header">
        <div className="panel-title">
          <span className="panel-kicker">Keys stay here</span>
          <h2 className="panel-heading">
            <KeyRound size={14} className="panel-heading-icon" />
            Provider credentials
          </h2>
        </div>
      </header>
      <p className="panel-description">
        Keys are stored in Windows Credential Manager and never shown again.
      </p>
      <div className="credential-list" aria-busy={busy}>
        <CredentialRow
          name="OpenAI API"
          detail={`Admin API key for separately billed API token usage · ${sourceLabel(status.get("openai-api"))}`}
          placeholder="sk-admin-…"
          ariaLabel="OpenAI Admin API key"
          value={openAiAdmin}
          onChange={setOpenAiAdmin}
          busy={busy}
          source={status.get("openai-api")}
          onSave={() => void saveKey("openai-api", openAiAdmin, () => setOpenAiAdmin(""))}
          onRemove={() => void removeKey("openai-api")}
        />
        <CredentialRow
          name="Anthropic / Claude"
          detail={`Admin key (sk-ant-admin) for usage and cost reports · ${sourceLabel(status.get("anthropic-claude"))}`}
          placeholder="sk-ant-admin-…"
          ariaLabel="Anthropic API key"
          value={anthropic}
          onChange={setAnthropic}
          busy={busy}
          source={status.get("anthropic-claude")}
          onSave={() => void saveKey("anthropic-claude", anthropic, () => setAnthropic(""))}
          onRemove={() => void removeKey("anthropic-claude")}
        />
        <CredentialRow
          name="DeepSeek"
          detail={`API key from platform.deepseek.com · ${sourceLabel(status.get("deepseek"))}`}
          placeholder="sk-…"
          ariaLabel="DeepSeek API key"
          value={deepseek}
          onChange={setDeepseek}
          busy={busy}
          source={status.get("deepseek")}
          onSave={() => void saveKey("deepseek", deepseek, () => setDeepseek(""))}
          onRemove={() => void removeKey("deepseek")}
        />
        <div className="credential-row credential-row-static">
          <div className="credential-info">
            <strong>OpenAI / Codex</strong>
            <span>Uses your `codex login` session; Ellie reuses it directly.</span>
          </div>
        </div>
      </div>
    </section>
  );
}

function CredentialRow({
  name,
  detail,
  placeholder,
  ariaLabel,
  value,
  onChange,
  busy,
  source,
  onSave,
  onRemove,
}: {
  name: string;
  detail: string;
  placeholder: string;
  ariaLabel: string;
  value: string;
  onChange: (value: string) => void;
  busy: boolean;
  source: ProviderKeySource | undefined;
  onSave: () => void;
  onRemove: () => void;
}) {
  return (
    <div className="credential-row">
      <div className="credential-info">
        <strong>{name}</strong>
        <span>{detail}</span>
      </div>
      <input
        type="password"
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        aria-label={ariaLabel}
        disabled={busy}
      />
      <Button size="sm" aria-label={`Save ${name} key`} onClick={onSave} disabled={busy}>
        Save
      </Button>
      {source === "credential_manager" && (
        <Button size="sm" variant="ghost" onClick={onRemove} disabled={busy}>
          Remove
        </Button>
      )}
    </div>
  );
}