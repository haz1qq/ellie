/* eslint-disable react-refresh/only-export-components */
import * as Dialog from "@radix-ui/react-dialog";
import {
  Activity,
  BarChart3,
  GitFork,
  LayoutDashboard,
  ListTodo,
  RefreshCw,
  Search,
  Settings as SettingsIcon,
  Sparkles,
  SquarePen,
  FolderPlus,
  ArrowUpRight,
} from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { copy } from "../../copy";
import { isView, type View } from "../../lib/desktop";
import { cn } from "../ui/cn";
import { Toaster } from "../ui/Toaster";

export interface NavItem {
  view: View;
  label: string;
  icon: ReactNode;
}

export const NAV_ITEMS: NavItem[] = [
  { view: "dashboard", label: "Overview", icon: <LayoutDashboard size={16} /> },
  { view: "ai-usage", label: "AI Usage", icon: <Activity size={16} /> },
  { view: "github", label: "GitHub", icon: <GitFork size={16} /> },
  { view: "todos", label: "To-do", icon: <ListTodo size={16} /> },
  { view: "history", label: "History", icon: <BarChart3 size={16} /> },
  { view: "settings", label: "Settings", icon: <SettingsIcon size={16} /> },
];

const VIEW_META: Record<View, { kicker: string; title: string }> = {
  dashboard: { kicker: "Command center", title: "Overview" },
  "ai-usage": { kicker: "Allowances & tokens", title: "AI Usage" },
  github: { kicker: "Connected work", title: "GitHub" },
  todos: { kicker: "Local lists", title: "To-do" },
  history: { kicker: "Local analytics", title: "History" },
  settings: { kicker: "Local controls", title: "Settings" },
};

export interface AppShellProps {
  view: View;
  onNavigate: (view: View) => void;
  friendly: boolean;
  native: boolean;
  refreshing: boolean;
  githubConnected: boolean;
  /** Opens the new-task dialog on the To-do view. */
  onNewTask: () => void;
  /** Opens the repository creation dialog on the GitHub view. */
  onNewRepository: () => void;
  onRefreshAll: () => void;
  children: ReactNode;
}

export function AppShell({
  view,
  onNavigate,
  friendly,
  native,
  refreshing,
  githubConnected,
  onNewTask,
  onNewRepository,
  onRefreshAll,
  children,
}: AppShellProps) {
  const meta = VIEW_META[view];

  return (
    <div className="shell">
      <a href="#main-content" className="skip-link">
        Skip to main content
      </a>
      <aside className="rail">
        <div className="rail-brand">
          <span className="rail-mark" aria-hidden="true">
            <Sparkles size={15} />
          </span>
          <div className="rail-brand-copy">
            <strong className="rail-wordmark">
              ellie<span className="pink">.</span>
            </strong>
            <span className="rail-caption">Personal workspace</span>
          </div>
        </div>
        <nav className="rail-nav" aria-label="Main navigation">
          {NAV_ITEMS.map((item) => (
            <button
              key={item.view}
              className={cn("rail-nav-item", view === item.view && "rail-nav-active")}
              aria-current={view === item.view ? "page" : undefined}
              onClick={() => onNavigate(item.view)}
            >
              <span className="rail-nav-icon" aria-hidden="true">
                {item.icon}
              </span>
              <span>{item.label}</span>
            </button>
          ))}
        </nav>
        <div className="rail-footer">
          <span className="rail-local">
            <span className="local-dot" aria-hidden="true" /> Local to this device
          </span>
          <span className="rail-version">Private by design</span>
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <div className="topbar-title">
            <span className="topbar-kicker">{meta.kicker}</span>
            <strong>{meta.title}</strong>
          </div>
          <div className="topbar-actions">
            <CommandPalette
              onNavigate={onNavigate}
              onNewTask={onNewTask}
              onNewRepository={onNewRepository}
              onRefreshAll={onRefreshAll}
              refreshing={refreshing}
              native={native}
              githubConnected={githubConnected}
            />
            <span className="topbar-state">
              <span className="local-dot" aria-hidden="true" /> Private workspace
            </span>
          </div>
        </header>
        <main id="main-content" className="workspace-main">
          {!native && (
            <div className="banner">
              <strong>Browser preview</strong>
              <span>
                Desktop settings, tray controls, GitHub sign-in, and task
                persistence are available in the Windows app only.
              </span>
            </div>
          )}
          {children}
        </main>
        <footer className="workspace-footer">
          <span>{friendly ? copy.quiet : "Ellie · Local-first AI usage monitor"}</span>
          <span>Trust the number.</span>
        </footer>
      </div>
      <Toaster />
    </div>
  );
}

