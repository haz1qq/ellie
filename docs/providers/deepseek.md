# DeepSeek provider

Provider id: `deepseek`. Display name: `DeepSeek`.

## Source

Ellie reads the account balance through the **documented DeepSeek API**:

```
GET https://api.deepseek.com/user/balance
Authorization: Bearer ${DEEPSEEK_API_KEY}
Accept: application/json
```

Response:

```json
{
  "is_available": true,
  "balance_infos": [
    {
      "currency": "USD",
      "total_balance": "12.34",
      "granted_balance": "0.00",
      "topped_up_balance": "12.34"
    }
  ]
}
```

Request/Auth docs: api-docs.deepseek.com (`base_url: https://api.deepseek.com`,
API key from platform.deepseek.com, `Authorization: Bearer`).

## What is deliberately not shown

The milestone says: *"Do not display a 5-hour or weekly quota unless DeepSeek
actually exposes one."* DeepSeek is pay-as-you-go API billing; the official
docs expose **no quota windows, no usage-report endpoint, and no reset
timestamps**. Research found only the balance endpoint as a usage-relevant,
documented surface. Ellie therefore renders the **account balance with its
real currency** and the provider's `is_available` signal is carried only
implicitly (a `false` availability with a zero balance shows `0`); the
provider declares `quota_windows: false` and never fabricates windows,
credits, resets, token totals, or estimated cost.

Locally observed token consumption / estimated cost (listed as "potential
information" in the milestone) would require local capture of requests,
which Ellie does not perform; it is deferred to a later analytics milestone
and not invented.

## Authentication

- DeepSeek API keys (platform.deepseek.com) are stored in **Windows
  Credential Manager** via Settings → Provider credentials, or read from the
  `DEEPSEEK_API_KEY` environment variable (env wins). Keys are validated,
  never logged, and never echoed back through the UI or IPC; only the
  configured/not-configured status is exposed.
- `detect()` reports `authentication_required` when neither source has a
  key. Because unconfigured providers are hidden by the dashboard, a fresh
  install shows no DeepSeek card until a key is saved.

## Cost used (estimated)

DeepSeek exposes no usage/cost API, so Ellie computes an **estimate**: the
balance decrease between the oldest stored balance snapshot inside the
trailing 30 days and the current balance (`spendEstimate` on the snapshot).
Rules:

- needs two prior balance observations in the window (a single point is
  usually the initial top-up, not spend);
- only a *positive* decrease counts (top-ups raise the balance and produce
  no estimate);
- currency must match the current balance; `windowDays` reports the actual
  span covered;
- rendered as "≈ spent (last N days)" and labeled as an estimate — top-ups
  and granted-balance expiry can skew it. Never presented as official usage.

## Fields

| DeepSeek field | Ellie field | Notes |
| --- | --- | --- |
| `balance_infos[].currency` | `balanceCurrency` | ISO code; **USD preferred**, otherwise the first reported currency |
| `balance_infos[].total_balance` | `balance` | Decimal string parsed to `f64`; must be finite and ≥ 0 |
| `is_available` | — | Not surfaced directly; zero balance when false |
| `granted_balance` / `topped_up_balance` | — | Composition details not surfaced in v0.1 |
| balance history | `spendEstimate` | See “Cost used (estimated)” above; `None` until computable |

## Interpretation and provenance

- `balance` is provider-reported (the provider's own number); the dashboard
  formats it with the reported currency via `Intl.NumberFormat` — Ellie does
  not guess the currency.
- Missing/empty `balance_infos`, unparseable, or negative amounts become
  `balance: None` (unknown), never `0`; a genuine `"0.00"` stays `0`.
- `has_subscription` is `null` (API billing), so DeepSeek is never affected
  by the unsubscribed-hiding logic.

## Refresh and failures

- One bounded `GET /user/balance` per fetch (20 s client timeout, 60 s
  overall bound).
- `401` → `authentication_expired`; `403` → `authentication_required`;
  network/timeout/`429`/`5xx` → `unavailable`; `402` (payment-related) →
  `unavailable` so a depleted balance surfaces as a provider-fetch issue
  rather than a fabricated number.
- Registry isolation: a DeepSeek failure never disturbs the other cards.

## Limitations

- Balance and spend estimate only — no windows, usage reports, resets, or
  exact cost; the estimate is balance-derived with explicit caveats.
- No local token/cost accounting yet (requires local capture, deferred to
  analytics).
- An estimate needs the app to have collected at least two balance
  snapshots (app starts; polling in a later milestone will make it precise).