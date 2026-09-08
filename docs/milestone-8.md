# Milestone 8 — Local API

Status: complete on `feat/local-api`. Native API smoke verification passed.

## Scope

Ellie exposes a versioned Axum API for local integrations such as Pi. The
server binds only to `127.0.0.1:9876`. Every route requires an
`Authorization: Bearer <ELLIE_API_TOKEN>` header; the token is supplied through
the Ellie process environment and is never stored in SQLite, returned by Ellie,
or logged.

## Routes

```text
GET  /api/v1/health
GET  /api/v1/providers
GET  /api/v1/usage
GET  /api/v1/providers/:id/usage
POST /api/v1/refresh
```

`/providers` and `/usage` return the normalized provider view with provider
identity, plan/account metadata, capabilities, quota windows, token activity,
balances, reset metadata, stale state, retry metadata, and typed errors where
available. The provider-specific route returns one normalized provider or a
404. `/refresh` uses the existing refresh coordinator and therefore shares
serialization, stale fallback, backoff bypass, history persistence, and
notification evaluation with desktop refresh paths.

## Security

The server has no permissive CORS layer and does not accept tokens in query
parameters or request bodies. Missing configuration returns service unavailable;
missing or invalid bearer headers return unauthorized. API responses never
include provider API keys, Codex credentials, cookies, authorization headers,
or raw provider response bodies.

Example PowerShell setup for a development session:

```powershell
$bytes = New-Object byte[] 32
[Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
$env:ELLIE_API_TOKEN = [Convert]::ToBase64String($bytes)
npm run tauri dev
```

Pi or another local client must inherit the same environment variable and send
it as the bearer token. The token is process configuration, not an Ellie
setting; users should avoid committing or broadly persisting it.

## Verification

Rust tests cover bearer-header parsing, constant-time token comparison, and
normalized response conversion. Native smoke verification passed: the server
bound to loopback, rejected invalid tokens, returned health and usage data with
a valid token, returned 404 for an unknown provider, and routed an
authenticated POST refresh through the existing coordinator without exposing
secrets.

## Deferred

History, settings, and other expanded API routes remain future enhancements.
Pi integration remains separate from Ellie core and consumes this API as a
client.
