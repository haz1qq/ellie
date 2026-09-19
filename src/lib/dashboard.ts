import type { ProviderOverview } from "./desktop";
import { formatRelativeAge, formatReset } from "./format";

export interface DashboardAttention {
  key: string;
  source: "provider" | "github";
  tone: "warning" | "error";
  title: string;
  detail: string;
}

export function providerErrorLabel(
  error: NonNullable<ProviderOverview["error"]>,
): string {
  switch (error) {
    case "authentication_required":
      return "Authentication is required.";
    case "authentication_expired":
      return "Authentication expired; reconnect this provider.";
    case "invalid_snapshot":
      return "The provider returned data Ellie could not validate.";
    case "unavailable":
      return "The provider is temporarily unavailable.";
  }
}

/** Lowest live, provider-reported remaining allowance across visible providers. */
export function lowestReportedAllowance(providers: ProviderOverview[]): {
  provider: string;
  window: string;
  remaining: number;
  stale: boolean;
} | null {
  const candidates = providers.flatMap((provider) =>
    provider.snapshot?.dataKind === "live"
      ? provider.snapshot.windows
          .filter(
            (window) =>
              window.source === "provider_reported" &&
              window.remainingPercent !== null,
          )
          .map((window) => ({
            provider: provider.displayName,
            window: window.label,
            remaining: window.remainingPercent!,
            stale: provider.stale === true,
          }))
      : [],
  );
  return candidates.sort((left, right) => left.remaining - right.remaining)[0] ?? null;
}

export function liveProviderCount(providers: ProviderOverview[]): number {
  return providers.filter(
    (provider) => provider.snapshot?.dataKind === "live" && !provider.stale,
  ).length;
}

/** Newest provider snapshot or last successful refresh timestamp. */
export function latestProviderUpdate(providers: ProviderOverview[]): string | null {
  const timestamps = providers
    .map(
      (provider) =>
        provider.snapshot?.fetchedAt ?? provider.lastSuccessfulRefresh ?? null,
    )
    .filter((value): value is string => value !== null)
    .filter((value) => !Number.isNaN(new Date(value).getTime()));
  return timestamps.sort(
    (left, right) => new Date(right).getTime() - new Date(left).getTime(),
  )[0] ?? null;
}

/** Providers with a snapshot, newest first, capped at `limit`. */
export function recentProviders(
  providers: ProviderOverview[],
  limit = 3,
): ProviderOverview[] {
  return [...providers]
    .filter((provider) => provider.snapshot)
    .sort(
      (left, right) =>
        new Date(right.snapshot!.fetchedAt).getTime() -
        new Date(left.snapshot!.fetchedAt).getTime(),
    )
    .slice(0, limit);
}

export function dashboardAttentionItems(
  providers: ProviderOverview[],
  githubError: string,
): DashboardAttention[] {
  const items: DashboardAttention[] = [];
  for (const provider of providers) {
    if (provider.error) {
      items.push({
        key: `${provider.providerId}-error`,
        source: "provider",
        tone: "error",
        title: `${provider.displayName} refresh issue`,
        detail: providerErrorLabel(provider.error),
      });
    } else if (provider.stale) {
      items.push({
        key: `${provider.providerId}-stale`,
        source: "provider",
        tone: "warning",
        title: `${provider.displayName} is showing cached data`,
        detail: provider.lastSuccessfulRefresh
          ? `Last success ${formatRelativeAge(provider.lastSuccessfulRefresh)}.`
          : "The last successful refresh time is unavailable.",
      });
    }
    if (provider.snapshot?.dataKind !== "live") continue;
    for (const window of provider.snapshot.windows) {
      if (
        window.source === "provider_reported" &&
        window.remainingPercent !== null &&
        window.remainingPercent <= 20
      ) {
        items.push({
          key: `${provider.providerId}-${window.id}-low`,
          source: "provider",
          tone: window.remainingPercent <= 10 ? "error" : "warning",
          title: `${provider.displayName} · ${window.label}`,
          detail: `${window.remainingPercent}% remaining · ${formatReset(
            window.resetAt,
            provider.snapshot.dataKind,
          )}`,
        });
      }
    }
  }
  if (githubError) {
    items.push({
      key: "github-error",
      source: "github",
      tone: "error",
      title: "GitHub connection",
      detail: githubError,
    });
  }
  return items;
}

export function hasLiveData(providers: ProviderOverview[]): boolean {
  return providers.some((provider) => provider.snapshot?.dataKind === "live");
}

export function providersCount(
  providers: ProviderOverview[],
  visible: ProviderOverview[],
): string {
  if (providers.length === 0) return "0 connected";
  const hidden = providers.length - visible.length;
  return hidden > 0
    ? `${visible.length} shown · ${hidden} hidden`
    : `${visible.length} shown`;
}

export function usageDescription(
  providers: ProviderOverview[],
  visible: ProviderOverview[],
): string {
  if (providers.length === 0) {
    return "Provider connections are not available in this build.";
  }
  if (visible.length === 0) {
    return "No visible providers. Check Provider visibility in Settings; unconfigured or unsubscribed providers also stay hidden.";
  }
  return hasLiveData(visible)
    ? "Cards show live quota from your configured logins; demo cards stay labeled."
    : "Demo providers exercise Ellie's display and are not account data.";
}

export function usageNote(
  providers: ProviderOverview[],
  visible: ProviderOverview[],
): string {
  if (providers.length === 0) {
    return "No provider requests.";
  }
  if (visible.length === 0) {
    return "Show hidden cards in Settings → Provider visibility. No credentials or history are deleted.";
  }
  return hasLiveData(visible)
    ? "Live data comes from configured provider connections on this device. Keys remain local."
    : "No provider requests. Demo data is illustrative only.";
}

/** Aggregate open/completed counts for a task collection. */
export function taskCounts(tasks: Array<{ completedAt: string | null }>): {
  open: number;
  completed: number;
} {
  let open = 0;
  let completed = 0;
  for (const task of tasks) {
    if (task.completedAt) completed += 1;
    else open += 1;
  }
  return { open, completed };
}

/** Incomplete tasks with a due date, soonest first, capped at `limit`. */
export function upcomingTasks(
  tasks: Array<{ completedAt: string | null; dueDate: string | null; id: number }>,
  limit = 4,
): Array<{ id: number; dueDate: string }> {
  return tasks
    .filter(
      (task): task is typeof task & { dueDate: string } =>
        !task.completedAt && task.dueDate !== null,
    )
    .filter((task) => task.dueDate.length === 10)
    .sort((left, right) => left.dueDate.localeCompare(right.dueDate))
    .slice(0, limit)
    .map((task) => ({ id: task.id, dueDate: task.dueDate }));
}