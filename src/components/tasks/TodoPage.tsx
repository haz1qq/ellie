/* eslint-disable react-refresh/only-export-components */
import { useEffect, useRef, useState } from "react";
import {
  CalendarClock,
  CheckCircle2,
  FolderPlus,
  Pencil,
  Pin,
  PinOff,
  Plus,
  SquarePen,
  Trash2,
} from "lucide-react";
import {
  desktop,
  type TaskCompletionFilter,
  type TaskItem,
  type TaskInput,
  type TaskPriority,
} from "../../lib/desktop";
import { formatDueDate, isOverdue } from "../../lib/format";
import type { TaskController } from "../../lib/tasks";
import { Badge, PriorityBadge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { Checkbox } from "../ui/Checkbox";
import { EmptyState, Spinner } from "../ui/Panel";
import { Menu } from "../ui/Menu";
import { Modal, ModalActions } from "../ui/Modal";
import { Field, Select } from "../ui/Select";
import { TaskEditorDialog } from "./TaskEditorDialog";

const COMPLETION_OPTIONS: Array<{ value: TaskCompletionFilter; label: string }> = [
  { value: "all", label: "All tasks" },
  { value: "open", label: "Open" },
  { value: "completed", label: "Completed" },
];

const PRIORITY_FILTER_OPTIONS: Array<{ value: string; label: string }> = [
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
  /** Commanded from elsewhere (dashboard/new-task action): opens the editor. */
  createRequest: number;
  onConsumeCreateRequest: () => void;
  /** Commanded from the dashboard focus card: open the editor for this task. */
  editTaskId?: number | null;
  onConsumeEditRequest?: () => void;
}

/** Column header helper. */
export function TodoPage(props: TodoPageProps) {
  const {
    native,
    tasks,
    repositories,
    createRequest,
    onConsumeCreateRequest,
    editTaskId,
    onConsumeEditRequest,
  } = props;
  const [selectedListId, setSelectedListId] = useState<number | null>(null);
  const [completion, setCompletion] = useState<TaskCompletionFilter>("all");
  const [priorityFilter, setPriorityFilter] = useState<TaskPriority | "any">("any");

  const [editorOpen, setEditorOpen] = useState(false);
  const [editingTask, setEditingTask] = useState<TaskItem | null>(null);
  const [saveError, setSaveError] = useState("");
  const [createTaskAfterList, setCreateTaskAfterList] = useState(false);

  const [listDialog, setListDialog] = useState<"create" | "rename" | "delete" | null>(null);
  const [listName, setListName] = useState("");
  const [listDeleteTarget, setListDeleteTarget] = useState<{ id: number; name: string; count: number } | null>(null);
  const [listDeleteLoading, setListDeleteLoading] = useState(false);

  const [deleteTask, setDeleteTask] = useState<TaskItem | null>(null);
  const [deleteTaskLoading, setDeleteTaskLoading] = useState(false);

  // Re-request from the shell (e.g. "Add a task" in the top bar).
  const [lastCreateRequest, setLastCreateRequest] = useState(0);
  useEffect(() => {
    if (createRequest === lastCreateRequest) return;
    setLastCreateRequest(createRequest);
    setEditingTask(null);
    setSaveError("");
    if (tasks.lists.length === 0) {
      setCreateTaskAfterList(true);
      setListName("");
      setListDialog("create");
    } else {
      setEditorOpen(true);
    }
    onConsumeCreateRequest();
  }, [createRequest, lastCreateRequest, onConsumeCreateRequest, tasks.lists.length]);

  // Edit request from the dashboard focus card.
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

  const lists = tasks.lists;
  const allTasks = tasks.tasks;
  const allTasksRef = useRef(allTasks);
  allTasksRef.current = allTasks;

  const activeList = lists.find((list) => list.id === selectedListId) ?? null;
  const effectiveListId = activeList ? activeList.id : null;

  const filtered = allTasks.filter((task) => {
    if (effectiveListId !== null && task.listId !== effectiveListId) return false;
    if (completion === "open" && task.completedAt) return false;
    if (completion === "completed" && !task.completedAt) return false;
    if (priorityFilter !== "any" && task.priority !== priorityFilter) return false;
    return true;
  });

  function openCreateOnList(listId?: number) {
    setEditingTask(null);
    setSaveError("");
    if (listId !== undefined) setSelectedListId(listId);
    if (lists.length === 0) {
      setCreateTaskAfterList(true);
      setListName("");
      setListDialog("create");
      return;
    }
    setEditorOpen(true);
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
      // Completing semantics are separate; creating/editing keeps the pin.
      setEditorOpen(false);
      setEditingTask(null);
      setSaveError("");
    } else {
      setSaveError(tasks.error || "Ellie couldn’t save the task. Try again.");
    }
  }

  async function handleDeleteList() {
    if (!listDeleteTarget) return;
    setListDeleteLoading(true);
    const ok = await tasks.deleteList(listDeleteTarget.id, listDeleteTarget.count);
    setListDeleteLoading(false);
    if (ok) {
      setListDialog(null);
      setListDeleteTarget(null);
      if (selectedListId === listDeleteTarget.id) setSelectedListId(null);
    } else {
      // Count changed or storage failed; refresh preview to renew confirmation.
      const preview = await desktop.taskListDeletePreview(listDeleteTarget.id);
      if (preview) {
        setListDeleteTarget({ ...listDeleteTarget, count: preview.taskCount });
      }
    }
  }

  async function handleDeleteTask() {
    if (!deleteTask) return;
    setDeleteTaskLoading(true);
    const ok = await tasks.deleteTask(deleteTask.id);
    setDeleteTaskLoading(false);
    if (ok) setDeleteTask(null);
  }

  return (
    <div className="todo-page">
      <div className="todo-toolbar">
        <div className="todo-toolbar-title">
          <p className="eyebrow">Local lists</p>
          <h1 className="page-title">To-do</h1>
          <p className="page-sub">
            {tasks.loading
              ? "Loading your lists…"
              : `${tasks.tasks.filter((task) => !task.completedAt).length} open · ${tasks.tasks.filter((task) => task.completedAt).length} completed · kept on this device`}
          </p>
        </div>
        <div className="todo-toolbar-actions">
          <Button variant="primary" onClick={() => openCreateOnList()} disabled={!native || tasks.loading}>
            <Plus size={15} />
            New task
          </Button>
        </div>
      </div>

      <div className="todo-layout">
        <aside className="todo-lists" aria-label="Task lists">
          <div className="todo-lists-header">
            <strong>Lists</strong>
            <Button size="icon" variant="ghost" aria-label="Create list" disabled={!native} onClick={() => { setCreateTaskAfterList(false); setListName(""); setListDialog("create"); }}>
              <FolderPlus size={15} />
            </Button>
          </div>
          <ul className="todo-list-nav">
            <li>
              <button
                type="button"
                className={!activeList ? "todo-list-item todo-list-active" : "todo-list-item"}
                onClick={() => setSelectedListId(null)}
              >
                <span>All tasks</span>
                <span className="todo-list-count">{tasks.tasks.length}</span>
              </button>
            </li>
            {lists.map((list) => (
              <li key={list.id} className="todo-list-row">
                <button
                  type="button"
                  className={activeList?.id === list.id ? "todo-list-item todo-list-active" : "todo-list-item"}
                  onClick={() => setSelectedListId(list.id)}
                >
                  <span>{list.name}</span>
                  <span className="todo-list-count">{list.taskCount}</span>
                </button>
                {activeList?.id === list.id && (
                  <>
                    <button
                      type="button"
                      className="list-delete-btn"
                      aria-label="Delete list"
                      onClick={() => {
                        setListName("");
                        void desktop.taskListDeletePreview(list.id).then((preview) => {
                          setListDeleteTarget({ id: list.id, name: list.name, count: preview.taskCount });
                        });
                        setListDialog("delete");
                      }}
                    >
                      <Trash2 size={13} />
                    </button>
                    <Menu
                      label="More list actions"
                      align="end"
                      items={[
                        {
                          label: "Rename list",
                          icon: <Pencil size={13} />,
                          onSelect: () => {
                            setListName(list.name);
                            setListDialog("rename");
                          },
                        },
                        {
                          label: "Delete list…",
                          danger: true,
                          icon: <Trash2 size={13} />,
                          onSelect: () => setListDeleteTarget({ id: list.id, name: list.name, count: list.taskCount }),
                        },
                      ]}
                    />
                  </>
                )}
              </li>
            ))}
            {lists.length === 0 && !tasks.loading && (
              <li className="todo-lists-empty">No lists yet. Create one to start.</li>
            )}
          </ul>
        </aside>

        <main className="todo-main" aria-label="Tasks">
          <div className="todo-filters" role="group" aria-label="Task filters">
            <Select
              label="Filter by completion"
              value={completion}
              onValueChange={setCompletion}
              options={COMPLETION_OPTIONS}
            />
            <Select
              label="Filter by priority"
              value={priorityFilter}
              onValueChange={(value) => setPriorityFilter(value as TaskPriority | "any")}
              options={PRIORITY_FILTER_OPTIONS}
            />
            {activeList && (
              <Button size="sm" variant="ghost" onClick={() => openCreateOnList(activeList.id)} disabled={!native}>
                <Plus size={13} />
                Add to {activeList.name}
              </Button>
            )}
          </div>

          {tasks.error && (
            <p className="page-alert" role="alert">
              {tasks.error}
            </p>
          )}

          {tasks.loading ? (
            <Spinner label="Loading tasks…" />
          ) : filtered.length === 0 ? (
            <EmptyState
              icon={<CheckCircle2 size={18} />}
              title={
                allTasks.length === 0
                  ? "No tasks yet"
                  : "No tasks match these filters"
              }
            >
              <p className="empty-state-text">
                {allTasks.length === 0
                  ? "Add your first task, or create a list to organize your work."
                  : "Clear a filter to see more tasks."}
              </p>
              {allTasks.length === 0 && (
                <Button size="sm" onClick={() => openCreateOnList()} disabled={!native} className="empty-state-action">
                  {lists.length === 0 ? <FolderPlus size={13} /> : <SquarePen size={13} />}
                  {lists.length === 0 ? "Create your first list" : "Add your first task"}
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
      </div>

      <TaskEditorDialog
        open={editorOpen}
        onOpenChange={setEditorOpen}
        task={editingTask}
        lists={lists}
        repositories={repositories}
        saveError={saveError}
        onSave={saveTask}
        onClose={() => setEditingTask(null)}
      />

      <ListDialog
        dialog={listDialog}
        native={native}
        lists={lists}
        target={activeList}
        name={listName}
        onNameChange={setListName}
        onCreate={async () => {
          const created = await tasks.createList(listName.trim());
          if (created) {
            setListDialog(null);
            setListName("");
            setSelectedListId(created.id);
            if (createTaskAfterList) {
              setCreateTaskAfterList(false);
              setEditingTask(null);
              setEditorOpen(true);
            }
          }
        }}
        onRename={async () => {
          if (activeList && listName.trim()) {
            const renamed = await tasks.renameList(activeList.id, listName.trim());
            if (renamed) setListDialog(null);
          }
        }}
        onOpenDelete={() => {
          if (!activeList) return;
          setListDialog("delete");
          void desktop.taskListDeletePreview(activeList.id).then((preview) => {
            setListDeleteTarget({ id: activeList.id, name: activeList.name, count: preview.taskCount });
          });
          setListName("");
        }}
        onClose={() => {
          setCreateTaskAfterList(false);
          setListDialog(null);
        }}
        busy={tasks.busy}
        createError={tasks.error}
      />

      {listDeleteTarget && listDialog === "delete" && (
        <Modal
          open
          onOpenChange={(open) => {
            if (!open && !listDeleteLoading) setListDialog(null);
          }}
          title="Delete this list?"
          description={`"${listDeleteTarget.name}" has ${listDeleteTarget.count} task${listDeleteTarget.count === 1 ? "" : "s"}. Deleting the list removes them too.`}
          footer={
            <>
              {tasks.error && (
                <p className="form-error" role="alert">
                  {tasks.error}
                </p>
              )}
              <ModalActions>
                <Button variant="ghost" onClick={() => setListDialog(null)} disabled={listDeleteLoading}>
                  Cancel
                </Button>
                <Button variant="danger" onClick={() => void handleDeleteList()} disabled={listDeleteLoading}>
                  <Trash2 size={14} />
                  {listDeleteLoading ? "Deleting…" : "Delete list"}
                </Button>
              </ModalActions>
            </>
          }
        >
          <p className="modal-note">
            If the list changed since confirmation, Ellie asks you to confirm again —
            it never deletes silent surprises.
          </p>
        </Modal>
      )}

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
              {tasks.error && (
                <p className="form-error" role="alert">
                  {tasks.error}
                </p>
              )}
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
          <p className="modal-note">
            If this is the pinned task, the pin is cleared automatically.
          </p>
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
    <li className={task.completedAt ? "task-row task-row-completed" : "task-row"}>
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
          {pinned && (
            <Badge tone="accent" className="task-pinned-badge">
              <Pin size={10} /> Focus
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
          {task.repository && (
            <a
              className="task-repo-link"
              href={task.repository.htmlUrl}
              target="_blank"
              rel="noreferrer"
              onClick={(event) => event.stopPropagation()}
            >
              {task.repository.fullName}
            </a>
          )}
        </div>
        {task.notes && <p className="task-row-notes">{task.notes}</p>}
      </div>
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
            label: pinned ? "Unpin from Focus" : "Pin to Focus",
            icon: pinned ? <PinOff size={14} /> : <Pin size={14} />,
            onSelect: onPin,
            disabled: busy || !native || Boolean(task.completedAt),
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
    </li>
  );
}

function ListDialog({
  dialog,
  native,
  lists,
  target,
  name,
  onNameChange,
  onCreate,
  onRename,
  onOpenDelete,
  onClose,
  busy,
  createError,
}: {
  dialog: "create" | "rename" | "delete" | null;
  native: boolean;
  lists: Array<{ id: number; name: string }>;
  target: { id: number; name: string } | null;
  name: string;
  onNameChange: (value: string) => void;
  onCreate: () => void;
  onRename: () => void;
  onOpenDelete: () => void;
  onClose: () => void;
  busy: boolean;
  createError: string;
}) {
  if (dialog !== "create" && dialog !== "rename") return null;
  const isCreate = dialog === "create";
  const conflict = lists.some(
    (list) => list.name.toLowerCase() === name.trim().toLowerCase() && list.id !== target?.id,
  );
  return (
    <Modal
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={isCreate ? "Create a list" : "Rename list"}
      description={isCreate ? "Lists keep related tasks together." : target?.name}
      footer={
        <>
          {createError && (
            <p className="form-error" role="alert">
              {createError}
            </p>
          )}
          <ModalActions>
            <Button variant="ghost" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button
              variant="primary"
              disabled={!name.trim() || conflict || busy || !native}
              onClick={() => (isCreate ? onCreate() : onRename())}
            >
              {busy ? "Saving…" : isCreate ? "Create list" : "Rename"}
            </Button>
            {!isCreate && (
              <Menu
                label="More list actions"
                items={[
                  {
                    label: "Delete list…",
                    danger: true,
                    icon: <Trash2 size={14} />,
                    onSelect: onOpenDelete,
                  },
                ]}
              />
            )}
          </ModalActions>
        </>
      }
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (isCreate) onCreate();
          else onRename();
        }}
      >
        <Field label="List name">
          <input
            className="input"
            value={name}
            onChange={(event) => onNameChange(event.target.value)}
            placeholder="e.g. Workspace, Errands, Ideas"
            autoFocus
            maxLength={80}
            required
          />
        </Field>
        {conflict && (
          <p className="form-error" role="alert">
            A list with that name already exists.
          </p>
        )}
      </form>
    </Modal>
  );
}

/** Stable helper: the first repo to suggest in the task editor. */
export function firstRepository(
  repositories: Array<{ id: number; fullName: string }>,
): { id: number; fullName: string } | null {
  return repositories[0] ?? null;
}