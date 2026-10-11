// Modal dialogs as promises: `await confirmDialog(...)`, `await promptDialog(...)`.
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { create } from 'zustand';

interface DialogEntry {
  id: number;
  node: ReactNode;
}

interface DialogState {
  stack: DialogEntry[];
}

export const useDialogs = create<DialogState>(() => ({ stack: [] }));
let nextId = 1;

/** Open a dialog; `render` receives `close(value)`. Resolves with the value. */
export function openDialog<T>(render: (close: (value: T) => void) => ReactNode, cancelValue: T): Promise<T> {
  return new Promise<T>((resolve) => {
    const id = nextId++;
    let done = false;
    const close = (value: T) => {
      if (done) return;
      done = true;
      useDialogs.setState((s) => ({ stack: s.stack.filter((d) => d.id !== id) }));
      resolve(value);
    };
    const node = (
      <DialogFrame key={id} onCancel={() => close(cancelValue)}>
        {render(close)}
      </DialogFrame>
    );
    useDialogs.setState((s) => ({ stack: [...s.stack, { id, node }] }));
  });
}

export function DialogHost() {
  const stack = useDialogs((s) => s.stack);
  return <>{stack.map((d) => d.node)}</>;
}

function DialogFrame({ children, onCancel }: { children: ReactNode; onCancel: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    const el = ref.current;
    const first = el?.querySelector<HTMLElement>('[data-autofocus], input, select, textarea, button');
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

function trapFocus(e: React.KeyboardEvent, root: HTMLElement | null) {
  if (!root) return;
  const items = [...root.querySelectorAll<HTMLElement>('button, input, select, textarea, [tabindex="0"]')].filter(
    (x) => !x.hasAttribute('disabled'),
  );
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

export function DialogHeader({ title, sub }: { title: string; sub?: string }) {
  return (
    <header className="modal-head">
      <h2>{title}</h2>
      {sub && <p>{sub}</p>}
    </header>
  );
}

export function confirmDialog(opts: {
  title: string;
  message: string;
  confirm: string;
  danger?: boolean;
}): Promise<boolean> {
  return openDialog<boolean>(
    (close) => (
      <form
        className="modal-body modal-narrow"
        data-testid="confirm-dialog"
        onSubmit={(e) => {
          e.preventDefault();
          close(true);
        }}
      >
        <DialogHeader title={opts.title} />
        <p className="modal-text">{opts.message}</p>
        <footer className="modal-foot">
          <button type="button" className="btn" onClick={() => close(false)}>
            Cancel
          </button>
          <button type="submit" className={opts.danger ? 'btn btn-danger' : 'btn btn-primary'} data-autofocus>
            {opts.confirm}
          </button>
        </footer>
      </form>
    ),
    false,
  );
}

export function promptDialog(opts: {
  title: string;
  label: string;
  initial?: string;
  hint?: string;
  mono?: boolean;
  confirm?: string;
  validate?: (v: string) => string | null;
}): Promise<string | null> {
  return openDialog<string | null>((close) => <PromptBody opts={opts} close={close} />, null);
}

function PromptBody({
  opts,
  close,
}: {
  opts: Parameters<typeof promptDialog>[0];
  close: (v: string | null) => void;
}) {
  const [value, setValue] = useState(opts.initial ?? '');
  const error = opts.validate?.(value) ?? null;
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="prompt-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        if (!error) close(value);
      }}
    >
      <DialogHeader title={opts.title} />
      <label className="field">
        <span className="field-label">{opts.label}</span>
        <input
          className={opts.mono ? 'input mono' : 'input'}
          value={value}
          spellCheck={false}
          onChange={(e) => setValue(e.target.value)}
          aria-invalid={!!error}
        />
        {error ? <span className="field-error">{error}</span> : opts.hint && <span className="field-hint">{opts.hint}</span>}
      </label>
      <footer className="modal-foot">
        <button type="button" className="btn" onClick={() => close(null)}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary" disabled={!!error}>
          {opts.confirm ?? 'OK'}
        </button>
      </footer>
    </form>
  );
}

export function chooseDialog<T extends string>(opts: {
  title: string;
  label: string;
  options: { value: T; label: string; hint?: string }[];
  initial: T;
  confirm?: string;
}): Promise<T | null> {
  return openDialog<T | null>((close) => <ChooseBody opts={opts} close={close} />, null);
}

function ChooseBody<T extends string>({
  opts,
  close,
}: {
  opts: { title: string; label: string; options: { value: T; label: string; hint?: string }[]; initial: T; confirm?: string };
  close: (v: T | null) => void;
}) {
  const [value, setValue] = useState<T>(opts.initial);
  return (
    <form
      className="modal-body modal-narrow"
      onSubmit={(e) => {
        e.preventDefault();
        close(value);
      }}
    >
      <DialogHeader title={opts.title} />
      <fieldset className="choice-list">
        <legend className="field-label">{opts.label}</legend>
        {opts.options.map((o) => (
          <label key={o.value} className="choice">
            <input type="radio" name="choice" checked={value === o.value} onChange={() => setValue(o.value)} />
            <span>
              <span className="choice-title">{o.label}</span>
              {o.hint && <span className="choice-hint">{o.hint}</span>}
            </span>
          </label>
        ))}
      </fieldset>
      <footer className="modal-foot">
        <button type="button" className="btn" onClick={() => close(null)}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary">
          {opts.confirm ?? 'OK'}
        </button>
      </footer>
    </form>
  );
}
