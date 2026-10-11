// Modal dialogs as promises: `await confirmDialog(...)`, `await promptDialog(...)`.
import { useState, type ReactNode } from 'react';
import { create } from 'zustand';
import { Button, DialogFrame, DialogHeader, RadioList, TextInput } from '../ui';

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

export { DialogFooter, DialogHeader } from '../ui';

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
          <Button onClick={() => close(false)}>Cancel</Button>
          <Button type="submit" variant={opts.danger ? 'danger' : 'primary'} data-autofocus>
            {opts.confirm}
          </Button>
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
        <TextInput mono={opts.mono} value={value} onChange={(e) => setValue(e.target.value)} invalid={!!error} />
        {error ? <span className="field-error">{error}</span> : opts.hint && <span className="field-hint">{opts.hint}</span>}
      </label>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!!error}>
          {opts.confirm ?? 'OK'}
        </Button>
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
      <div className="field">
        <span className="field-label">{opts.label}</span>
        <RadioList label={opts.label} value={value} onChange={setValue} choices={opts.options.map((o) => ({ value: o.value, label: o.label, hint: o.hint }))} />
      </div>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary">
          {opts.confirm ?? 'OK'}
        </Button>
      </footer>
    </form>
  );
}
