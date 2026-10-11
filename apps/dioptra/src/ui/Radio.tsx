// A list of drawn radio choices, each with a title and an optional one-line hint.
import { useRef, type ReactNode } from 'react';

export interface RadioChoice<T> {
  value: T;
  label: ReactNode;
  hint?: ReactNode;
  testId?: string;
}

export function RadioList<T extends string | number>({
  choices,
  value,
  onChange,
  label,
  testId,
}: {
  choices: RadioChoice<T>[];
  value: T | null;
  onChange: (v: T) => void;
  label: string;
  testId?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const idx = choices.findIndex((c) => c.value === value);
  return (
    <div
      ref={ref}
      className="radio-list"
      role="radiogroup"
      aria-label={label}
      data-testid={testId}
      onKeyDown={(e) => {
        const d = e.key === 'ArrowDown' || e.key === 'ArrowRight' ? 1 : e.key === 'ArrowUp' || e.key === 'ArrowLeft' ? -1 : 0;
        if (!d) return;
        e.preventDefault();
        e.stopPropagation();
        const n = (Math.max(0, idx) + d + choices.length) % choices.length;
        onChange(choices[n]!.value);
        requestAnimationFrame(() => ref.current?.querySelectorAll<HTMLElement>('[role="radio"]')[n]?.focus());
      }}
    >
      {choices.map((c, i) => (
        <RadioRow key={String(c.value)} checked={i === idx} onSelect={() => onChange(c.value)} hint={c.hint} testId={c.testId} tabbable={i === idx || (idx < 0 && i === 0)}>
          {c.label}
        </RadioRow>
      ))}
    </div>
  );
}

/** One radio row, for groups laid out by hand. */
export function RadioRow({
  checked,
  onSelect,
  children,
  hint,
  testId,
  tabbable = true,
}: {
  checked: boolean;
  onSelect: () => void;
  children: ReactNode;
  hint?: ReactNode;
  testId?: string;
  tabbable?: boolean;
}) {
  return (
    <button type="button" role="radio" aria-checked={checked} tabIndex={tabbable ? 0 : -1} className="radio" data-testid={testId} onClick={onSelect}>
      <span className="radio-dot" aria-hidden="true" />
      <span className="radio-text">
        <span className="radio-title">{children}</span>
        {hint && <span className="radio-hint">{hint}</span>}
      </span>
    </button>
  );
}
