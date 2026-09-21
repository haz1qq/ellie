/* eslint-disable react-refresh/only-export-components */
import { RefreshCw, EyeOff } from "lucide-react";
import type { ProviderOverview, UsageSnapshot } from "../../lib/desktop";
import {
  hasLiveData,
  providersCount,
  usageDescription,
} from "../../lib/dashboard";
import { formatAge, formatBalance, formatCount, formatReset } from "../../lib/format";
import { ProviderMark } from "../providers/ProviderMark";
import { Badge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/Panel";

export interface ProviderSummaryPanelProps {
  providers: ProviderOverview[];
  visibleProviders: ProviderOverview[];
  native: boolean;
  refreshing: boolean;
  refreshingProvider: string | null;
  onRefreshAll: () => void;
  onRefreshProvider: (providerId: string) => void;
  onHideProvider: (providerId: string) => void;
  onOpenDetails: () => void;
  saving: boolean;
  settingsReady: boolean;
}

/**
 * Compact provider sheet for the Overview: per-window bars, balances, token
 * counts and provenance — the same facts as the detailed AI Usage page, denser.
 */
export function ProviderSummaryPanel({
  providers,
  visibleProviders,
  native,
  refreshing,
  refreshingProvider,
  onRefreshAll,
  onRefreshProvider,
  onHideProvider,
  onOpenDetails,
  saving,
  settingsReady,
}: ProviderSummaryPanelProps) {
  return (
    <section className="panel panel-providers" aria-label="AI usage summary">
      <header className="panel-header">
        <div className="panel-title">
          <span className="panel-kicker">Allowance monitor</span>
          <h2 className="panel-heading">AI usage</h2>
        </div>
        <div className="panel-actions">
          <span className="panel-count">{providersCount(providers, visibleProviders)}</span>
          <Button size="sm" variant="ghost" onClick={onOpenDetails}>
            Open details
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={onRefreshAll}
            disabled={!native || refreshing || refreshingProvider !== null}
          >
            <RefreshCw size={13} className={refreshing ? "spin" : ""} />
            {refreshing ? "Refreshing…" : "Refresh"}
          </Button>
        </div>
      </header>
      <p className="panel-description">{usageDescription(providers, visibleProviders)}</p>
      {providers.length === 0 ? (
        <EmptyState title="No provider data">
          <p className="empty-state-text">
            Provider connections are not available in this build.
          </p>
        </EmptyState>
      ) : visibleProviders.length === 0 ? (
        <EmptyState title="No visible provider cards">
          <p className="empty-state-text">
            Use Settings → Provider visibility to bring cards back. Unconfigured or
            unsubscribed providers stay hidden automatically.
          </p>
        </EmptyState>
      ) : (
        <div className="provider-sheet">
          {visibleProviders.map((provider) => (
            <CompactProviderRow
              key={provider.providerId}
              provider={provider}
              native={native}
              refreshing={refreshingProvider === provider.providerId}
              refreshDisabled={refreshing || refreshingProvider !== null}
              onRefresh={() => onRefreshProvider(provider.providerId)}
              hideDisabled={saving || !settingsReady || !native}
              onHide={() => onHideProvider(provider.providerId)}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function CompactProviderRow({
  provider,
  native,
  refreshing,
  refreshDisabled,
  onRefresh,
  hideDisabled,
  onHide,
}: {
  provider: ProviderOverview;
  native: boolean;
  refreshing: boolean;
  refreshDisabled: boolean;
  onRefresh: () => void;
  hideDisabled: boolean;
  onHide: () => void;
}) {
  const snapshot = provider.snapshot;
  const mock = snapshot?.dataKind === "mock";
  return (
    <article className="provider-row" aria-label={provider.displayName}>
      <header className="provider-row-header">
        <div className="provider-row-identity">
          <span className="provider-symbol" aria-hidden="true">
            <ProviderMark
              providerId={provider.providerId}
              displayName={snapshot ? snapshot.displayName : provider.displayName}
            />
          </span>
          <div>
            <h3 className="provider-row-name">{snapshot?.displayName ?? provider.displayName}</h3>
            <p className="provider-row-detail">
              {snapshot ? [snapshot.accountLabel, snapshot.plan].filter(Boolean).join(" · ") : provider.error?.replaceAll("_", " ")}
            </p>
          </div>
        </div>
        <div className="provider-row-actions">
          {mock && (
            <Badge tone="accent">Mock data</Badge>
          )}
          {provider.stale && <Badge tone="warning">Cached</Badge>}
          {provider.error && !snapshot && <Badge tone="danger">Unavailable</Badge>}
          <Button size="sm" variant="ghost" aria-label={`Refresh ${provider.displayName}`} disabled={refreshDisabled || !native} onClick={onRefresh}>
            <RefreshCw size={12} className={refreshing ? "spin" : ""} />
            {refreshing ? "…" : "Refresh"}
          </Button>
          <Button size="sm" variant="ghost" aria-label={`Hide ${provider.displayName}`} disabled={hideDisabled} onClick={onHide}>
            <EyeOff size={12} />
            <span className="visually-hidden">Hide</span>
          </Button>
        </div>
      </header>
      {!snapshot ? (
        <p className="provider-row-note">
          {provider.error === "authentication_required"
            ? "Authentication is required before this provider can report usage."
            : provider.error === "authentication_expired"
              ? "Authentication expired; reconnect this provider in Settings."
              : "The provider is temporarily unavailable. Existing data is unchanged."}
        </p>
      ) : (
        <div className="provider-row-body">
          {snapshot.capabilities.quotaWindows &&
            snapshot.windows.map((window) => <WindowBar key={window.id} window={window} dataKind={snapshot.dataKind} />)}
          {snapshot.balance !== null && (
            <div className="provider-balance">
              <span>Balance</span>
              <strong>{formatBalance(snapshot.balance, snapshot.balanceCurrency)}</strong>
              {snapshot.spendEstimate && (
                <span className="provider-balance-spend">
                  ≈ {formatBalance(snapshot.spendEstimate.amount, snapshot.spendEstimate.currency)} last{" "}
                  {snapshot.spendEstimate.windowDays}d
                </span>
              )}
            </div>
          )}
          {snapshot.capabilities.tokenUsage && snapshot.tokenUsage && (
            <p className="provider-token-note">
              {snapshot.tokenUsage.totalTokens == null
                ? "Token total unavailable"
                : `${formatCount(snapshot.tokenUsage.totalTokens)} tokens`}
              {snapshot.tokenUsage.requestCount != null
                ? ` · ${formatCount(snapshot.tokenUsage.requestCount)} requests`
                : ""}
              <span className="provider-token-source">
                {" "}
                · {mock ? "Locally calculated sample · illustrative only" : snapshot.tokenUsage.source === "locally_calculated" ? "calculated by Ellie" : "provider-reported"}
              </span>
            </p>
          )}
          <p className="provider-freshness">
            {snapshot.fetchedAt ? `Updated ${formatAge(snapshot.fetchedAt)} ago` : ""}
            {provider.stale && provider.lastSuccessfulRefresh
              ? ` · Showing data from ${formatAge(provider.lastSuccessfulRefresh)}${provider.error ? " · refresh failed" : ""}`
              : ""}
          </p>
        </div>
      )}
    </article>
  );
}

function WindowBar({
  window,
  dataKind,
}: {
  window: UsageSnapshot["windows"][number];
  dataKind: UsageSnapshot["dataKind"];
}) {
  const used = window.usedPercent;
  const remaining = window.remainingPercent;
  const mock = dataKind === "mock";
  return (
    <div className="window-bar">
      <div className="window-bar-top">
        <span>
          <strong>{window.label}</strong>
          <span className="window-bar-source">
            {" "}
            · {mock ? "Demo data" : window.source === "provider_reported" ? "Provider-reported" : "Estimated"}
          </span>
        </span>
        <span>
          {used !== null ? `${used}% used` : "—"}
          <span className="window-bar-remaining">
            {" "}· {remaining ?? "—"}% remaining
          </span>
        </span>
      </div>
      {used !== null && (
        <div
          className="usage-track"
          role="progressbar"
          aria-label={`${window.label}: ${used}% used, ${mock ? "illustrative data" : "provider data"}`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={used}
        >
          <span style={{ width: `${Math.max(0, Math.min(100, used))}%` }} />
        </div>
      )}
      <p className="window-bar-reset">
        {mock ? "Illustrative provider-reported sample · no connected account · Sample reset;" : formatReset(window.resetAt, dataKind)}
      </p>
    </div>
  );
}

export function hasLiveSummary(providers: ProviderOverview[]): boolean {
  return hasLiveData(providers);
}