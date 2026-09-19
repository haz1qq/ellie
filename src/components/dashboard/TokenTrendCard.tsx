import { Area, AreaChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import type { AnalyticsResponse } from "../../lib/desktop";
import { analyticsSourceLabel, formatAnalyticsDate, formatCount } from "../../lib/format";
import { EmptyState } from "../ui/Panel";
import { useId } from "react";

const TOOLTIP_STYLE = {
  background: "#201f23",
  border: "1px solid #3a363f",
  borderRadius: 10,
  fontSize: 12,
  color: "#f2eee8",
};

/**
 * Real token trend chart (Recharts). Values come from local analytics only;
 * the source label accompanies every rendering.
 */
export function TokenTrendCard({ analytics }: { analytics: AnalyticsResponse | null }) {
  const id = useId();
  const data = (analytics?.tokenSeries ?? []).map((point) => ({
    date: formatAnalyticsDate(point.date),
    tokens: point.totalTokens,
  }));
  if (!analytics || analytics.snapshotCount === 0) {
    return (
      <div className="panel panel-chart">
        <EmptyState title="No token history yet">
          <p className="empty-state-text">
            Keep Ellie running to build a local token picture.
          </p>
        </EmptyState>
      </div>
    );
  }
  if (data.length === 0) {
    return (
      <div className="panel panel-chart">
        <EmptyState title="No token activity reported">
          <p className="empty-state-text">The selected history has no token rows.</p>
        </EmptyState>
      </div>
    );
  }
  const maxTokens = Math.max(1, ...data.map((point) => point.tokens));
  const yAxisWidth = Math.max(56, Math.min(92, formatCount(maxTokens).length * 8 + 12));
  return (
    <section className="panel panel-chart" aria-labelledby={`${id}-title`}>
      <header className="panel-header">
        <div className="panel-title">
          <span className="panel-kicker">Local analytics</span>
          <h2 className="panel-heading" id={`${id}-title`}>
            Token activity
          </h2>
        </div>
        <span className="panel-count">
          {analyticsSourceLabel(analytics.tokenSource)}
        </span>
      </header>
      <div className="chart-body" aria-label="Token activity over time">
        <ResponsiveContainer width="100%" height={150}>
          <AreaChart data={data} margin={{ top: 6, right: 8, bottom: 0, left: 4 }}>
            <defs>
              <linearGradient id={`${id}-fill`} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#eeaec4" stopOpacity={0.34} />
                <stop offset="100%" stopColor="#eeaec4" stopOpacity={0.02} />
              </linearGradient>
            </defs>
            <XAxis
              dataKey="date"
              tick={{ fill: "#a29ba5", fontSize: 10 }}
              axisLine={false}
              tickLine={false}
              minTickGap={26}
            />
            <YAxis
              domain={[0, maxTokens]}
              tick={{ fill: "#a29ba5", fontSize: 10 }}
              axisLine={false}
              tickLine={false}
              tickFormatter={(value: number) => formatCount(value)}
              width={yAxisWidth}
            />
            <Tooltip
              contentStyle={TOOLTIP_STYLE}
              formatter={(value) => [`${formatCount(Number(value))} tokens`, "Total"]}
              labelStyle={{ color: "#bcb6bf" }}
            />
            <Area
              type="monotone"
              dataKey="tokens"
              stroke="#eeaec4"
              strokeWidth={1.8}
              fill={`url(#${id}-fill)`}
              isAnimationActive={false}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </section>
  );
}

/** Latest provider-reported quota utilization as horizontal bars. */
export function QuotaUtilizationCard({ analytics }: { analytics: AnalyticsResponse | null }) {
  const windows = analytics?.quotaWindows ?? [];
  return (
    <section className="panel panel-chart" aria-label="Quota utilization">
      <header className="panel-header">
        <div className="panel-title">
          <span className="panel-kicker">Latest observation</span>
          <h2 className="panel-heading">Quota utilization</h2>
        </div>
      </header>
      {windows.length === 0 ? (
        <EmptyState title="No quota windows reported">
          <p className="empty-state-text">Providers did not report windows in range.</p>
        </EmptyState>
      ) : (
        <div className="quota-chart-list">
          {windows.map((window) => {
            const used = window.usedPercent == null ? null : Math.max(0, Math.min(100, window.usedPercent));
            return (
              <div className="quota-chart-row" key={`${window.providerId}-${window.windowId}`}>
                <div className="quota-chart-label">
                  <strong>{window.displayName}</strong>
                  <span>{window.windowLabel}</span>
                </div>
                <strong className="quota-chart-value">
                  {used == null ? "—" : `${Math.round(used)}% used`}
                </strong>
                <div
                  className="quota-analytics-track"
                  role="progressbar"
                  aria-label={`${window.displayName} ${window.windowLabel}`}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={used ?? undefined}
                >
                  <span style={{ width: `${used ?? 0}%` }} />
                </div>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}