import { Toaster as SonnerToaster } from "sonner";

/** App-wide toast surface. Tiny, quiet, respects reduced-motion via CSS. */
export function Toaster() {
  return (
    <SonnerToaster
      theme="dark"
      position="bottom-right"
      toastOptions={{
        classNames: {
          toast: "toast",
          description: "toast-description",
        },
      }}
    />
  );
}