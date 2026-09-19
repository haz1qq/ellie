import { useState } from "react";
import {
  githubErrorCopyOf,
  useGitHubConnection,
} from "../lib/github";
import { ConfirmDialog } from "./ConfirmDialog";

export function GitHubSettings({ native }: { native: boolean }) {
  const connection = useGitHubConnection(native);
  const { status, error: connectionError, busy, signInPending } = connection;
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [savedMessage, setSavedMessage] = useState("");
  const [cancelling, setCancelling] = useState(false);
  const [disconnectOpen, setDisconnectOpen] = useState(false);

  const connected = status?.state === "Connected";
  const authorizing = status?.state === "Authorizing";
  const waiting = signInPending || authorizing;
  const statusError = status?.lastError ? githubErrorCopyOf(status.lastError) : "";
  const alertText = connectionError || statusError;
  const trimmedClientId = clientId.trim();
  const trimmedClientSecret = clientSecret.trim();

  async function handleSaveClientId() {
    if (trimmedClientId === "" || busy) return;
    const saved = await connection.saveClientId(trimmedClientId);
    if (saved) setSavedMessage("GitHub App Client ID saved on this device.");
  }

  async function handleSaveClientSecret() {
    if (trimmedClientSecret === "" || busy) return;
    const saved = await connection.saveClientSecret(trimmedClientSecret);
    if (saved) {
      setClientSecret("");
      setSavedMessage("GitHub App Client Secret saved securely.");
    }
  }

  async function handleCancelSignIn() {
    setCancelling(true);
    try {
      await connection.cancelSignIn();
    } finally {
      setCancelling(false);
    }
  }

  async function handleDisconnectConfirm() {
    await connection.disconnect();
    setDisconnectOpen(false);
  }

  return (
    <fieldset className="provider-visibility github-settings" disabled={!native}>
      <legend>GitHub connection</legend>
      <p>
        Connect once to browse repositories and bounded commit history in the
        GitHub view. Quota monitoring, history, and provider credentials are
        unaffected.
      </p>

      <div className="github-client-id-row">
        <label htmlFor="github-client-id">GitHub App Client ID</label>
        <input
          id="github-client-id"
          type="text"
          value={clientId}
          placeholder="Iv1.…"
          disabled={busy}
          onChange={(event) => {
            setClientId(event.target.value);
            setSavedMessage("");
          }}
        />
        <button
          type="button"
          onClick={() => void handleSaveClientId()}
          disabled={busy || trimmedClientId === ""}
        >
          Save Client ID
        </button>
      </div>

      <div className="github-client-id-row">
        <label htmlFor="github-client-secret">GitHub App Client Secret</label>
        <input
          id="github-client-secret"
          type="password"
          value={clientSecret}
          placeholder="Enter the generated secret"
          autoComplete="off"
          spellCheck={false}
          disabled={busy}
          onChange={(event) => {
            setClientSecret(event.target.value);
            setSavedMessage("");
          }}
        />
        <button
          type="button"
          onClick={() => void handleSaveClientSecret()}
          disabled={busy || trimmedClientSecret === ""}
        >
          Save Client Secret
        </button>
      </div>

      <p role="status">
        {!native
          ? "Open the desktop app to connect GitHub."
          : !status
            ? "GitHub connection status unavailable."
            : connected
              ? `Connected as @${status.account?.login ?? "your account"}.`
              : waiting
                ? "Waiting for GitHub… Complete the sign-in in the browser tab that opened."
                : "Not connected to GitHub."}
      </p>
      {status && connected && (
        <p>
          {status.tokenPresent
            ? "Refresh token saved on this device — never shown."
            : "No refresh token saved yet."}
        </p>
      )}
      {status && (
        <>
          <p>
            {status.clientIdConfigured
              ? "GitHub App Client ID is set."
              : "No GitHub App Client ID saved yet — add yours above."}
          </p>
          <p>
            {status.clientSecretConfigured
              ? "GitHub App Client Secret is stored securely."
              : "No GitHub App Client Secret saved yet — generate one in GitHub and add it above."}
          </p>
        </>
      )}
      {savedMessage && <p role="status">{savedMessage}</p>}
      {alertText && <p role="alert">{alertText}</p>}

      <div className="save-row github-settings-actions">
        <button
          type="button"
          onClick={() => void connection.signIn()}
          disabled={
            busy ||
            connected ||
            waiting ||
            !status ||
            !status.clientIdConfigured ||
            !status.clientSecretConfigured
          }
        >
          Connect GitHub
        </button>
        {waiting && (
          <button
            type="button"
            onClick={() => void handleCancelSignIn()}
            disabled={cancelling}
          >
            Cancel sign-in
          </button>
        )}
        {connected && (
          <button
            type="button"
            onClick={() => setDisconnectOpen(true)}
            disabled={busy}
          >
            Disconnect GitHub
          </button>
        )}
      </div>

      <p className="github-privacy-note">
        The App Client Secret and refresh token are stored in Windows Credential
        Manager and are never displayed or returned to the interface. Disconnect
        removes both. You can also revoke the app in your GitHub account settings.
        Repositories and commits are fetched on demand and are not stored by Ellie.
      </p>

      <ConfirmDialog
        open={disconnectOpen}
        title="Disconnect GitHub?"
        body="This removes the connected account, App Client Secret, and refresh token from this device. The Client ID and local AI usage data remain, and you can reconnect after saving the secret again."
        confirmLabel="Disconnect"
        cancelLabel="Keep connected"
        busy={busy}
        onConfirm={() => void handleDisconnectConfirm()}
        onCancel={() => setDisconnectOpen(false)}
      />
    </fieldset>
  );
}