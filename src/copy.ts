import type { GitHubErrorCategory } from "./lib/desktop";

export const copy = {
  greeting: "There you are. I saved your spot.",
  quiet: "A little company. A clearer picture.",
  hidden: "I’ll be in the tray.",
};

/**
 * Friendly, precise copy for GitHub error categories. These strings are shown
 * instead of raw categories or stack traces; they never reveal internals and
 * never invent data.
 */
export const githubErrorCopy: Record<GitHubErrorCategory, string> = {
  window_denied: "This window cannot manage GitHub. Use the main Ellie window.",
  invalid_input: "The GitHub request isn’t valid. Check the repository and branch details, then retry.",
  busy: "A GitHub sign-in or request is already in progress. Let it finish, or cancel it first.",
  authorization_state_mismatch: "The sign-in response didn’t match this request. Try connecting again.",
  authorization_denied: "GitHub declined the sign-in. No account was connected; you can try again.",
  app_credentials_invalid: "GitHub rejected the App Client ID or Client Secret. Check both values in Settings, then retry.",
  token_expiration_required: "GitHub returned a non-expiring user token without a refresh token. Keep User-to-server token expiration enabled, revoke the existing app authorization, wait a few seconds, then reconnect.",
  token_response_invalid: "GitHub returned token metadata Ellie cannot safely use. No token was stored. Please report this diagnostic before retrying.",
  account_response_invalid: "GitHub issued a token, but the account response was not usable. No connection was saved. Please report this diagnostic.",
  authentication_required: "Save the GitHub App Client ID and Client Secret, then connect your account.",
  authentication_expired: "Your GitHub session expired. Reconnect to continue.",
  rate_limited: "GitHub is rate-limiting requests right now. Wait a little, then retry.",
  permission_denied: "GitHub denied access for this request. Check the app’s permissions in GitHub, or reconnect.",
  not_found: "The requested GitHub resource wasn’t found. It may have been renamed or removed.",
  validation_failed: "GitHub rejected the request. Check the repository name and branch, then retry.",
  network_unavailable: "GitHub isn’t reachable. Check your connection, then retry.",
  provider_unavailable: "GitHub is temporarily unavailable. Retry in a moment.",
  malformed_response: "GitHub returned an unexpected response. Existing data is unchanged; retry.",
  credential_store: "GitHub credentials couldn’t be saved securely. Check Windows Credential Manager access, then retry.",
  cancelled: "The GitHub operation was cancelled.",
  conflict: "A repository with that name already exists on the connected account. Choose another name.",
  creation_outcome_unknown: "GitHub received the creation request, but the result was lost. Inspect your repositories before retrying; Ellie will not create it twice on its own.",
};

/** Fallback copy for unknown or non-categorical failures; never raw errors. */
export const githubErrorGeneric =
  "GitHub couldn’t complete the request. Existing data is unchanged; retry.";