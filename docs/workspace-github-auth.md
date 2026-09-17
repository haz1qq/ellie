# GitHub authentication and permissions — W1 feasibility record

**Status: implemented; live token-exchange correction recorded.** This record documents verified official GitHub facts and the chosen authentication mechanism for Ellie's GitHub **track + create** scope. The owner approved **GitHub App + PKCE web flow with a `127.0.0.1` loopback redirect** (option A). W2 registration and setup instructions are in [`workspace-github-app-registration.md`](workspace-github-app-registration.md). During live verification, GitHub accepted the callback but rejected Ellie's secret-less token exchange. Re-checking GitHub's current official GitHub App documentation confirmed that `client_secret` is **required** at both code exchange and token refresh even when PKCE is used. The owner approved storing that secret only in Windows Credential Manager.

Research performed with official GitHub documentation pages (cited inline). Facts marked **verified** were read directly from the cited pages; items marked **confirm at registration** are decisions/behaviors to check live in the app-registration or consent screens during W2. The structured source check produced no contradicting passages; two provider-side answer failures are noted in the appendix.

## 1. Verified: GitHub supports both desktop sign-in flows

| Flow | Verified facts | Source |
| --- | --- | --- |
| Device flow | Recommended explicitly for desktop applications: *"CLI tools, simple Raspberry Pis, and desktop applications should use the device flow."* Requires enabling in the app's settings. User enters an 8-character code at `https://github.com/login/device`; device code valid 15 minutes; poll `POST /login/oauth/access_token` every 5+ seconds. Errors: `slow_down`, `access_denied`, `device_flow_disabled`, `token_expired`. No redirect URI and **no client secret required**. | [Generating a user access token for a GitHub App](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app) |
| Web flow + PKCE | PKCE (RFC 7636, `S256` only) is **supported and recommended** since July 14, 2025 for both OAuth Apps and GitHub Apps user authentication. `code_challenge`/`code_challenge_method` are strongly recommended on the authorize URL; `code_verifier` is required at exchange when a challenge was sent; `state` is recommended. For a **GitHub App**, GitHub's token-exchange documentation separately marks `client_secret` as **required**; PKCE supplements rather than replaces the App secret. | [PKCE support changelog](https://github.blog/changelog/2025-07-14-pkce-support-for-oauth-and-github-app-authentication/), [Generating a user access token for a GitHub App](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app), [Authorizing OAuth apps](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps) |
| Loopback redirect | Native desktop apps can register `http://127.0.0.1/path` and redirect to any port: *"For the `http://127.0.0.1/path` callback URL, you can use this `redirect_uri` if your application is listening on port 1234: `http://127.0.0.1:1234/path`."* RFC 8252 recommends the loopback literal `127.0.0.1` (or IPv6 `::1`) over `localhost`. | [Authorizing OAuth apps — loopback redirect URLs](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps) |
| Phishing trade-off | GitHub's own guidance: *"It is preferable to use the authorization code with PKCE over the device flow."* Device flow has no redirect URIs, which raises phishing-abuse risk; it must be deliberately enabled. | [Best practices for creating a GitHub App](https://docs.github.com/en/apps/creating-github-apps/about-creating-github-apps/best-practices-for-creating-a-github-app) |

## 2. Verified: GitHub Apps are the recommended app type

*"In general, GitHub Apps are preferred over OAuth Apps. GitHub Apps use fine-grained permissions, give the user more control over which repositories the app can access, and use short-lived tokens."* OAuth App tokens persist long-lived with broad scopes and manual revocation only.

**Credential lifespan (verified):**

| Credential | Prefix | Lifespan | Revocation |
| --- | --- | --- | --- |
| OAuth App access token | `gho_` | Long-lived | Manual |
| GitHub App user access token | `ghu_` | Short-lived: **8 hours** (default, strongly encouraged) | Automatic expiry or manual |
| GitHub App refresh token | `ghr_` | **6 months** | Manual / expiry |
| Fine-grained PAT | `github_pat_` | Configurable up to 1 year, or no expiry | Manual |

