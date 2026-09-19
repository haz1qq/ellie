import { useEffect, useState } from "react";
import type { TaskItem, TaskInput, TaskPriority } from "../../lib/desktop";
import { Button } from "../ui/Button";
import { Field, Select } from "../ui/Select";
import { Modal, ModalActions } from "../ui/Modal";

const PRIORITY_OPTIONS: Array<{ value: TaskPriority; label: string }> = [
  { value: "none", label: "No priority" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
];

export interface TaskEditorProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** null = create; otherwise edit. */
  task: TaskItem | null;
  /** Default list for new tasks. */
  lists: Array<{ id: number; name: string; taskCount: number }>;
  repositories: Array<{ id: number; fullName: string }>;
  /** Friendly error shown after a failed save; draft is preserved. */
  saveError: string;
  onSave: (input: TaskInput) => Promise<void>;
  onClose: () => void;
}

/**
 * Create/edit task dialog. On save failure the parent keeps the current draft
 * inputs and shows `saveError`, then re-renders this dialog with state intact.
 */
export function TaskEditorDialog({
  open,
  onOpenChange,
  task,
  lists,
  repositories,
  saveError,
  onSave,
  onClose,
}: TaskEditorProps) {
  const [listId, setListId] = useState<number>(0);
  const [title, setTitle] = useState("");
  const [notes, setNotes] = useState("");
  const [priority, setPriority] = useState<TaskPriority>("none");
  const [dueDate, setDueDate] = useState("");
  const [repositoryId, setRepositoryId] = useState<number>(0);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    const defaultList = lists[0];
    setListId(task?.listId ?? defaultList?.id ?? 0);
    setTitle(task?.title ?? "");
    setNotes(task?.notes ?? "");
    setPriority(task?.priority ?? "none");
    setDueDate(task?.dueDate ?? "");
    setRepositoryId(task?.repository?.repositoryId ?? 0);
    setSaving(false);
  }, [open, task, lists]);

  const repositoryOptions = [
    ...repositories.map((repo) => ({ value: String(repo.id), label: repo.fullName })),
  ];

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!listId || !title.trim() || saving) return;
    setSaving(true);
    await onSave({
      listId,
      title: title.trim(),
      notes: notes.trim() ? notes : null,
      priority,
      dueDate: dueDate || null,
      repository: repositoryId
        ? (() => {
            const repo = repositories.find((item) => item.id === repositoryId);
            return repo ? { repositoryId: repo.id, fullName: repo.fullName } : null;
          })()
        : null,
    });
    setSaving(false);
  }

  const canSave = Boolean(listId) && Boolean(title.trim()) && !saving;

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      onClose={onClose}
      title={task ? "Edit task" : "New task"}
      description={
        task
          ? "Save changes locally on this device."
          : "Tasks live locally — no account or sync required."
      }
      width="lg"
      footer={
        <>
          {saveError && (
            <p className="form-error" role="alert">
              {saveError}
            </p>
          )}
          <ModalActions>
            <Button type="submit" form="task-editor-form" variant="primary" disabled={!canSave}>
              {saving ? "Saving…" : task ? "Save changes" : "Add task"}
            </Button>
          </ModalActions>
        </>
      }
    >
      <form id="task-editor-form" onSubmit={(event) => void submit(event)} className="task-editor-form">
        <Field label="Title">
          <input
            className="input"
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="What needs doing?"
            autoFocus
            maxLength={200}
            required
          />
        </Field>
        <Field label="Notes">
          <textarea
            className="input textarea"
            value={notes}
            onChange={(event) => setNotes(event.target.value)}
            placeholder="Optional context, kept on this device"
            rows={3}
            maxLength={4000}
          />
        </Field>
        <div className="task-editor-grid">
          <Field label="List">
            <Select
              label="List"
              value={String(listId)}
              onValueChange={(value) => setListId(Number(value))}
              options={lists.map((list) => ({ value: String(list.id), label: list.name }))}
              placeholder="Choose a list"
            />
          </Field>
          <Field label="Priority">
            <Select
              label="Priority"
              value={priority}
              onValueChange={(value) => setPriority(value)}
              options={PRIORITY_OPTIONS}
            />
          </Field>
        </div>
        <div className="task-editor-grid">
          <Field label="Due date">
            <input
              className="input"
              type="date"
              value={dueDate}
              onChange={(event) => setDueDate(event.target.value)}
              aria-label="Due date"
            />
          </Field>
          <Field label="Repository link">
            <Select
              label="Repository link"
              value={repositoryId ? String(repositoryId) : "0"}
              onValueChange={(value) => setRepositoryId(Number(value))}
              options={[
                { value: "0", label: "No repository" },
                ...repositoryOptions,
              ]}
              placeholder="Choose a repository"
            />
          </Field>
        </div>
        {repositories.length === 0 && (
          <p className="form-hint">
            Loaded GitHub repositories appear here. Connect GitHub to link a task to a repository.
          </p>
        )}
      </form>
    </Modal>
  );
}