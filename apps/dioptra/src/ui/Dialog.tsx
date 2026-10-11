// Dialogs: a modal surface over a scrim, focus trapped inside, Escape cancels, focus returns to
// where it was. A designed header (title, one sentence, close button) and footer.
import { X } from 'lucide-react';
import { useEffect, useRef, type ReactNode } from 'react';

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), [tabindex="0"]';

export function DialogFrame({
  children,
  onCancel,
  label,
  testId,
}: {
  children: ReactNode;
  onCancel: () => void;
  label?: string;
  testId?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    const el = ref.current;
    const first =
      el?.querySelector<HTMLElement>('[data-autofocus]') ??
      el?.querySelector<HTMLElement>('input:not([disabled]), textarea:not([disabled])') ??
      el?.querySelector<HTMLElement>('.modal-foot .btn-primary:not([disabled])') ??
      el?.querySelector<HTMLElement>(FOCUSABLE);
    first?.focus();
    if (first instanceof HTMLInputElement) first.select();
    return () => prev?.focus?.();
  }, []);
  return (
    <div className="modal-scrim" onMouseDown={(e) => e.target === e.currentTarget && onCancel()}>
      <div
        ref={ref}
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label={label}
        data-testid={testId}
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            e.stopPropagation();
            onCancel();
          }
          if (e.key === 'Tab') trapFocus(e, ref.current);
          e.stopPropagation();
        }}
      >
        {children}
      </div>
    </div>
  );
}

export function trapFocus(e: React.KeyboardEvent, root: HTMLElement | null) {
  if (!root) return;
  const items = [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((x) => x.offsetParent !== null || x === document.activeElement);
  if (!items.length) return;
  const first = items[0]!;
  const last = items[items.length - 1]!;
  if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}

export function DialogHeader({ title, sub, onClose }: { title: string; sub?: ReactNode; onClose?: () => void }) {
  return (
    <header className="modal-head">
      <div className="modal-head-text">
        <h2>{title}</h2>
        {sub && <p>{sub}</p>}
      </div>
      {onClose && (
        <button type="button" className="btn btn-quiet btn-icon modal-x" aria-label="Close" data-tip="Close" data-tip-kbd="Escape" onClick={onClose}>
          <X size={16} strokeWidth={1.5} aria-hidden="true" />
        </button>
      )}
    </header>
  );
}

export function DialogFooter({ children, note }: { children: ReactNode; note?: ReactNode }) {
  return (
    <footer className="modal-foot">
      {note && <span className="foot-note">{note}</span>}
      {children}
    </footer>
  );
}
