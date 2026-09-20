import { useEffect, useState } from "react";
import {
  BriefcaseBusiness,
  CalendarClock,
  Cat,
  Check,
  ExternalLink,
  Heart,
  PinOff,
} from "lucide-react";
import { desktop, type TaskItem } from "../../lib/desktop";
import { formatDueDate, isOverdue } from "../../lib/format";
import { taskErrorText } from "../../lib/tasks";

/** Least-privilege always-on-top view for Ellie's one pinned local task. */
export default function TaskStickyNote() {
  const [task, setTask] = useState<TaskItem | null>(null);
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState<"complete" | "unpin" | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    if (!desktop.available()) {
      setLoading(false);
      return;
    }
    void desktop.onTaskNoteUpdated((next) => {
      if (active) {
        setTask(next);
        setError("");
      }
    }).then((stop) => {
      if (active) unlisten = stop;
      else stop();
    });
    void desktop.taskNoteBootstrap()
      .then((next) => {
        if (active) setTask(next);
      })
      .catch((reason: unknown) => {
        if (active) setError(taskErrorText(reason));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  async function complete() {
    if (!task || working) return;
    setWorking("complete");
    setError("");
    try {
      await desktop.taskNoteComplete();
      setTask(null);
    } catch (reason) {
      setError(taskErrorText(reason));
    } finally {
      setWorking(null);
    }
  }

  async function unpin() {
    if (!task || working) return;
    setWorking("unpin");
    setError("");
    try {
      await desktop.taskNoteUnpin();
      setTask(null);
    } catch (reason) {
      setError(taskErrorText(reason));
    } finally {
      setWorking(null);
    }
  }

  const overdue = task ? isOverdue(task.dueDate, task.completedAt) : false;

  return (
    <main className="task-sticky-shell" aria-label="Ellie pinned task">
      <section className="task-sticky-note">
        <header className="task-sticky-header" data-tauri-drag-region>
          <div className="task-sticky-brand" data-tauri-drag-region>
            <span className="task-sticky-cat" aria-hidden="true" data-tauri-drag-region>
              <Cat size={16} />
            </span>
            <span data-tauri-drag-region>
              <strong>Ellie note</strong>
              <small>Drag me anywhere</small>
            </span>
          </div>
          <button
            type="button"
            className="task-sticky-icon-btn"
            aria-label="Unpin and close sticky note"
            title="Unpin and close"
            disabled={!task || Boolean(working)}
            onClick={() => void unpin()}
          >
            <PinOff size={15} />
          </button>
        </header>

        <div className="task-sticky-body">
          {loading ? (
            <p className="task-sticky-status" role="status">Finding your pinned task…</p>
          ) : task ? (
            <>
              <div className="task-sticky-badges">
                <span className={`task-kind-chip task-kind-${task.kind}`}>
                  {task.kind === "work" ? <BriefcaseBusiness size={11} /> : <Heart size={11} />}
                  {task.kind === "work" ? "Work" : "Personal"}
                </span>
                {task.repository && (
                  <span className="task-sticky-repo">{task.repository.fullName}</span>
                )}
              </div>
              <h1 className="task-sticky-title">{task.title}</h1>
              {task.notes && <p className="task-sticky-details">{task.notes}</p>}
              {task.dueDate && (
                <p className={overdue ? "task-sticky-due task-sticky-overdue" : "task-sticky-due"}>
                  <CalendarClock size={13} />
                  {overdue ? "Overdue · " : "Due · "}{formatDueDate(task.dueDate)}
                </p>
              )}
            </>
          ) : (
            <p className="task-sticky-status">No task is pinned.</p>
          )}
          {error && <p className="task-sticky-error" role="alert">{error}</p>}
        </div>

        <footer className="task-sticky-actions">
          <button
            type="button"
            className="task-sticky-primary"
            disabled={!task || Boolean(working)}
            onClick={() => void complete()}
          >
            <Check size={14} />
            {working === "complete" ? "Finishing…" : "Complete"}
          </button>
          <button
            type="button"
            className="task-sticky-secondary"
            onClick={() => void desktop.openMainWindow()}
          >
            <ExternalLink size={13} />
            Open Ellie
          </button>
        </footer>
      </section>
    </main>
  );
}
