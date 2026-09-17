import { useEffect, useId, useRef } from "react";

function focusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  );
}

function trapTab(event: React.KeyboardEvent, container: HTMLElement) {
  const elements = focusableElements(container);
  if (elements.length === 0) return;
  const first = elements[0]!;
  const last = elements[elements.length - 1]!;
  const active = document.activeElement;
  if (event.shiftKey) {
    if (active === first || active === null || !container.contains(active)) {
      event.preventDefault();
      last.focus();
    }
  } else if (active === last || active === null || !container.contains(active)) {
    event.preventDefault();
    first.focus();
  }
}

export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  body: string;
  confirmLabel: string;
  cancelLabel: string;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Keyboard-operable confirmation dialog. Focus moves into the dialog on open,
 * Tab is trapped inside, Escape cancels, and focus returns to the element that
 * opened it on close. Rendered inline (not portaled) inside its section.
 */
export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel,
  cancelLabel,
  busy,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const dialogRef = useRef<HTMLDivElement | null>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const titleId = useId();
  const bodyId = useId();

  useEffect(() => {
    if (!open) return;
    const active = document.activeElement;
    previousFocus.current = active instanceof HTMLElement ? active : null;
    const dialog = dialogRef.current;
    if (dialog) {
      (focusableElements(dialog)[0] ?? dialog).focus();
    }
    return () => {
      previousFocus.current?.focus();
    };
  }, [open]);

  if (!open) return null;

  function handleKeyDown(event: React.KeyboardEvent) {
    const dialog = dialogRef.current;
    if (event.key === "Escape") {
      event.preventDefault();
      onCancel();
      return;
    }
    if (event.key === "Tab" && dialog) {
      trapTab(event, dialog);
    }
  }

  return (
    <div className="dialog-backdrop">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={bodyId}
        className="dialog"
        onKeyDown={handleKeyDown}
      >
        <h2 id={titleId}>{title}</h2>
        <p id={bodyId}>{body}</p>
        <div className="save-row dialog-actions">
          <button type="button" className="primary" onClick={onConfirm} disabled={busy}>
            {confirmLabel}
          </button>
          <button type="button" onClick={onCancel} disabled={busy}>
            {cancelLabel}
          </button>
        </div>
      </div>
    </div>
  );
}