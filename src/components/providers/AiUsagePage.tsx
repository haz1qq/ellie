import { EyeOff, RefreshCw } from "lucide-react";
import type { ProviderOverview, UsageSnapshot } from "../../lib/desktop";
import { providersCount, usageNote } from "../../lib/dashboard";
import { formatAge, formatBalance, formatCount, formatReset, headerDetail } from "../../lib/format";
import { ProviderMark } from "./ProviderMark";
import { Badge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/Panel";

export interface AiUsagePageProps {
  providers: ProviderOverview[];
  visibleProviders: ProviderOverview[];
  native: boolean;
  saving: boolean;
  settingsReady: boolean;
  refreshing: boolean;
  refreshingProvider: string | null;
  onRefreshAll: () => void;
  onRefreshProvider: (providerId: string) => void;
  onHideProvider: (providerId: string) => void;
  onHideToTray: () => void;
}

/** Detailed AI usage page — the full provider cards and provenance labels. */
export function AiUsagePage({
  providers,
  visibleProviders,
  native,
  saving,
  settingsReady,
  refreshing,
  refreshingProvider,
  onRefreshAll,
  onRefreshProvider,
  onHideProvider,
  onHideToTray,
}: AiUsagePageProps) {
  return (
    <div className="ai-page">
      <div className="todo-toolbar">
        <div className="todo-toolbar-title">
          <p className="eyebrow">Allowances & tokens</p>
          <h1 className="page-title">AI Usage</h1>
          <p className="page-sub">
            {providersCount(providers, visibleProviders)} ·{" "}
            {usageNote(providers, visibleProviders)}
          </p>
        </div>
        <div className="todo-toolbar-actions">
          <Button
            variant="primary"
            onClick={onRefreshAll}
            disabled={!native || refreshing || refreshingProvider !== null}
          >
            <RefreshCw size={14} className={refreshing ? "spin" : ""} />
            {refreshing ? "Refreshing…" : "Refresh all"}
          </Button>
          <Button variant="ghost" onClick={onHideToTray} disabled={!native}>
            Hide to tray
          </Button>
        </div>
      </div>

      {providers.length === 0 ? (
        <EmptyState title="No provider data">
          <p className="empty-state-text">
            Provider connections are not available in this build.
          </p>
        </EmptyState>
      ) : (
        <div className="providers-grid">
          {visibleProviders.map((provider) => (
            <ProviderCard
              key={provider.providerId}
              provider={provider}
              onHide={() => onHideProvider(provider.providerId)}
              hideDisabled={saving || !settingsReady || !native || !provider.providerId}
              onRefresh={() => onRefreshProvider(provider.providerId)}
              refreshDisabled={refreshing || refreshingProvider !== null || !native}
              refreshing={refreshingProvider === provider.providerId}
            />
          ))}
          {visibleProviders.length === 0 && (
            <EmptyState title="No visible provider cards">
              <p className="empty-state-text">
                Use Settings → Provider visibility to bring cards back. Unconfigured
                or unsubscribed providers stay hidden automatically.
              </p>
            </EmptyState>
          )}
        </div>
      )}
      <p className="page-footnote">
        Live data comes from configured provider connections on this device. Keys
        remain in Windows Credential Manager and never leave the machine.
      </p>
    </div>
  );
}

function ProviderCard({
  provider,
  onHide,
  hideDisabled,
  onRefresh,
  refreshDisabled,
  refreshing,
}: {
  provider: ProviderOverview;
  onHide: () => void;
  hideDisabled: boolean;
  onRefresh: () => void;
  refreshDisabled: boolean;
  refreshing: boolean;
}) {
  const snapshot = provider.snapshot;
  if (!snapshot) {
    return (
      <article className="provider-card provider-card-error">
        <header className="provider-card-header">
          <div className="provider-identity">
            <span className="provider-symbol" aria-hidden="true">
              <ProviderMark providerId={provider.providerId} displayName={provider.displayName} />
            </span>
            <div>
              <h3>{provider.displayName}</h3>
              <p>Provider unavailable</p>
            </div>
          </div>
          <Badge tone="danger">{provider.error?.replaceAll("_", " ")}</Badge>
        </header>
        <p className="provider-card-note">
          Ellie kept other provider results available; this one needs attention in
          Settings.
        </p>
        <div className="provider-actions">
          <Button size="sm" variant="ghost" onClick={onRefresh} disabled={refreshDisabled}>
            <RefreshCw size={12} />
            Refresh
          </Button>
          <Button size="sm" variant="ghost" onClick={onHide} disabled={hideDisabled}>
            <EyeOff size={12} />
            Hide
          </Button>
        </div>
      </article>
    );
  }
  return (
    <article className="provider-card">
      <header className="provider-card-header">
        <div className="provider-identity">
          <span className="provider-symbol" aria-hidden="true">
            <ProviderMark providerId={provider.providerId} displayName={snapshot.displayName} />
          </span>
          <div>
            <h3>{snapshot.displayName}</h3>
            <p className="provider-identity-detail">{headerDetail(snapshot)}</p>
            {snapshot.model && <p className="provider-identity-model">Model: {snapshot.model}</p>}
            <RefreshStatus provider={provider} />
          </div>
        </div>
        {snapshot.dataKind === "mock" ? (
          <Badge tone="accent">Mock data</Badge>
        ) : provider.stale ? (
          <Badge tone="warning">Cached</Badge>
        ) : (
          <Badge tone="good">Live</Badge>
        )}
      </header>
      {snapshot.capabilities.quotaWindows &&
        snapshot.windows.map((window) => (
          <UsageWindowCard key={window.id} window={window} dataKind={snapshot.dataKind} />
        ))}
      {snapshot.balance !== null && (
        <div className="balance-summary">
          <span>Account balance</span>
          <span>{formatBalance(snapshot.balance, snapshot.balanceCurrency)}</span>
          {snapshot.spendEstimate && (
            <>
              <span>≈ spent (last {snapshot.spendEstimate.windowDays} days)</span>
              <span>
                {formatBalance(snapshot.spendEstimate.amount, snapshot.spendEstimate.currency)}
              </span>
              <small>
                Estimated by Ellie from reported balance changes; top-ups can skew it.
              </small>
            </>
          )}
        </div>
      )}
      {snapshot.capabilities.tokenUsage && snapshot.tokenUsage && (
        <TokenSummaryCard snapshot={snapshot} />
      )}
      <div className="provider-actions">
        <Button size="sm" variant="ghost" onClick={onRefresh} disabled={refreshDisabled}>
          <RefreshCw size={12} className={refreshing ? "spin" : ""} />
          {refreshing ? "Refreshing…" : "Refresh"}
        </Button>
        <Button size="sm" variant="ghost" onClick={onHide} disabled={hideDisabled}>
          <EyeOff size={12} />
          Hide
        </Button>
      </div>
    </article>
  );
}

function RefreshStatus({ provider }: { provider: ProviderOverview }) {
  if (provider.stale && provider.lastSuccessfulRefresh) {
    return (
      <span className="provider-stale" role="status">
        Showing data from {formatAge(provider.lastSuccessfulRefresh)}
        {provider.error ? " · refresh failed" : ""}
      </span>
    );
  }
  if (provider.error && !provider.snapshot) {
    return <span className="provider-stale">Refresh unavailable</span>;
  }
  if (provider.snapshot) {
    return (
      <span className="provider-fresh">Updated {formatAge(provider.snapshot.fetchedAt)} ago</span>
    );
  }
  return null;
}

function TokenSummaryCard({ snapshot }: { snapshot: UsageSnapshot }) {
  const tokens = snapshot.tokenUsage!;
  const mock = snapshot.dataKind === "mock";
  const breaksDown = tokens.inputTokens != null && tokens.outputTokens != null;
  return (
    <div className="token-summary">
      <span>{mock ? "Sample token activity" : "Token activity (last 30 days)"}</span>
      <span>
        {tokens.totalTokens == null
          ? "Token total unavailable"
          : `${formatCount(tokens.totalTokens)} tokens`}
        {breaksDown
          ? ` · ${formatCount(tokens.inputTokens)} input / ${formatCount(tokens.outputTokens)} output`
          : tokens.requestCount != null
            ? ` · ${formatCount(tokens.requestCount)} requests`
            : ""}
      </span>
      <small>
        {tokens.estimatedCostUsd != null && `${formatBalance(tokens.estimatedCostUsd, "USD")} · `}
        {tokens.cachedInputTokens != null &&
          `${formatCount(tokens.cachedInputTokens)} cached input · `}
        {mock
          ? "Locally calculated sample · illustrative only"
          : tokens.source === "locally_calculated"
            ? "Calculated by Ellie from provider activity · not a quota"
            : "Provider-reported"}
      </small>
    </div>
  );
}

function UsageWindowCard({
  window,
  dataKind,
}: {
  window: UsageSnapshot["windows"][number];
  dataKind: UsageSnapshot["dataKind"];
}) {
  const used = window.usedPercent;
  const remaining = window.remainingPercent;
  return (
    <div className="usage-window">
      <div className="usage-window-title">
        <strong>{window.label}</strong>
        <span>
          {dataKind === "mock"
            ? "Demo data"
            : window.source === "provider_reported"
              ? "Provider-reported"
              : "Estimated"}
        </span>
      </div>
      {used !== null && (
        <>
          <div
            className="usage-track"
            role="progressbar"
            aria-label={`${window.label}: ${used}% used, ${dataKind === "mock" ? "illustrative data" : "provider data"}`}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={used}
          >
            <span style={{ width: `${used}%` }} />
          </div>
          <div className="usage-summary">
            <span>{used}% used</span>
            <span>
              {remaining ?? "—"}% remaining · {formatReset(window.resetAt, dataKind)}
            </span>
          </div>
        </>
      )}
      <small>
        {dataKind === "mock"
          ? "Illustrative provider-reported sample · no connected account"
          : "Provider-reported quota · remaining and reset are derived by Ellie"}
      </small>
    </div>
  );
}