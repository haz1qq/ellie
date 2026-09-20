import { BriefcaseBusiness, Heart, Pin, PinOff, SquarePen, StickyNote } from "lucide-react";
import type { TaskItem } from "../../lib/desktop";
import { formatDueDate } from "../../lib/format";
import { PriorityBadge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/Panel";

export interface FocusTaskCardProps {
  pinnedTask: TaskItem | null;
  tasks: TaskItem[];
  busy: boolean;
  native: boolean;
  onOpenTodo: () => void;
  onEdit: (task: TaskItem) => void;
  onComplete: (taskId: number) => void;
  onShowNote: (taskId: number) => void;
  onUnpin: () => void;
}

/** Overview card for the pinned ("current") task; invites when nothing is pinned. */
export function FocusTaskCard({
  pinnedTask,
  tasks,
  busy,
  native,
  onOpenTodo,
  onEdit,
  onComplete,
  onShowNote,
  onUnpin,
}: FocusTaskCardProps) {
  const noneOpen = tasks.length > 0 && tasks.every((task) => task.completedAt);
  return (
    <section className="panel panel-focus" aria-label="Current task">
      <header className="panel-header">
        <div className="panel-title">
          <span className="panel-kicker">Current task</span>
          <h2 className="panel-heading">
            <Pin size={14} className="panel-heading-icon" />
            Focus
          </h2>
        </div>
        {pinnedTask && (
          <Button size="sm" variant="ghost" onClick={onUnpin} disabled={busy || !native}>
            <PinOff size={13} />
            Unpin
          </Button>
        )}
      </header>
      {pinnedTask ? (
        <div className="focus-task">
          <h3 className="focus-task-title">{pinnedTask.title}</h3>
          <div className="focus-task-meta">
            <span className={`task-kind-chip task-kind-${pinnedTask.kind}`}>
              {pinnedTask.kind === "work" ? <BriefcaseBusiness size={10} /> : <Heart size={10} />}
              {pinnedTask.kind === "work" ? "Work" : "Personal"}
            </span>
            <PriorityBadge priority={pinnedTask.priority} />
            {pinnedTask.dueDate && (
              <span className="focus-task-due">{formatDueDate(pinnedTask.dueDate)}</span>
            )}
            {pinnedTask.repository && (
              <span className="focus-task-repo">{pinnedTask.repository.fullName}</span>
            )}
          </div>
          {pinnedTask.notes && <p className="focus-task-notes">{pinnedTask.notes}</p>}
          <div className="focus-task-actions">
            <Button size="sm" onClick={() => onComplete(pinnedTask.id)} disabled={busy || !native}>
              Complete
            </Button>
            <Button size="sm" variant="ghost" onClick={() => onShowNote(pinnedTask.id)} disabled={busy || !native}>
              <StickyNote size={13} />
              Show sticky
            </Button>
            <Button size="sm" variant="ghost" onClick={() => onEdit(pinnedTask)} disabled={busy || !native}>
              Edit
            </Button>
          </div>
        </div>
      ) : (
        <EmptyState
          icon={<Pin size={18} />}
          title={noneOpen ? "All tasks are done" : "No current task"}
        >
          <p className="empty-state-text">
            {noneOpen
              ? "Nothing left on the board — nicely done."
              : "Pin an open task from the To-do page to keep it here."}
          </p>
          <Button size="sm" onClick={onOpenTodo} className="empty-state-action">
            <SquarePen size={13} />
            Open To-do
          </Button>
        </EmptyState>
      )}
    </section>
  );
}