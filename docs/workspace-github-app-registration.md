# GitHub App registration and PKCE flow — W2 preparation

**Status: documentation only.** Follows the approved W1 decision (owner-selected **option A**: GitHub App + web application flow with PKCE and a `127.0.0.1` loopback redirect; see [W1 record](workspace-github-auth.md)). This document is the registration checklist, the exact permission request, the consent copy, and the PKCE implementation notes that W2 must verify before any code. **Nothing here has been executed; the GitHub App does not exist yet.**

## 1. Who does what

| Step | Owner (you) | Ellie implementation (later) |
| --- | --- | --- |
| Register the GitHub App under your account | Yes — at `github.com/settings/apps/new` | — |
| Enter app metadata, callback URL, permissions | Yes (using values below) | — |
| Enable user-token expiration | Yes (one toggle) | — |
| Provide the App Client ID (non-secret) | Yes — to Settings | Read non-secret client ID; never a client secret (none exists for PKCE) |
| Sign in, exchange code, store refresh token | Authorize in browser | Rust code-verifier → exchange → Credential Manager |
| Read repositories/commits, create repositories | Confirm in Ellie | `GET /user/repos`, `GET /repos/{o}/{r}/commits`, `POST /user/repos` |

No secret value is embedded in the binary: PKCE uses a runtime `code_verifier`; the client secret is never required for the web flow with PKCE on a public client per the W1 record.

## 2. App-registration checklist (owner, in GitHub)

Fill in on the GitHub App **settings → General** page for the new app:

1. **GitHub App name**: `ellie — AI usage companion` (or a name you prefer; visible to users).
2. **Description**: short line, e.g. “Tracks commits in your repositories and creates new repositories on your behalf.”
3. **Homepage URL**: any reachable URL you control (required field; used only as a link target).
4. **User authorization callback URL** (required):
   - Set the base callback to `http://127.0.0.1/callback`.
   - Because GitHub allows a **loopback redirect URL on any port** for native apps, runtime redirects such as `http://127.0.0.1:58210/callback` match this registration without per-port registration (verified in W1: *“For the `http://127.0.0.1/path` callback URL, you can use this `redirect_uri` if your application is listening on port 1234.”*).
   - Use `127.0.0.1`, not `localhost` (RFC 8252 guidance, W1).
   - Leave **wildcard matching off**.
5. **Expire user authorization tokens**: **enable** (this is what returns the 8-hour user access token plus a 6-month refresh token; strongly encouraged by GitHub).
6. **Request user authorization (OAuth) during installation**: recommended **off**; Ellie will start the web flow itself from Settings → GitHub → Connect.
7. **Webhook**: not needed for the first slice. (Deferred: `github_app_authorization` revocation webhook requires a public endpoint; W1 section 7 item 4 decides whether 401s suffice.)
8. **Permissions — Repository permissions**:
   - **Contents**: **Read-only** (enables `GET /repos/{owner}/{repo}/commits` and related commit reads).
   - **Administration**: **Read and write** — required for `POST /user/repos` (create a repository for the authenticated user); write here is scoped to the user-granted resources, not repository deletion. **Confirm at registration** that the consent screen labels this as repository creation rather than broader admin; if GitHub presents a wider-looking wording, note it in section 7 before approving consent copy.
   - All other permissions: **No access**.
9. **Account permissions**: leave default (none additional). `GET /user/repos` relies on the app's repository grant; **confirm at registration** whether listing own repositories appears under a repository permission label.
10. **Repository access**: choose **All repositories** on the authorizing account during installation, so every repository the owner later selects for tracking is readable and newly created repositories remain visible. Ellie stores only the tracked subset locally; selection is presentation data, not an API grant.
11. **Where the app can be installed**: your personal account only (this slice is personal-account repositories; organization creation remains deferred).
12. After creation, copy the **Client ID** (public, non-secret) into Ellie Settings → GitHub. Treat it as public metadata; never treat it as a credential, and never ask for a client secret.

## 3. PKCE flow implementation notes (for the later Rust implementation)

These are design notes, not code:

- **Authorize URL** (`https://github.com/login/oauth/authorize`):
  - `client_id` (from registration), `response_type=code`, `redirect_uri=http://127.0.0.1:<ephemeral_port>/callback`.
  - `code_challenge` = base64url(SHA-256(verifier)); `code_challenge_method=S256` (only S256 is supported — W1 verified).
  - `state` = unguessable random; validated on redirect to prevent CSRF.
  - `prompt=select_account` optional to force the account picker.
