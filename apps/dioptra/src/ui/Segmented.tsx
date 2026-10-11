// A segmented control for small enums (Cell/Column, In/Out/Cross, light/dark/system): a radio
// group drawn as one strip, arrow keys move the choice, the chosen segment is raised.
import { useRef, type ReactNode } from 'react';

export interface SegmentedChoice<T> {
  value: T;
  label: ReactNode;
  /** Accessible name when the label is an icon. */
  aria?: string;
  /** Tooltip. */
  hint?: string;
  testId?: string;
}

const keyOf = (v: unknown) => JSON.stringify(v);

export function Segmented<T>({
  choices,
  value,
  onChange,
  label,
  labelledBy,
  testId,
  size = 'sm',
  className,
  disabled,
}: {
  choices: SegmentedChoice<T>[];
  value: T;
  onChange: (v: T) => void;
  label?: string;
  labelledBy?: string;
  testId?: string;
  size?: 'sm' | 'md';
  className?: string;
  disabled?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const idx = choices.findIndex((c) => keyOf(c.value) === keyOf(value));
  return (
    <div
      ref={ref}
      className={'seg seg-' + size + (className ? ` ${className}` : '')}
      role="radiogroup"
      aria-label={label}
      aria-labelledby={labelledBy}
      aria-disabled={disabled || undefined}
      data-testid={testId}
      onKeyDown={(e) => {
        const d = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
        if (!d || disabled) return;
        e.preventDefault();
        e.stopPropagation();
        const n = (Math.max(0, idx) + d + choices.length) % choices.length;
        onChange(choices[n]!.value);
        requestAnimationFrame(() => (ref.current?.querySelectorAll<HTMLElement>('[role="radio"]')[n])?.focus());
      }}
    >
      {choices.map((c, i) => {
        const on = i === idx;
        return (
          <button
            key={keyOf(c.value)}
            type="button"
            role="radio"
            aria-checked={on}
            aria-label={c.aria}
            tabIndex={on || (idx < 0 && i === 0) ? 0 : -1}
            className="seg-btn"
            data-testid={c.testId}
            data-tip={c.hint}
            disabled={disabled}
            onClick={() => onChange(c.value)}
          >
            {c.label}
          </button>
        );
      })}
    </div>
  );
}
