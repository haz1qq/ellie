import type { UsageSnapshot } from "./desktop";

/** Formatted count without units; "—" for missing values. */
export function formatCount(value: number | null | undefined): string {
  return value == null ? "—" : new Intl.NumberFormat().format(value);
}

/** Human age string like "3 minutes", "2 hours", "5 days"; never "0". */
export function formatAge(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "an unknown time";
  const elapsed = Math.max(0, Date.now() - date.getTime());
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} minute${minutes === 1 ? "" : "s"}`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} hour${hours === 1 ? "" : "s"}`;
  const days = Math.floor(hours / 24);
  return `${days} day${days === 1 ? "" : "s"}`;
}

/** "Just now" / "3 minutes ago" style label. */
export function formatRelativeAge(value: string): string {
  const age = formatAge(value);
  return age === "just now" ? "Just now" : `${age} ago`;
}

export function formatBalance(value: number, currency: string | null): string {
  const amount = value.toLocaleString(undefined, { maximumFractionDigits: 2 });
  if (!currency) return amount;
  try {
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency,
    }).format(value);
  } catch {
    return `${currency} ${amount}`;
  }
}

export function formatReset(
  value: string | null,
  dataKind: UsageSnapshot["dataKind"],
): string {
  if (!value) return "Reset unavailable";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Reset unavailable";
  const localDateTime = date.toLocaleString([], {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
  return dataKind === "mock" ? `Sample reset ${localDateTime}` : `Resets ${localDateTime}`;
}

/** "Jan 5"-style label for a YYYY-MM-DD analytics date. */
export function formatAnalyticsDate(value: string): string {
  const date = new Date(`${value}T00:00:00Z`);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString([], { month: "short", day: "numeric" });
}

/**
 * Local calendar date label for a YYYY-MM-DD due date. "Today"/"Tomorrow"
 * for immediate dates; always returns text alongside any tone color.
 */
export function formatDueDate(value: string | null): string {
  if (!value) return "No due date";
  const date = new Date(`${value}T00:00:00Z`);
  if (Number.isNaN(date.getTime())) return value;
  const today = localCalendarDate(new Date());
  if (value === today) return "Due today";
  const tomorrow = addCalendarDays(new Date(), 1);
  if (value === localCalendarDate(tomorrow)) return "Due tomorrow";
  return date.toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" });
}

/** YYYY-MM-DD in the local calendar (matching the validated task field). */
export function localCalendarDate(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function addCalendarDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

/** An incomplete task is overdue when its due date precedes today's local date. */
export function isOverdue(dueDate: string | null, completedAt: string | null): boolean {
  if (!dueDate || completedAt) return false;
  return dueDate < localCalendarDate(new Date());
}

/** Short stable abbreviation for a commit SHA. */
export function shortSha(value: string): string {
  return value.length > 7 ? value.slice(0, 7) : value;
}

export function headerDetail(snapshot: UsageSnapshot): string {
  return [snapshot.accountLabel, snapshot.plan].filter(Boolean).join(" · ");
}

export function analyticsSourceLabel(
  source: "provider_reported" | "locally_calculated" | "mixed" | null,
): string {
  switch (source) {
    case "provider_reported":
      return "provider-reported";
    case "locally_calculated":
      return "Ellie-calculated estimate";
    case "mixed":
      return "mixed sources";
    default:
      return "source unavailable";
  }
}