- **Token exchange** (`POST https://github.com/login/oauth/access_token`): send `client_id`, `code`, `redirect_uri`, `code_verifier`, `grant_type=authorization_code`. Accept `application/json`. No client secret with PKCE.
- **Session flow**: Rust opens the system browser; Rust owns the ephemeral loopback listener (random high port per attempt); parse `code` + `state` from the loopback GET; close the listener immediately; validate `state`; exchange; never let the WebView see the code or token.
- **Credential storage**: store only the **refresh token** in Windows Credential Manager under a dedicated Ellie GitHub identity. Keep the 8-hour user access token in memory; regenerate via `grant_type=refresh_token` (W1: `refresh_token_expires_in` 6 months). Store whatever token the refresh response returns (rotation behavior to be confirmed live — W1 section 7 item 3).
- **Failure signals**: `bad_refresh_token` → prompt reconnect; 401 on API → mark sync paused and attempt one refresh; `access_denied`/`token expired` → clear session state and show re-authorize.
- **Consent-time boundary**: only after a successful exchange that returns a non-`gho_` (user access token, `ghu_`) token may Ellie persist the connection record and begin reads.

## 4. Permission request and consent copy

### What Ellie requests (exact)

- **Contents: read** on your repositories — to list repositories and read commit history for repositories you choose to track.
- **Administration: read/write** as required by GitHub’s API for **creating new repositories** — used only for the authenticated-user “Create a repository” endpoint (`POST /user/repos`), defaulting new repositories to private, with a public-visibility confirmation.

No repository deletion, code editing, branch/PR/issue operations, members, secrets, or organization-level administration is requested or used.

### Ellie "Connect GitHub" panel copy (Settings → GitHub)

> **Connect GitHub**
> Ellie will open GitHub in your browser to let you sign in.
> Ellie asks for two things only:
> - **Read** your repositories and their commit history, for repositories you choose to track.
> - **Create** new repositories when you use the New repository action. New repositories default to private.
> Ellie never edits, deletes, or changes your repositories, and never touches issues, pull requests, or members.
> The sign-in uses GitHub's secure device-safe OAuth flow with PKCE. Only a refresh token is saved to Windows Credential Manager on this PC; it can be revoked from GitHub at any time, after which Ellie stops sync and asks you to reconnect.
> [Connect to GitHub]  [Learn what Ellie stores locally]

### Browser authorization screen explanation (shown before opening the browser)

> You're about to be redirected to github.com. On GitHub's screen, review the requested permissions (repository content: read; create repositories with private default) and approve them to continue. The code returned to Ellie stays on this machine.

### Public-visibility warning (New repository)

> You're creating a **public** repository on GitHub. Anyone can see it. Public repositories are visible to everyone on the internet. [Make it private] [Continue with public]

## 5. Privacy/storage expectations

- Stored locally: non-secret connection record (account ID/login, status, timestamps), tracked-repository selection, GitHub cache (commits, coverage), task repository links. The **refresh token** is in Windows Credential Manager, never SQLite/IPC/events/logs.
- The 90-day GitHub cache and task links do not leave this PC; no Ellie cloud.
- Same-user malware limitation from `docs/security.md` applies; no capture-exclusion or encryption promises.

## 6. What W2 will verify live (after you create the app)

1. Consent screen shows exactly Contents read + create-repository permission; no wider label.
2. `GET /user/repos` works with the UAT and its exact permission label.
3. Refresh-token rotation behavior (same vs new token) for correct storage.
4. 422 on duplicate repository name; 401/403 behavior under permission loss; rate-limit headers.
5. Loopback redirect on a random ephemeral port matches the registered `http://127.0.0.1/callback` base.
6. Revocation detection: 401 vs optional webhook; choose the simpler verified signal.

## 7. Open confirm-at-registration items

- Exact permission-label wording GitHub shows for `POST /user/repos` and `GET /user/repos` under a GitHub App (affects the consent text in section 4).
- Whether GitHub shows an account-level "create repositories" permission separate from repository `Administration`; if so, use that narrower grant if available.
- Default `expire user tokens` behavior for newly created apps and any deprecation notices on the settings page.

These items cannot be observed until the app exists; they are recorded now so they are not mistaken for verified facts.