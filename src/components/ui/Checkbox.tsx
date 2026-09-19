import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";
import { cn } from "./cn";

export interface CheckboxProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  /** Accessible name, e.g. "Mark Task title complete". */
  label: string;
  disabled?: boolean;
  className?: string;
}

/** Accessible checkbox built on Radix, used for task completion. */
export function Checkbox({ checked, onCheckedChange, label, disabled, className }: CheckboxProps) {
  return (
    <CheckboxPrimitive.Root
      className={cn("checkbox", checked && "checkbox-checked", className)}
      checked={checked}
      onCheckedChange={(next) => onCheckedChange(next === true)}
      disabled={disabled}
      aria-label={label}
    >
      <CheckboxPrimitive.Indicator className="checkbox-indicator">
        <Check size={12} strokeWidth={3} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}