Verified: GitHub App user access tokens returned by either flow include `expires_in` (28800 s = 8 h) and `refresh_token_expires_in` (15897600 s = 6 months) when expiring tokens are enabled. Refresh tokens regenerate user access tokens; `bad_refresh_token` errors require restarting the sign-in flow.

## 3. Verified: endpoints and permissions for track + create

| Operation | Endpoint | Token/permission needed (verified) |
| --- | --- | --- |
| List the user's repositories | `GET /user/repos` | Available to GitHub App user access tokens (read). Confirm the exact permission label at registration. |
| List commits in a repository | `GET /repos/{owner}/{repo}/commits` | GitHub App: repository permission **Contents: read** (user access token or installation token). Fine-grained PAT: Contents read. |
| Get repository + branch metadata | `GET /repos/{owner}/{repo}` · branches endpoints | Read access (Contents/Metadata); confirm exact labels at registration. |
| Create a repository for the authenticated user | `POST /user/repos` | **Available to GitHub App user access tokens (write)** — listed in the endpoints table. Fine-grained tokens: repository **Administration: write**. OAuth App tokens: `public_repo` or `repo` scope for public; `repo` scope for private. |

Sources: [Endpoints available for GitHub App user access tokens](https://docs.github.com/en/rest/authentication/endpoints-available-for-github-app-user-access-tokens), [Permissions required for GitHub Apps](https://docs.github.com/en/rest/authentication/permissions-required-for-github-apps), [Permissions required for fine-grained PATs](https://docs.github.com/en/rest/authentication/permissions-required-for-fine-grained-personal-access-tokens), [REST repositories](https://docs.github.com/en/rest/repos/repos).

The earlier community thread suggesting GitHub App user access tokens cannot create repositories applies to **installation tokens**; the official endpoints table explicitly lists `POST /user/repos` as available to **user access tokens**.

**Scope semantics (verified):** GitHub App user access tokens carry no scopes; they are limited to the intersection of the app's granted permissions and the user's own permissions — the user remains in control. This makes "Administration: write" on an app token materially narrower than the OAuth App `repo` scope (which grants full private-repository control).

## 4. Verified: rate limits

- Authenticated REST: **5,000 requests/hour** personal primary rate limit; fetching the quota endpoint (`GET /rate_limit`) reports remaining. Requests on behalf of a GitHub App owned by a GitHub Enterprise Cloud organization receive 15,000/hour.
- GitHub App user access tokens and installation tokens have app-level primary limits (installation: 5,000/hour; Enterprise Cloud org-owned apps higher).
- Search endpoints: 30 requests/minute authenticated (10/minute for code search), separate from the primary limit.

Sources: [Rate limits for the REST API](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api), [Rate limits for GitHub Apps](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/rate-limits-for-github-apps), [Search API](https://docs.github.com/en/rest/search/search).

The 5-minute polling proposal in the backend design is far below these limits for a small tracked-repository set, but page-depth, retry-on-secondary-limit, and quota-check behavior must still be implemented.

## 5. Proposed mechanism and recommendation

**Recommended: GitHub App + web application flow with PKCE and a `127.0.0.1` loopback redirect.**

Rationale:
- Trusts GitHub's own guidance: PKCE web flow over device flow; GitHub Apps over OAuth Apps.
- No client secret embedded in the desktop binary or SQLite. The owner supplies the generated GitHub App secret at runtime; Rust stores it under a dedicated Windows Credential Manager identity. The `code_verifier` remains memory-only, and the refresh token uses a separate Credential Manager identity.
- Loopback redirect means no public callback server; the code never leaves the machine.
- Narrow fine-grained permissions: **Contents: read** + **Administration: write** (user-granted), giving exactly commits/repository listing plus personal repository creation.
- Short-lived user access tokens (8 h) with automatic refresh-token renewal (6 months) already matching Ellie's polling model; revocation detectable via `github_app_authorization` webhook/401 (verifiable in W2).
- Device flow remains the supported fallback if the owner later prefers no browser interaction; it requires enabling in app settings and carries GitHub's documented phishing trade-off, so PKCE is primary.

Proposed permission consent (single authorization, two documented capabilities): read repositories and commit history for authorized repositories; create new personal repositories (explicit `POST /user/repos`) with private default and a public-visibility disclosure. No repository edits, deletions, issues, PRs, members, or administration beyond new-repository creation.

**Alternatives (all feasible, with trade-offs):**

| Option | Pros | Cons |
| --- | --- | --- |
| A. GitHub App + PKCE web flow (recommended) | Least privilege; short-lived credentials; loopback redirect; GitHub-endorsed; PKCE protects the authorization code | Requires registering a GitHub App and securely storing its required client secret at runtime; PKCE is a 2025 feature (fine-grained apps created earlier may need checking) |
| B. GitHub App + device flow | No browser redirect handling at all; no secret | GitHub documents phishing trade-off; extra user step (enter code); must enable device flow |
| C. OAuth App + device flow/`repo` scope | Simplest registration; device flow documented | Needs broad `repo` scope for private access — far exceeds track/create; long-lived token; manual revocation |
| D. Manual fine-grained PAT | No app registration; user controls exactly | Long-lived static secret; manual creation/rotation; requires "Administration: write" + Contents read; cannot silently refresh; worse UX |

## 6. Security/storage mapping (unchanged Ellie rules)

- Sign-in is Rust-owned; the WebView never handles the authorization code or token. The owner enters the App Client Secret into a masked Settings field once; it crosses only the main-window IPC save command and is immediately stored by Rust in Windows Credential Manager. IPC/status returns only `clientSecretConfigured`, never the value. The refresh token uses a separate Credential Manager identity. The 8-hour user access token lives in memory and is regenerated on demand.
- Never log tokens, client secrets, authorization headers, raw bodies, commit subjects, task notes, or private repository names. No browser/CLI credential scraping; no secret embedded in source, config, SQLite, fixtures, or the binary. Disconnect removes both the App Client Secret and refresh token while retaining the non-secret Client ID.
- Rate-limit and secondary-limit handling from verified limits above; conditional requests and bounded pagination per backend design.
- `127.0.0.1` redirect only; the callback accepts exactly one `code`, one `state`, and GitHub's optional RFC 9207 `iss` parameter only when it equals `https://github.com/login/oauth`. Unknown, duplicate, or mismatched parameters are rejected. The existing local API opt-in/token rules are unchanged and workspace data is not exposed to it.

## 7. Confirm at registration / live verification (W2)

1. Exact fine-grained permission row(s) GitHubs presents for `GET /user/repos` under a GitHub App.
2. Whether the created GitHub App must target the user account "all repositories" or can be granted per-repository, and whether that limits where personal commits are readable.
3. Whether refreshed tokens rotate the refresh token (GitHub's refresh endpoint behavior) so rotation storage is correct.
4. Webhook availability/cost for `github_app_authorization` revocation versus relying on 401s; choose the simpler verified signal.
5. Exact behavior when creating a second repository with the same name (conflict 422 message) and organization-owned creation refusal for this slice.
6. User access token rate-limit bucket confirmation during a live burst test.
7. Whether `is_owner` or concurrency quirks affect the proposed two-step create confirmation.

## 8. Owner decision (recorded)

The owner selected **option A: GitHub App + web application flow with PKCE and a `127.0.0.1` loopback redirect**. Approved alongside it: continue documentation before implementation, with W2 documentation prepared and no code or live repository test until a separate go-ahead.

The W2 follow-up documents the app-registration checklist, exact fine-grained permission request (Contents read + repository creation via `POST /user/repos`), consent copy, and the PKCE implementation notes needed before scaffolding.

## Appendix: research provenance

- Passages were read directly from official docs pages listed above; exact quotes are marked with quotes in section tables. The `source_check` pipeline's answer provider failed with a session routing error twice, so its structured artifact is empty; the claims were instead verified from directly fetched page content (see response ids in session artifacts for `fetch_content`/`get_search_content`).
- No contradiction was found in any retrieved official passage.
- Nothing in this document should be read as authorization to create the app, tokens, or repositories; that requires the owner decision below.