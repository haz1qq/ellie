import { useEffect, useRef, useState } from "react";
import {
  BriefcaseBusiness,
  CalendarClock,
  CheckCircle2,
  Heart,
  ListChecks,
  Pencil,
  Pin,
  PinOff,
  Plus,
  Trash2,
} from "lucide-react";
import type {
  TaskCompletionFilter,
  TaskItem,
  TaskInput,
  TaskKind,
  TaskPriority,
} from "../../lib/desktop";
import { formatDueDate, isOverdue } from "../../lib/format";
import type { TaskController } from "../../lib/tasks";
import { Badge, PriorityBadge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { Checkbox } from "../ui/Checkbox";
import { EmptyState, Spinner } from "../ui/Panel";
import { Menu } from "../ui/Menu";
import { Modal, ModalActions } from "../ui/Modal";
import { Select } from "../ui/Select";
import { TaskEditorDialog } from "./TaskEditorDialog";

const COMPLETION_OPTIONS: Array<{ value: TaskCompletionFilter; label: string }> = [
  { value: "all", label: "All tasks" },
  { value: "open", label: "Open" },
  { value: "completed", label: "Completed" },
];

const KIND_OPTIONS: Array<{ value: TaskKind | "any"; label: string }> = [
  { value: "any", label: "Work + personal" },
  { value: "work", label: "Work" },
  { value: "personal", label: "Personal" },
];

const PRIORITY_FILTER_OPTIONS: Array<{ value: TaskPriority | "any"; label: string }> = [
  { value: "any", label: "Any priority" },
  { value: "none", label: "No priority" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
];

export interface TodoPageProps {
  native: boolean;
  tasks: TaskController;
  repositories: Array<{ id: number; fullName: string; htmlUrl: string }>;
  createRequest: number;
  onConsumeCreateRequest: () => void;
  editTaskId?: number | null;
  onConsumeEditRequest?: () => void;
}

/** Ellie's single, local-first task board. */
export function TodoPage({
  native,
  tasks,
  repositories,
  createRequest,
  onConsumeCreateRequest,
  editTaskId,
  onConsumeEditRequest,
}: TodoPageProps) {
  const [completion, setCompletion] = useState<TaskCompletionFilter>("all");
  const [kindFilter, setKindFilter] = useState<TaskKind | "any">("any");
  const [priorityFilter, setPriorityFilter] = useState<TaskPriority | "any">("any");
  const [editorOpen, setEditorOpen] = useState(false);
  const [editingTask, setEditingTask] = useState<TaskItem | null>(null);
  const [saveError, setSaveError] = useState("");
  const [deleteTask, setDeleteTask] = useState<TaskItem | null>(null);
  const [deleteTaskLoading, setDeleteTaskLoading] = useState(false);
  const [lastCreateRequest, setLastCreateRequest] = useState(0);

  const allTasks = tasks.tasks;
  const allTasksRef = useRef(allTasks);
  allTasksRef.current = allTasks;
  const primaryListId = tasks.lists[0]?.id ?? null;

  useEffect(() => {
    if (createRequest === lastCreateRequest || tasks.loading) return;
    setLastCreateRequest(createRequest);
    setEditingTask(null);
    setSaveError("");
    if (primaryListId) setEditorOpen(true);
    onConsumeCreateRequest();
  }, [createRequest, lastCreateRequest, onConsumeCreateRequest, primaryListId, tasks.loading]);

  useEffect(() => {
    if (editTaskId == null) return;
    const task = allTasksRef.current.find((item) => item.id === editTaskId);
    if (task) {
      setEditingTask(task);
      setSaveError("");
      setEditorOpen(true);
    }
    onConsumeEditRequest?.();
  }, [editTaskId, onConsumeEditRequest]);

  const filtered = allTasks.filter((task) => {
    if (completion === "open" && task.completedAt) return false;
    if (completion === "completed" && !task.completedAt) return false;
    if (kindFilter !== "any" && task.kind !== kindFilter) return false;
    if (priorityFilter !== "any" && task.priority !== priorityFilter) return false;
    return true;
  });
  const openCount = allTasks.filter((task) => !task.completedAt).length;
  const completedCount = allTasks.length - openCount;
  const overdueCount = allTasks.filter((task) => isOverdue(task.dueDate, task.completedAt)).length;
  const pinnedTask = allTasks.find((task) => task.id === tasks.pinnedTaskId) ?? null;

  function openCreate() {
    setEditingTask(null);
    setSaveError("");
    if (primaryListId) setEditorOpen(true);
  }

  function openEdit(task: TaskItem) {
    setEditingTask(task);
    setSaveError("");
    setEditorOpen(true);
  }

  async function saveTask(input: TaskInput) {
    const result = editingTask
      ? await tasks.updateTask(editingTask.id, input)
      : await tasks.createTask(input);
    if (result) {
      setEditorOpen(false);
      setEditingTask(null);
      setSaveError("");
    } else {
      setSaveError(tasks.error || "Ellie couldn’t save the task. Your draft is still here.");
    }
  }

  async function handleDeleteTask() {
    if (!deleteTask) return;
    setDeleteTaskLoading(true);
    const deleted = await tasks.deleteTask(deleteTask.id);
    setDeleteTaskLoading(false);
    if (deleted) setDeleteTask(null);
  }

  return (
    <div className="todo-page">
      <div className="todo-toolbar">
        <div className="todo-toolbar-title">
          <p className="eyebrow">Private · saved on this PC</p>
          <h1 className="page-title">My tasks</h1>
          <p className="page-sub">
            One quiet place for work, personal plans, due dates, and your current sticky note.
          </p>
        </div>
        <div className="todo-toolbar-actions">
          <Button
            variant="primary"
            onClick={openCreate}
            disabled={!native || tasks.loading || !primaryListId}
          >
            <Plus size={15} />
            New task
          </Button>
        </div>
      </div>

      <section className="todo-summary" aria-label="Task summary">
        <div><strong>{openCount}</strong><span>Open</span></div>
        <div><strong>{completedCount}</strong><span>Completed</span></div>
        <div className={overdueCount ? "todo-summary-attention" : ""}>
          <strong>{overdueCount}</strong><span>Overdue</span>
        </div>
        <div className={pinnedTask ? "todo-summary-pinned" : ""}>
          <strong>{pinnedTask ? "1" : "0"}</strong><span>Sticky</span>
        </div>
      </section>

      <main className="todo-main" aria-label="Tasks">
        <div className="todo-filters" role="group" aria-label="Task filters">
          <Select
            label="Filter by completion"
            value={completion}
            onValueChange={setCompletion}
            options={COMPLETION_OPTIONS}
          />
          <Select
            label="Filter by task type"
            value={kindFilter}
            onValueChange={setKindFilter}
            options={KIND_OPTIONS}
          />
          <Select
            label="Filter by priority"
            value={priorityFilter}
            onValueChange={setPriorityFilter}
            options={PRIORITY_FILTER_OPTIONS}
          />
        </div>

        {tasks.error && (
          <div className="page-alert" role="alert">
            <span>{tasks.error}</span>
            <Button size="sm" variant="ghost" onClick={() => void tasks.refresh()} disabled={tasks.loading}>
              Retry
            </Button>
          </div>
        )}

        {tasks.loading ? (
          <Spinner label="Opening your local task board…" />
        ) : filtered.length === 0 ? (
          <EmptyState
            icon={<ListChecks size={19} />}
            title={allTasks.length === 0 ? "Your task board is ready" : "No tasks match these filters"}
          >
            <p className="empty-state-text">
              {allTasks.length === 0
                ? "Add a task with details, a due date, and an optional GitHub repository."
                : "Try another type, status, or priority filter."}
            </p>
            {allTasks.length === 0 && (
              <Button size="sm" onClick={openCreate} disabled={!native || !primaryListId} className="empty-state-action">
                <Plus size={13} />
                Add your first task
              </Button>
            )}
          </EmptyState>
        ) : (
          <ul className="task-list">
            {filtered.map((task) => (
              <TaskRow
                key={task.id}
                task={task}
                pinned={task.id === tasks.pinnedTaskId}
                native={native}
                busy={tasks.busy}
                onToggle={(completed) => void tasks.setCompleted(task.id, completed)}
                onEdit={() => openEdit(task)}
                onDelete={() => setDeleteTask(task)}
                onPin={() => void tasks.setPinned(task.id === tasks.pinnedTaskId ? null : task.id)}
              />
            ))}
          </ul>
        )}
      </main>

      <TaskEditorDialog
        open={editorOpen}
        onOpenChange={setEditorOpen}
        task={editingTask}
        listId={primaryListId}
        repositories={repositories}
        saveError={saveError}
        onSave={saveTask}
        onClose={() => setEditingTask(null)}
      />

      {deleteTask && (
        <Modal
          open
          onOpenChange={(open) => {
            if (!open && !deleteTaskLoading) setDeleteTask(null);
          }}
          title="Delete this task?"
          description={deleteTask.title}
          footer={
            <>
              {tasks.error && <p className="form-error" role="alert">{tasks.error}</p>}
              <ModalActions>
                <Button variant="ghost" onClick={() => setDeleteTask(null)} disabled={deleteTaskLoading}>
                  Cancel
                </Button>
                <Button variant="danger" onClick={() => void handleDeleteTask()} disabled={deleteTaskLoading}>
                  <Trash2 size={14} />
                  {deleteTaskLoading ? "Deleting…" : "Delete task"}
                </Button>
              </ModalActions>
            </>
          }
        >
          <p className="modal-note">Deleting a pinned task also closes its desktop sticky note.</p>
        </Modal>
      )}
    </div>
  );
}

function TaskRow({
  task,
  pinned,
  native,
  busy,
  onToggle,
  onEdit,
  onDelete,
  onPin,
}: {
  task: TaskItem;
  pinned: boolean;
  native: boolean;
  busy: boolean;
  onToggle: (completed: boolean) => void;
  onEdit: () => void;
  onDelete: () => void;
  onPin: () => void;
}) {
  const overdue = isOverdue(task.dueDate, task.completedAt);
  return (
    <li className={task.completedAt ? `task-row task-row-${task.kind} task-row-completed` : `task-row task-row-${task.kind}`}>
      <Checkbox
        checked={Boolean(task.completedAt)}
        onCheckedChange={onToggle}
        label={`Mark "${task.title}" ${task.completedAt ? "open" : "complete"}`}
        disabled={busy || !native}
      />
      <div className="task-row-body">
        <div className="task-row-line">
          <button type="button" className="task-row-title" onClick={onEdit}>
            {task.title}
          </button>
          <span className={`task-kind-chip task-kind-${task.kind}`}>
            {task.kind === "work" ? <BriefcaseBusiness size={10} /> : <Heart size={10} />}
            {task.kind === "work" ? "Work" : "Personal"}
          </span>
          {pinned && (
            <Badge tone="accent" className="task-pinned-badge">
              <Pin size={10} /> Sticky
            </Badge>
          )}
        </div>
        <div className="task-row-meta">
          <PriorityBadge priority={task.priority} />
          {task.dueDate && (
            <span className={overdue ? "task-due task-due-overdue" : "task-due"}>
              <CalendarClock size={11} />
              {formatDueDate(task.dueDate)}
              {overdue && " · overdue"}
            </span>
          )}
          {task.repository && <span className="task-repo-link">{task.repository.fullName}</span>}
        </div>
        {task.notes && <p className="task-row-notes">{task.notes}</p>}
      </div>
      <div className="task-row-actions">
        <button
          type="button"
          className={pinned ? "task-pin-button task-pin-button-active" : "task-pin-button"}
          aria-label={pinned ? `Unpin ${task.title} sticky note` : `Pin ${task.title} as sticky note`}
          aria-pressed={pinned}
          title={pinned ? "Unpin sticky note" : "Pin as sticky note"}
          onClick={onPin}
          disabled={busy || !native || Boolean(task.completedAt)}
        >
          {pinned ? <PinOff size={14} /> : <Pin size={14} />}
        </button>
        <Menu
          label={`Actions for ${task.title}`}
          items={[
            {
              label: task.completedAt ? "Reopen task" : "Mark complete",
              icon: <CheckCircle2 size={14} />,
              onSelect: () => onToggle(!task.completedAt),
              disabled: busy || !native,
            },
            {
              label: "Edit",
              icon: <Pencil size={14} />,
              onSelect: onEdit,
            },
            {
              label: "Delete",
              icon: <Trash2 size={14} />,
              danger: true,
              onSelect: onDelete,
            },
          ]}
        />
      </div>
    </li>
  );
}
