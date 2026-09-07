# OpenAI API billing provider

Provider id: `openai-api`. Display name: `OpenAI API`.

This is intentionally separate from [OpenAI / Codex](openai.md). It reports
separately billed OpenAI API completion activity, not ChatGPT subscription
usage, Codex rate limits, or Codex activity.

## Source and authentication

Ellie calls the documented OpenAI Organization Usage endpoint:

```text
GET https://api.openai.com/v1/organization/usage/completions
```

It requests a trailing 30-day period with one-day buckets, groups results by
model, and follows pagination up to four pages. The endpoint requires an
**OpenAI Admin API key**. An ordinary OpenAI API key does not grant this
organization-administration access.

Set `OPENAI_ADMIN_KEY` before launching Ellie, or save an Admin API key in
**Settings → Provider credentials → OpenAI API**. Environment keys take
precedence over Windows Credential Manager. Saved keys use the `ellie` Windows
Credential Manager service and `openai_admin_api_key` account name. Ellie
never returns, logs, stores in SQLite, or exports the key.

## Fields and interpretation

| OpenAI field | Ellie field | Interpretation |
| --- | --- | --- |
| `input_tokens` | input tokens | Provider-reported in each usage result |
| `output_tokens` | output tokens | Provider-reported in each usage result |
| `input_cached_tokens` | cached input | Provider-reported subset of input tokens |
| `num_model_requests` | requests | Provider-reported count |
| `model` | model | The model with the most input + output tokens across Ellie's selected window, when provided |

Ellie sums the provider-reported daily result rows across its trailing 30-day
window. The displayed aggregate has `source: locally_calculated` because Ellie
chooses and sums the window; it is never represented as an official quota or
as a ChatGPT/Codex subscription measurement. The endpoint does not supply an
account balance, reset timestamp, or a cost total, so Ellie does not invent
those values.

## Failures and limitations

- Missing configuration returns `authentication_required` and stays hidden
  until configured, like other unconfigured providers.
- HTTP 401 maps to `authentication_expired`.
- HTTP 403 (for example, an ordinary key or inadequate Admin role) remains a
  visible `unavailable` card rather than being mistaken for an unconfigured
  provider.
- Rate limits, malformed responses, network failures, pagination beyond four
  pages, and request timeouts are `unavailable`; partial pages are never shown.
- `input_tokens`, `output_tokens`, `input_cached_tokens`, and
  `num_model_requests` must be nonnegative integer values. Missing, negative,
  or malformed values reject the report instead of being replaced with zero.
- Token/cost data from ChatGPT web usage and Codex subscription quotas are not
  API billing data and are not combined into this provider.

## Verification

Hermetic tests cover token aggregation, malformed/negative data rejection,
status classification, and bounded pagination against a local server. No live
OpenAI Admin API key was used during implementation, so real account access
and response shape still require an owner-provided Admin key smoke test.