interface CommandItem {
  id: string;
  label: string;
  hint?: string;
  icon?: ReactNode;
  onSelect: () => void;
  disabled?: boolean;
}

function CommandPalette({
  onNavigate,
  onNewTask,
  onNewRepository,
  onRefreshAll,
  refreshing,
  native,
  githubConnected,
}: {
  onNavigate: (view: View) => void;
  onNewTask: () => void;
  onNewRepository: () => void;
  onRefreshAll: () => void;
  refreshing: boolean;
  native: boolean;
  githubConnected: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");

  const items = useMemo<CommandItem[]>(() => {
    const navigation: CommandItem[] = NAV_ITEMS.map((item) => ({
      id: `nav-${item.view}`,
      label: `Open ${item.label}`,
      hint: item.label,
      icon: item.icon,
      onSelect: () => onNavigate(item.view),
    }));
    const actions: CommandItem[] = [
      {
        id: "new-task",
        label: "Add a new task",
        hint: "To-do",
        icon: <SquarePen size={15} />,
        onSelect: onNewTask,
      },
      {
        id: "new-repository",
        label: "Create a repository",
        hint: "GitHub",
        icon: <FolderPlus size={15} />,
        onSelect: onNewRepository,
        disabled: !native || !githubConnected,
      },
      {
        id: "refresh",
        label: refreshing ? "Refreshing providers…" : "Refresh AI providers",
        hint: "Providers",
        icon: <RefreshCw size={15} />,
        onSelect: onRefreshAll,
        disabled: refreshing || !native,
      },
    ];
    return [...navigation, ...actions];
  }, [
    onNavigate,
    onNewTask,
    onNewRepository,
    onRefreshAll,
    refreshing,
    native,
    githubConnected,
  ]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setOpen((current) => !current);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const visible = items.filter((item) => {
    if (!query.trim()) return true;
    const needle = query.toLowerCase();
    return (
      item.label.toLowerCase().includes(needle) ||
      (item.hint ?? "").toLowerCase().includes(needle)
    );
  });

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>
        <button className="command-trigger" aria-label="Quick actions (Ctrl+K)">
          <Search size={14} />
          <span>Quick actions</span>
          <kbd>Ctrl K</kbd>
        </button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="command-overlay" />
        <Dialog.Content className="command-dialog" onOpenAutoFocus={(event) => event.preventDefault()}>
          <Dialog.Title className="visually-hidden">Quick actions</Dialog.Title>
          <div className="command-input-row">
            <Search size={15} className="command-search-icon" aria-hidden="true" />
            <input
              className="command-input"
              placeholder="Search views and actions…"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              aria-label="Search views and actions"
            />
            <Dialog.Close asChild>
              <button className="command-esc" aria-label="Close quick actions">
                Esc
              </button>
            </Dialog.Close>
          </div>
          <div className="command-list" role="listbox" aria-label="Commands">
            {visible.length === 0 && <p className="command-empty">No matches.</p>}
            {visible.map((item) => (
              <button
                key={item.id}
                role="option"
                aria-selected={false}
                className="command-item"
                disabled={item.disabled}
                onClick={() => {
                  const next = item.onSelect();
                  setOpen(false);
                  setQuery("");
                  void next;
                }}
              >
                <span className="command-item-icon" aria-hidden="true">
                  {item.icon}
                </span>
                <span>{item.label}</span>
                <span className="command-item-hint">
                  {item.hint} <ArrowUpRight size={11} />
                </span>
              </button>
            ))}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function useSafeNavigate(
  setView: (view: View) => void,
): (view: View | unknown) => void {
  return (viewOrValue) => {
    if (isView(viewOrValue)) setView(viewOrValue);
  };
}