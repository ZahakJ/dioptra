// Small form controls for the inspector: text fields that commit on Enter or blur.
import { useEffect, useState } from 'react';

export function CommitField({
  value,
  onCommit,
  label,
  mono,
  placeholder,
  disabled,
  testId,
  validate,
}: {
  value: string;
  onCommit: (v: string) => void | Promise<unknown>;
  label: string;
  mono?: boolean;
  placeholder?: string;
  disabled?: boolean;
  testId?: string;
  validate?: (v: string) => string | null;
}) {
  const [v, setV] = useState(value);
  const [editing, setEditing] = useState(false);
  useEffect(() => {
    if (!editing) setV(value);
  }, [value, editing]);
  const error = editing ? (validate?.(v) ?? null) : null;
  const commit = () => {
    setEditing(false);
    if (v !== value && !validate?.(v)) void onCommit(v);
    else setV(value);
  };
  return (
    <label className="kv-field">
      <span className="kv-label">{label}</span>
      <input
        className={'input input-compact' + (mono ? ' mono' : '')}
        value={v}
        placeholder={placeholder}
        disabled={disabled}
        spellCheck={false}
        aria-invalid={!!error}
        title={error ?? undefined}
        data-testid={testId}
        onFocus={() => setEditing(true)}
        onChange={(e) => {
          setEditing(true);
          setV(e.target.value);
        }}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            (e.target as HTMLInputElement).blur();
          } else if (e.key === 'Escape') {
            e.preventDefault();
            setV(value);
            setEditing(false);
            requestAnimationFrame(() => (e.target as HTMLInputElement).blur());
          }
          e.stopPropagation();
        }}
      />
      {error && <span className="field-error">{error}</span>}
    </label>
  );
}

export function KV({ label, children, mono }: { label: string; children: React.ReactNode; mono?: boolean }) {
  return (
    <div className="kv">
      <span className="kv-label">{label}</span>
      <span className={'kv-value' + (mono ? ' mono' : '')}>{children}</span>
    </div>
  );
}
