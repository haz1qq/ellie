import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { desktop, type TaskItem } from "../../lib/desktop";
import TaskStickyNote from "./TaskStickyNote";

vi.mock("../../lib/desktop", () => ({
  desktop: {
    available: vi.fn(() => true),
    onTaskNoteUpdated: vi.fn(),
    taskNoteBootstrap: vi.fn(),
    taskNoteComplete: vi.fn(),
    taskNoteUnpin: vi.fn(),
    openMainWindow: vi.fn(),
  },
}));

const task: TaskItem = {
  id: 11,
  listId: 1,
  title: "Redesign Ellie",
  notes: "Polish the task board before launch.",
  kind: "work",
  priority: "high",
  dueDate: "2026-09-27",
  repository: {
    repositoryId: 42,
    fullName: "haz1qq/ellie",
    htmlUrl: "https://github.com/haz1qq/ellie",
  },
  completedAt: null,
  createdAt: "2026-09-01T00:00:00Z",
  updatedAt: "2026-09-01T00:00:00Z",
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(desktop.available).mockReturnValue(true);
  vi.mocked(desktop.taskNoteBootstrap).mockResolvedValue(task);
  vi.mocked(desktop.onTaskNoteUpdated).mockResolvedValue(() => {});
  vi.mocked(desktop.taskNoteComplete).mockResolvedValue({
    ...task,
    completedAt: "2026-09-20T10:00:00Z",
  });
  vi.mocked(desktop.taskNoteUnpin).mockResolvedValue(undefined);
  vi.mocked(desktop.openMainWindow).mockResolvedValue(undefined);
});

describe("TaskStickyNote", () => {
  it("renders only the pinned task and can restore Ellie", async () => {
    const user = userEvent.setup();
    render(<TaskStickyNote />);
    expect(await screen.findByRole("heading", { name: "Redesign Ellie" })).toBeVisible();
    expect(screen.getByText("Work")).toBeVisible();
    expect(screen.getByText("haz1qq/ellie")).toBeVisible();
    expect(screen.getByText("Polish the task board before launch.")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Open Ellie" }));
    expect(desktop.openMainWindow).toHaveBeenCalledTimes(1);
  });

  it("completes the one pinned task through the restricted command", async () => {
    const user = userEvent.setup();
    render(<TaskStickyNote />);
    await screen.findByRole("heading", { name: "Redesign Ellie" });
    await user.click(screen.getByRole("button", { name: "Complete" }));
    await waitFor(() => expect(desktop.taskNoteComplete).toHaveBeenCalledTimes(1));
  });

  it("unpins and closes through the restricted command", async () => {
    const user = userEvent.setup();
    render(<TaskStickyNote />);
    await screen.findByRole("heading", { name: "Redesign Ellie" });
    await user.click(screen.getByRole("button", { name: "Unpin and close sticky note" }));
    await waitFor(() => expect(desktop.taskNoteUnpin).toHaveBeenCalledTimes(1));
  });
});
