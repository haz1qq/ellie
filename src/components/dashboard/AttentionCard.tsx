import { AlertTriangle, CheckCircle2 } from "lucide-react";
import type { DashboardAttention } from "../../lib/dashboard";
import { cn } from "../ui/cn";

export function AttentionCard({ items }: { items: DashboardAttention[] }) {
  return (
    <section className="panel panel-attention" aria-label="Needs attention">
      <header className="panel-header">
        <div className="panel-title">
          <span className="panel-kicker">Needs a look</span>
          <h2 className="panel-heading">
            <AlertTriangle size={14} className="panel-heading-icon" />
            Attention
          </h2>
        </div>
        <span className="panel-count">{items.length}</span>
      </header>
      {items.length === 0 ? (
        <div className="steady-state">
          <span aria-hidden="true">
            <CheckCircle2 size={14} />
          </span>
          <div>
            <strong>Everything monitored looks steady</strong>
            <p>No stale providers, failed refreshes, or low allowances.</p>
          </div>
        </div>
      ) : (
        <ul className="attention-list">
          {items.map((item) => (
            <li key={item.key} className="attention-item">
              <span
                className={cn("attention-dot", `attention-${item.tone}`)}
                aria-hidden="true"
              />
              <div className="attention-copy">
                <strong>{item.title}</strong>
                <p>{item.detail}</p>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function UpcomingTasksCard({
  upcoming,
  onOpenTodo,
}: {
  upcoming: Array<{ id: number; dueDate: string }>;
  onOpenTodo: () => void;
}) {
  return (
    <section className="panel panel-upcoming" aria-label="Upcoming tasks">
      <header className="panel-header">
        <div className="panel-title">
          <span className="panel-kicker">Due next</span>
          <h2 className="panel-heading">Upcoming tasks</h2>
        </div>
        <button type="button" className="inline-link" onClick={onOpenTodo}>
          Open To-do
        </button>
      </header>
      {upcoming.length === 0 ? (
        <p className="panel-muted">No dated open tasks. Add one from the To-do page.</p>
      ) : (
        <ul className="upcoming-list">
          {upcoming.map((task) => (
            <li key={task.id} className="upcoming-item">
              <span className="upcoming-due">{task.dueDate.slice(5).replace("-", "/")}</span>
              <span className="upcoming-copy">Due soon</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}