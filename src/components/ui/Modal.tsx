import * as Dialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "./cn";

export interface ModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
  /** Width variant; "lg" for forms with side-by-side fields. */
  width?: "md" | "lg";
  /** Called when the dialog fully closes; useful for un-mounting form state. */
  onClose?: () => void;
}

/**
 * Accessible modal built on Radix Dialog: focus trap, labelled by title,
 * Esc to close, restored focus, and portal rendering.
 */
export function Modal({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  width = "md",
  onClose,
}: ModalProps) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="modal-overlay" />
        <Dialog.Content
          className={cn("modal", width === "lg" && "modal-lg")}
          aria-describedby={description ? undefined : undefined}
          onCloseAutoFocus={onClose ? () => onClose() : undefined}
        >
          <div className="modal-header">
            <div>
              <Dialog.Title className="modal-title">{title}</Dialog.Title>
              {description && (
                <Dialog.Description className="modal-description">
                  {description}
                </Dialog.Description>
              )}
            </div>
            <Dialog.Close asChild>
              <button className="modal-close" aria-label="Close dialog">
                <X size={16} />
              </button>
            </Dialog.Close>
          </div>
          <div className="modal-body">{children}</div>
          {footer && <div className="modal-footer">{footer}</div>}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function ModalActions({ children }: { children: ReactNode }) {
  return <div className="modal-actions">{children}</div>;
}