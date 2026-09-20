import { useCallback, useEffect, useRef, useState } from "react";
import {
  desktop,
  type TaskBootstrap,
  type TaskErrorCategory,
  type TaskInput,
  type TaskItem,
  type TaskList,
} from "./desktop";
import { githubErrorText } from "./github";

/** Redacted categories returned by the Rust task service. */
const TASK_ERROR_CATEGORIES: readonly TaskErrorCategory[] = [
  "window_denied",
  "invalid_input",
  "not_found",
  "conflict",
  "count_changed",
  "storage",
];

export function taskCategoryOf(reason: unknown): TaskErrorCategory | null {
  if (typeof reason !== "object" || reason === null) return null;
  const category = (reason as { category?: unknown }).category;
  if (typeof category !== "string") return null;
  return (TASK_ERROR_CATEGORIES as readonly string[]).includes(category)
    ? (category as TaskErrorCategory)
    : null;
}

const TASK_ERROR_COPY: Record<TaskErrorCategory, string> = {
  window_denied: "This window cannot manage tasks. Use the main Ellie window.",
  invalid_input: "The task details aren’t valid. Check the title, dates, and repository, then retry.",
  not_found: "That task or list no longer exists. Refresh and try again.",
  conflict: "A list with that name already exists. Choose another name.",
  count_changed: "This list changed before deletion was confirmed. Review it and confirm again.",
  storage: "Local task storage is unavailable. Existing data is unchanged; retry.",
};

export function taskErrorText(reason: unknown): string {
  const category = taskCategoryOf(reason);
  return category ? TASK_ERROR_COPY[category] : "Ellie couldn’t save the task. Existing data is unchanged; retry.";
}

export interface TaskController {
  loading: boolean;
  lists: TaskList[];
  tasks: TaskItem[];
  pinnedTaskId: number | null;
  /** Friendly copy for the last failed operation; "" when none. */
  error: string;
  busy: boolean;
  refresh: () => Promise<void>;
  createList: (name: string) => Promise<TaskList | null>;
  renameList: (listId: number, name: string) => Promise<TaskList | null>;
  deleteList: (listId: number, expectedTaskCount: number) => Promise<boolean>;
  createTask: (input: TaskInput) => Promise<TaskItem | null>;
  updateTask: (taskId: number, input: TaskInput) => Promise<TaskItem | null>;
  setCompleted: (taskId: number, completed: boolean) => Promise<TaskItem | null>;
  deleteTask: (taskId: number) => Promise<boolean>;
  setPinned: (taskId: number | null) => Promise<boolean>;
  /** True while any mutation or bootstrap read is in flight. */
}

/**
 * Bootstraps the local task workspace once and owns every task mutation.
 * After a mutation the returned entity is applied directly; when an operation
 * gives no entity back (delete/pin), the local state is reconciled locally.
 */
