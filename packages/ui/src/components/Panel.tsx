import { useEffect, useRef, type ReactNode } from "react";

/** Non-modal inspector: the conversation remains available while reviewing. */
export function Panel({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  const close = useRef<HTMLButtonElement>(null);
  const dismiss = useRef(onClose);
  dismiss.current = onClose;
  useEffect(() => {
    const previous = document.activeElement;
    close.current?.focus();
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); dismiss.current(); }
    };
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("keydown", escape);
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus();
    };
  }, []);
  return <aside className="inspector" aria-label={title}>
    <div className="inspector-heading"><div><p className="eyebrow">Workspace settings & review</p><h2>{title}</h2></div>
      <button ref={close} onClick={onClose} aria-label="Close" className="close-button">✕</button>
    </div>
    <div className="inspector-body">{children}</div>
  </aside>;
}
