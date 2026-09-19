import type { ReactNode } from "react";
import { cn } from "./cn";

type Tone = "neutral" | "good" | "warning" | "danger" | "accent";

const TONE_CLASSES: Record<Tone, string> = {
  neutral: "badge-neutral",
  good: "badge-good",
  warning: "badge-warning",
  danger: "badge-danger",
  accent: "badge-accent",
};

export function Badge({
  tone = "neutral",
  children,
  className,
}: {
  tone?: Tone;
  children: ReactNode;
  className?: string;
}) {
  return <span className={cn("badge", TONE_CLASSES[tone], className)}>{children}</span>;
}

export function PriorityBadge({ priority }: { priority: "none" | "low" | "medium" | "high" }) {
  const tone: Tone =
    priority === "high"
      ? "danger"
      : priority === "medium"
        ? "warning"
        : priority === "low"
          ? "neutral"
          : "neutral";
  if (priority === "none") return null;
  return (
    <Badge tone={tone} className="priority-badge">
      {priority === "high" ? "High" : priority === "medium" ? "Medium" : "Low"}
    </Badge>
  );
}