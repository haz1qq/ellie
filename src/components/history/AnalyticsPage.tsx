import { BarChart3 } from "lucide-react";
import type { AnalyticsRange, AnalyticsResponse } from "../../lib/desktop";
import { analyticsSourceLabel, formatBalance, formatCount } from "../../lib/format";
import { useAnalytics } from "../../lib/useAnalytics";
import { TokenTrendCard, QuotaUtilizationCard } from "../dashboard/TokenTrendCard";
import { EmptyState, Spinner } from "../ui/Panel";

const RANGE_OPTIONS: Array<{ value: AnalyticsRange; label: string }> = [
  { value: "today", label: "Today" },
  { value: "sevenDays", label: "7 days" },
  { value: "thirtyDays", label: "30 days" },
  { value: "ninetyDays", label: "90 days" },
];

export interface AnalyticsPageProps {
  native: boolean;
  range: AnalyticsRange;
  onRangeChange: (range: AnalyticsRange) => void;
}

/** History & insights — local analytics with real charts. */
export function AnalyticsPage({ native, range, onRangeChange }: AnalyticsPageProps) {
  const { analytics, loading, error } = useAnalytics(native, range, true);

  return (
    <div className="history-page">
      <div className="todo-toolbar">
        <div className="todo-toolbar-title">
          <p className="eyebrow">Local analytics</p>
          <h1 className="page-title">History</h1>
          <p className="page-sub">Kept on this device; never uploaded.</p>
        </div>
        <div className="todo-toolbar-actions">
          <div className="segmented" role="group" aria-label="Analytics range">
            {RANGE_OPTIONS.map((option) => (
              <button
                key={option.value}
                type="button"
                className={range === option.value ? "segmented-active" : ""}
                aria-pressed={range === option.value}
                disabled={!native || loading}
                onClick={() => onRangeChange(option.value)}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      {!native ? (
        <EmptyState title="History needs the desktop app">
          <p className="empty-state-text">
            Browser preview does not read local analytics.
          </p>
        </EmptyState>
      ) : loading && !analytics ? (
        <Spinner label="Loading local history…" />
      ) : error ? (
        <EmptyState title="History unavailable" className="empty-state-error">
          <p className="empty-state-text" role="alert">
            {error}
          </p>
        </EmptyState>
      ) : !analytics || analytics.snapshotCount === 0 ? (
        <EmptyState icon={<BarChart3 size={18} />} title="No live history in this range yet">
          <p className="empty-state-text">
            Keep Ellie running to build a local picture.
          </p>
        </EmptyState>
      ) : (
        <div className="history-layout">
          <SummaryGrid analytics={analytics} />
          <TokenTrendCard analytics={analytics} />
          <QuotaUtilizationCard analytics={analytics} />
          <p className="page-footnote">
            Analytics are calculated from fresh live snapshots. Repeated refreshes
            are not added together; estimates and provider-reported windows retain
            their original meaning.
          </p>
        </div>
      )}
    </div>
  );
}

function SummaryGrid({ analytics }: { analytics: AnalyticsResponse }) {
  return (
    <div className="analytics-summary">
      <Metric
        label="Latest tracked tokens"
        value={formatCount(analytics.latestTotalTokens)}
        detail={`Token metrics · ${analyticsSourceLabel(analytics.tokenSource)}`}
      />
      <Metric
        label="Latest request count"
        value={
          analytics.latestRequestCount == null
            ? "Not available"
            : formatCount(analytics.latestRequestCount)
        }
        detail={
          analytics.latestRequestCount == null
            ? "No request counts in the selected history"
            : `Request metrics · ${analyticsSourceLabel(analytics.tokenSource)}`
        }
      />
      <Metric
        label="Estimated spend"
        value={
          analytics.estimatedSpend.length === 0
            ? "Not available"
            : analytics.estimatedSpend
                .map((spend) => formatBalance(spend.amount, spend.currency))
                .join(" · ")
        }
        detail={
          analytics.estimatedSpend.length === 0
            ? "No cost estimates in the selected history"
            : "Latest estimates, not a bill for this range"
        }
      />
      <Metric
        label="Observed history"
        value={formatCount(analytics.snapshotCount)}
        detail={`${formatCount(analytics.providerCount)} live provider${analytics.providerCount === 1 ? "" : "s"}`}
      />
    </div>
  );
}

function Metric({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div className="analytics-metric">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </div>
  );
}