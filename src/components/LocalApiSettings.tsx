import { useEffect, useRef, useState } from "react";
import { desktop, type LocalApiAction, type LocalApiFailure, type LocalApiStatus } from "../lib/desktop";

const failureCopy: Record<LocalApiFailure, string> = {
  credentialStore: "Windows Credential Manager is unavailable. Check access, then retry. An unsuccessful rotation keeps the previous token.",
  invalidOverride: "ELLIE_API_TOKEN is invalid. Unset it and restart Ellie to use Windows Credential Manager; no fallback was attempted.",
  missingToken: "The stored token is missing. Enable Local API again to create one.",
  persistence: "The preference could not be saved. Requests remain denied after a failed disable; retry before restarting Ellie.",
  bind: "Could not listen on 127.0.0.1:9876. Close the conflicting listener, then retry enabling.",
  server: "The local API stopped unexpectedly. Try enabling it again.",
  overrideActive: "Rotation is unavailable while ELLIE_API_TOKEN overrides the stored token.",
  disabled: "Enable Local API before rotating its token.",
  controlsUnavailable: "This window cannot control Local API. Use the owning Ellie instance, or close other Ellie instances and restart this one. If none are open, check application data folder access. No change was saved.",
};

export function LocalApiSettings({ native }: { native: boolean }) {
  const [status, setStatus] = useState<LocalApiStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  // Mount reads, manual reads, and mutations share one ordering boundary.
  const requestGeneration = useRef(0);

  useEffect(() => {
    const requestGenerationRef = requestGeneration;
    const generation = ++requestGenerationRef.current;
    if (native) {
      desktop.localApiStatus().then((value) => {
        if (generation !== requestGenerationRef.current) return;
        setStatus(value);
        setError("");
        setBusy(false);
      }).catch(() => {
        if (generation !== requestGenerationRef.current) return;
        setError("Local API status could not be read. Retry status.");
        setBusy(false);
      });
    }
    return () => { ++requestGenerationRef.current; };
  }, [native]);

  async function update(action?: LocalApiAction) {
    const generation = ++requestGeneration.current;
    setBusy(true);
    setError("");
    try {
      const value = action ? await desktop.configureLocalApi(action) : await desktop.localApiStatus();
      if (generation !== requestGeneration.current) return;
      setStatus(value);
    } catch {
      if (generation !== requestGeneration.current) return;
      // An IPC transport failure may follow a successful native change. Do not
      // retain a stale operational claim; an explicit status read recovers it.
      setStatus(null);
      setError("Local API status is unknown. Retry status before making another change.");
    } finally {
      if (generation === requestGeneration.current) setBusy(false);
    }
  }

  const controlsUnavailable = status?.error === "controlsUnavailable";

  return (
    <fieldset className="provider-visibility" disabled={!native || busy}>
      <legend>Integrations · Local API</legend>
      <p>Off by default, including upgrades. Enable to let ellie-cli read usage through 127.0.0.1:9876/api/v1. Changes apply immediately.</p>
      <p role="status">
        {!native ? "Open the desktop app to manage Local API." : !status ? "Status unavailable" : status.listening ? "Listening on 127.0.0.1:9876" : status.enabled ? "Enabled preference · Not listening" : "Disabled · Not listening"}
      </p>
      {status && <p>Token source: {status.tokenSource === "environment" ? "ELLIE_API_TOKEN override" : status.tokenSource === "credentialManager" ? "Windows Credential Manager" : "Not loaded"}. Token values are never displayed.</p>}
      <p>The CLI reads the stored token automatically. An explicit ELLIE_API_TOKEN override takes precedence and never falls back. Disable denies requests even with an override.</p>
      {status?.tokenSource === "environment" && <p>To use automatic authentication or rotation, unset ELLIE_API_TOKEN and restart Ellie and your CLI environment.</p>}
      {(error || status?.error) && <p role="alert">{error || (status?.error ? failureCopy[status.error] : "")}</p>}
      <div className="save-row">
        <button type="button" disabled={controlsUnavailable || !status || status.listening} onClick={() => void update("enable")}>Enable Local API</button>
        <button type="button" disabled={controlsUnavailable || !status || (!status.enabled && !status.listening)} onClick={() => void update("disable")}>Disable Local API</button>
        <button type="button" disabled={controlsUnavailable || !status?.listening || status.tokenSource === "environment"} onClick={() => void update("rotate")}>Rotate API token</button>
        <button type="button" onClick={() => void update()}>Refresh API status</button>
      </div>
      <p>Rotation immediately invalidates the old token for subsequent requests. The next CLI invocation reads the replacement; already-authorized work may finish.</p>
    </fieldset>
  );
}
