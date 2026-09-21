import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";
import { cn } from "./cn";

export interface CheckboxProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  /** Accessible name, e.g. "Mark Task title complete". */
  label: string;
  id?: string;
  disabled?: boolean;
  className?: string;
}

/** Accessible shared checkbox for tasks, settings, and creation forms. */
export function Checkbox({
  checked,
  onCheckedChange,
  label,
  id,
  disabled,
  className,
}: CheckboxProps) {
  return (
    <CheckboxPrimitive.Root
      id={id}
      className={cn("checkbox", className)}
      checked={checked}
      onCheckedChange={(next) => onCheckedChange(next === true)}
      disabled={disabled}
      aria-label={label}
    >
      <CheckboxPrimitive.Indicator className="checkbox-indicator">
        <Check size={13} strokeWidth={3.2} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}