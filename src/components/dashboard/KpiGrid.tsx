/* eslint-disable react-refresh/only-export-components */
import { Activity, CircleDollarSign, GitFork, TimerReset, CheckCircle2 } from "lucide-react";
import type { ReactNode } from "react";
import type { ProviderOverview, View } from "../../lib/desktop";
import { liveProviderCount, lowestReportedAllowance, taskCounts } from "../../lib/dashboard";
import { formatRelativeAge } from "../../lib/format";
import { cn } from "../ui/cn";

type Tone = "default" | "good" | "attention" | "accent";

export interface KpiDefinition {
  key: string;
  label: string;
  value: string;
  detail: string;
  tone: Tone;
  icon: ReactNode;
  actionLabel?: string;
  actionView?: View;
}

export function buildKpis(
  visibleProviders: ProviderOverview[],
  githubState: { label: string; connected: boolean; detail: string },
  tasks: Array<{ completedAt: string | null }>,
  latestUpdate: string | null,
  lowestAllowance: ReturnType<typeof lowestReportedAllowance>,
): KpiDefinition[] {
  const live = liveProviderCount(visibleProviders);
  const { open, completed } = taskCounts(tasks);
  return [
    {
      key: "allowance",
      label: "AI allowance",
      value: lowestAllowance ? `${Math.round(lowestAllowance.remaining)}%` : "Unavailable",
      detail: lowestAllowance
        ? `${lowestAllowance.provider} · ${lowestAllowance.window} remaining · provider-reported${lowestAllowance.stale ? " · cached" : ""}`
        : visibleProviders.length === 0
          ? "No visible provider data"
          : "No provider-reported allowance",
      tone: lowestAllowance && lowestAllowance.remaining <= 20 ? "attention" : "default",
      icon: <CircleDollarSign size={15} />,
    },
    {
      key: "providers",
      label: "Provider health",
      value: `${live}/${visibleProviders.length}`,
      detail:
        visibleProviders.length === 0
          ? "Nothing shown on the dashboard"
          : `${live} live of ${visibleProviders.length} visible`,
      tone: visibleProviders.length > 0 && live === visibleProviders.length ? "good" : "default",
      icon: <Activity size={15} />,
    },
    {
      key: "github",
      label: "GitHub",
      value: githubState.label,
      detail: githubState.detail,
      tone: githubState.connected ? "good" : "default",
      icon: <GitFork size={15} />,
      actionLabel: "Open GitHub",
      actionView: "github",
    },
    {
      key: "tasks",
      label: "Open tasks",
      value: String(open),
      detail: completed > 0 ? `${completed} completed locally` : "No completed tasks yet",
      tone: open === 0 ? "good" : "default",
      icon: <CheckCircle2 size={15} />,
      actionLabel: "Open To-do",
      actionView: "todos",
    },
    {
      key: "freshness",
      label: "Last AI update",
      value: latestUpdate ? formatRelativeAge(latestUpdate) : "Unavailable",
      detail: latestUpdate ? "Newest visible provider snapshot" : "No successful provider snapshot",
      tone: latestUpdate ? "default" : "attention",
      icon: <TimerReset size={15} />,
    },
  ];
}

export function KpiGrid({
  kpis,
  onNavigate,
}: {
  kpis: KpiDefinition[];
  onNavigate: (view: View) => void;
}) {
  return (
    <div className="kpi-grid" aria-label="Workspace summary">
      {kpis.map((kpi) => (
        <article
          key={kpi.key}
          className={cn("kpi", kpi.tone !== "default" && `kpi-${kpi.tone}`)}
        >
          <header className="kpi-header">
            <span className="kpi-icon" aria-hidden="true">
              {kpi.icon}
            </span>
            <span className="kpi-label">{kpi.label}</span>
          </header>
          <strong className="kpi-value">{kpi.value}</strong>
          <p className="kpi-detail">{kpi.detail}</p>
          {kpi.actionLabel && kpi.actionView && (
            <button
              type="button"
              className="kpi-link"
              onClick={() => onNavigate(kpi.actionView!)}
            >
              {kpi.actionLabel}
            </button>
          )}
        </article>
      ))}
    </div>
  );
}