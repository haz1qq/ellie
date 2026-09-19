import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { MoreHorizontal } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "./cn";

export interface MenuItem {
  label: string;
  onSelect?: () => void;
  disabled?: boolean;
  danger?: boolean;
  icon?: ReactNode;
}

export interface MenuProps {
  items: MenuItem[];
  /** Accessible name for the trigger button. */
  label: string;
  /** Custom trigger icon (default: horizontal ellipsis). */
  trigger?: ReactNode;
  align?: "start" | "end";
  className?: string;
}

/** Row-action menu built on Radix DropdownMenu with keyboard support. */
export function Menu({ items, label, trigger, align = "end", className }: MenuProps) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <button className={cn("menu-trigger", className)} aria-label={label}>
          {trigger ?? <MoreHorizontal size={16} />}
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className="menu-content" align={align} sideOffset={6}>
          {items.map((item, index) => (
            <DropdownMenu.Item
              key={index}
              className={cn("menu-item", item.danger && "menu-item-danger")}
              disabled={item.disabled}
              onSelect={item.onSelect}
            >
              {item.icon}
              <span>{item.label}</span>
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}