import { useEffect, useId, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";

type DialogProps = {
  title: string;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
};

const FOCUSABLE = "button, [href], input, select, textarea, [tabindex]:not([tabindex='-1'])";

export function Dialog({ title, onClose, children, wide = false }: DialogProps) {
  const titleId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    const previouslyFocused = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;

    const root = rootRef.current;
    const body = root?.querySelector<HTMLElement>(".dialog-body");
    const initial =
      body?.querySelector<HTMLElement>(FOCUSABLE) ??
      root?.querySelector<HTMLElement>(FOCUSABLE);
    initial?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onCloseRef.current();
        return;
      }

      if (event.key !== "Tab" || root === null) {
        return;
      }

      const nodes = [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
        (node) => !node.hasAttribute("disabled") && node.getAttribute("aria-hidden") !== "true",
      );
      if (nodes.length === 0) {
        return;
      }

      const first = nodes[0];
      const last = nodes[nodes.length - 1];
      if (first === undefined || last === undefined) {
        return;
      }

      const active = document.activeElement;
      const activeIsTrapped = active instanceof HTMLElement && nodes.includes(active);
      if (!activeIsTrapped) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
        return;
      }

      if (event.shiftKey && active === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
    }

    const shell = document.querySelector(".app-shell");
    shell?.setAttribute("inert", "");

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      shell?.removeAttribute("inert");
      previouslyFocused?.focus();
    };
  }, []);

  return createPortal(
    <div className="dialog-backdrop">
      <div
        ref={rootRef}
        className={wide ? "dialog dialog-wide" : "dialog"}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <div className="dialog-header">
          <h2 id={titleId}>{title}</h2>
          <button type="button" className="ghost-button" onClick={onClose}>
            Close
          </button>
        </div>
        {children}
      </div>
    </div>,
    document.body,
  );
}