export function useTasks(native: boolean): TaskController {
  const [loading, setLoading] = useState(true);
  const [lists, setLists] = useState<TaskList[]>([]);
  const [tasks, setTasks] = useState<TaskItem[]>([]);
  const [pinnedTaskId, setPinnedTaskId] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const generation = useRef(0);

  useEffect(() => {
    const generationAtMount = ++generation.current;
    if (!native) {
      setLoading(false);
      return;
    }
    let active = true;
    setLoading(true);
    desktop
      .taskBootstrap()
      .then((value: TaskBootstrap) => {
        if (active && generationAtMount === generation.current) {
          applyBootstrap(value);
        }
      })
      .catch((reason: unknown) => {
        if (active && generationAtMount === generation.current) {
          setError(taskErrorText(reason));
        }
      })
      .finally(() => {
        if (active && generationAtMount === generation.current) setLoading(false);
      });
    return () => {
      active = false;
      generation.current += 1;
    };
  }, [native]);

  function applyBootstrap(value: TaskBootstrap) {
    setLists(value.lists);
    setTasks(value.tasks);
    setPinnedTaskId(value.pinnedTaskId);
  }

  const refresh = useCallback(async () => {
    const current = ++generation.current;
    setError("");
    try {
      const value = await desktop.taskBootstrap();
      if (current === generation.current) applyBootstrap(value);
    } catch (reason) {
      if (current === generation.current) setError(taskErrorText(reason));
    }
  }, []);

  useEffect(() => {
    if (!native) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    void desktop.onTasksUpdated(() => {
      if (active) void refresh();
    }).then((stop) => {
      if (active) unlisten = stop;
      else stop();
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [native, refresh]);

  async function run<T>(
    action: () => Promise<T>,
  ): Promise<T | null> {
    const current = ++generation.current;
    setBusy(true);
    setError("");
    try {
      const value = await action();
      if (current !== generation.current) return null;
      return value;
    } catch (reason) {
      if (current === generation.current) setError(taskErrorText(reason));
      return null;
    } finally {
      if (current === generation.current) setBusy(false);
    }
  }

  return {
    loading,
    lists,
    tasks,
    pinnedTaskId,
    error,
    busy,
    refresh,
    createList: async (name) => {
      const list = await run(() => desktop.taskCreateList(name));
      if (list) await refresh();
      return list;
    },
    renameList: async (listId, name) => {
      const list = await run(() => desktop.taskRenameList(listId, name));
      if (list) await refresh();
      return list;
    },
    deleteList: async (listId, expectedTaskCount) => {
      const succeeded = await run(async () => {
        await desktop.taskDeleteList(listId, expectedTaskCount);
        return true;
      });
      if (succeeded) await refresh();
      return succeeded === true;
    },
    createTask: async (input) => {
      const task = await run(() => desktop.taskCreate(input));
      if (task) await refresh();
      return task;
    },
    updateTask: async (taskId, input) => {
      const task = await run(() => desktop.taskUpdate(taskId, input));
      if (task) await refresh();
      return task;
    },
    setCompleted: async (taskId, completed) => {
      const task = await run(() => desktop.taskSetCompleted(taskId, completed));
      if (task) await refresh();
      return task;
    },
    deleteTask: async (taskId) => {
      const succeeded = await run(async () => {
        await desktop.taskDelete(taskId);
        return true;
      });
      if (succeeded) await refresh();
      return succeeded === true;
    },
    setPinned: async (taskId) => {
      const current = ++generation.current;
      setBusy(true);
      setError("");
      try {
        const next = await desktop.taskSetPinned(taskId);
        if (current === generation.current) setPinnedTaskId(next);
        return current === generation.current;
      } catch (reason) {
        if (current === generation.current) setError(taskErrorText(reason));
        return false;
      } finally {
        if (current === generation.current) setBusy(false);
      }
    },
  };
}

export interface RepositoryPickerState {
  repositories: Array<{ id: number; fullName: string; htmlUrl: string }>;
  loading: boolean;
  error: string;
  connected: boolean;
  reload: () => void;
}

export function useRepositories(
  native: boolean,
  connected: boolean,
): RepositoryPickerState {
  const [repositories, setRepositories] = useState<
    Array<{ id: number; fullName: string; htmlUrl: string }>
  >([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    if (!native || !connected) {
      setRepositories([]);
      setLoading(false);
      setError("");
      return;
    }
    let active = true;
    setLoading(true);
    setError("");
    desktop
      .githubListRepositories()
      .then((rows) => {
        if (active)
          setRepositories(
            rows.map((row) => ({
              id: row.id,
              fullName: row.fullName,
              htmlUrl: row.htmlUrl,
            })),
          );
      })
      .catch((reason: unknown) => {
        if (active) setError(githubErrorText(reason));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [native, connected, reloadKey]);

  const reload = useCallback(() => {
    setReloadKey((value) => value + 1);
  }, []);

  return {
    repositories,
    loading,
    error,
    connected: native && connected,
    reload,
  };
}