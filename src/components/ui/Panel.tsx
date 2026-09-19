import { Loader2 } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "./cn";

export function Spinner({ label, className }: { label?: string; className?: string }) {
  return (
    <span className={cn("spinner", className)} role="status">
      <Loader2 size={15} className="spinner-icon" aria-hidden="true" />
      {label ? <span>{label}</span> : <span className="visually-hidden">Loading</span>}
    </span>
  );
}

export function EmptyState({
  icon,
  title,
  children,
  className,
}: {
  icon?: ReactNode;
  title: string;
  children?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("empty-state", className)}>
      {icon && <span className="empty-state-icon">{icon}</span>}
      <div>
        <strong className="empty-state-title">{title}</strong>
        {children && <div className="empty-state-copy">{children}</div>}
      </div>
    </div>
  );
}

export function Panel({
  title,
  kicker,
  icon,
  actions,
  children,
  className,
  ariaLabel,
}: {
  title?: string;
  kicker?: string;
  icon?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  ariaLabel?: string;
}) {
  return (
    <section className={cn("panel", className)} aria-label={ariaLabel}>
      {(title || actions) && (
        <header className="panel-header">
          <div className="panel-title">
            {kicker && <span className="panel-kicker">{kicker}</span>}
            {title && (
              <h2 className="panel-heading">
                {icon}
                {title}
              </h2>
            )}
          </div>
          {actions && <div className="panel-actions">{actions}</div>}
        </header>
      )}
      {children}
    </section>
  );
}