import { BriefcaseBusiness, Heart } from "lucide-react";
import { useEffect, useState } from "react";
import type {
  TaskItem,
  TaskInput,
  TaskKind,
  TaskPriority,
} from "../../lib/desktop";
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
  /** Rust-owned internal list used by Ellie's single task board. */
  listId: number | null;
  repositories: Array<{ id: number; fullName: string }>;
  /** Friendly error shown after a failed save; draft is preserved. */
  saveError: string;
  onSave: (input: TaskInput) => Promise<void>;
  onClose: () => void;
}

/** Create/edit dialog for Ellie's single local task board. */
export function TaskEditorDialog({
  open,
  onOpenChange,
  task,
  listId,
  repositories,
  saveError,
  onSave,
  onClose,
}: TaskEditorProps) {
  const [title, setTitle] = useState("");
  const [notes, setNotes] = useState("");
  const [kind, setKind] = useState<TaskKind>("personal");
  const [priority, setPriority] = useState<TaskPriority>("none");
  const [dueDate, setDueDate] = useState("");
  const [repositoryId, setRepositoryId] = useState<number>(0);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setTitle(task?.title ?? "");
    setNotes(task?.notes ?? "");
    setKind(task?.kind ?? "personal");
    setPriority(task?.priority ?? "none");
    setDueDate(task?.dueDate ?? "");
    setRepositoryId(task?.repository?.repositoryId ?? 0);
    setSaving(false);
  }, [open, task]);

  function chooseKind(next: TaskKind) {
    setKind(next);
    if (next === "personal") setRepositoryId(0);
  }

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!listId || !title.trim() || saving) return;
    setSaving(true);
    const repository =
      kind === "work" && repositoryId
        ? (() => {
            const repo = repositories.find((item) => item.id === repositoryId);
            return repo ? { repositoryId: repo.id, fullName: repo.fullName } : null;
          })()
        : null;
    await onSave({
      listId: task?.listId ?? listId,
      title: title.trim(),
      notes: notes.trim() ? notes : null,
      kind,
      priority,
      dueDate: dueDate || null,
      repository,
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
      description="Saved privately in Ellie's local database on this PC."
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
        <Field label="Task name">
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
        <Field label="Task details">
          <textarea
            className="input textarea"
            value={notes}
            onChange={(event) => setNotes(event.target.value)}
            placeholder="Notes, context, or the next step"
            rows={4}
            maxLength={4000}
          />
        </Field>

        <fieldset className="task-kind-field">
          <legend className="field-label">Task type</legend>
          <div className="task-kind-picker" role="radiogroup" aria-label="Task type">
            <button
              type="button"
              role="radio"
              aria-checked={kind === "work"}
              className={kind === "work" ? "task-kind-option task-kind-option-active" : "task-kind-option"}
              onClick={() => chooseKind("work")}
            >
              <BriefcaseBusiness size={16} />
              <span><strong>Work</strong><small>Optionally link a GitHub repository</small></span>
            </button>
            <button
              type="button"
              role="radio"
              aria-checked={kind === "personal"}
              className={kind === "personal" ? "task-kind-option task-kind-option-active" : "task-kind-option"}
              onClick={() => chooseKind("personal")}
            >
              <Heart size={16} />
              <span><strong>Personal</strong><small>Learning, life, errands, or anything else</small></span>
            </button>
          </div>
        </fieldset>

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
          <Field label="Priority">
            <Select
              label="Priority"
              value={priority}
              onValueChange={setPriority}
              options={PRIORITY_OPTIONS}
            />
          </Field>
        </div>

        {kind === "work" && (
          <Field label="GitHub repository (optional)">
            <Select
              label="GitHub repository"
              value={repositoryId ? String(repositoryId) : "0"}
              onValueChange={(value) => setRepositoryId(Number(value))}
              options={[
                { value: "0", label: "No repository" },
                ...repositories.map((repo) => ({ value: String(repo.id), label: repo.fullName })),
              ]}
              placeholder="Choose a repository"
            />
          </Field>
        )}
        {kind === "work" && repositories.length === 0 && (
          <p className="form-hint">
            Connect GitHub and load repositories to attach one. You can still save this work task now.
          </p>
        )}
      </form>
    </Modal>
  );
}
