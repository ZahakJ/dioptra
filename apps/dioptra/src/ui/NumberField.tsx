// A number field: mono tabular digits, the unit written inside the field, stepper buttons,
// ArrowUp/ArrowDown by `step` (Shift: ten steps), commit on Enter or blur, Escape reverts.
// Empty means "automatic" when the field allows it (the placeholder says what applies).
import { ChevronDown, ChevronUp } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

export interface NumberFieldProps {
  value: number | null | undefined;
  /** Called with the parsed value when the user commits (Enter, blur, a step). */
  onCommit?: (v: number | null) => void;
  /** Called on every keystroke with the text and its value (NaN when it does not parse). */
  onText?: (text: string, value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  integer?: boolean;
  /** `mm`, `pt`, `dpi`, `%`, `°`: written inside the field, after the digits. */
  unit?: string;
  placeholder?: string;
  /** Empty is allowed and commits null. Default true. */
  nullable?: boolean;
  label?: string;
  labelledBy?: string;
  testId?: string;
  disabled?: boolean;
  size?: 'sm' | 'md';
  /** Marks the field invalid from outside (a dialog's cross-field check). */
  invalid?: boolean;
  className?: string;
  /** Hide the stepper (lists of values, very narrow fields). */
  noStepper?: boolean;
  /** Text shown as is (keeps a value's own formatting); default trims to 6 decimals. */
  format?: (v: number) => string;
}

const show = (x: number | null | undefined, format?: (v: number) => string) =>
  typeof x === 'number' && Number.isFinite(x) ? (format ? format(x) : String(Math.round(x * 1e6) / 1e6)) : '';

export const parseNumber = (s: string): number => (s.trim() === '' ? NaN : Number(s.trim().replace(',', '.')));

function decimalsOf(n: number): number {
  const s = String(n);
  const i = s.indexOf('.');
  return i < 0 ? 0 : s.length - i - 1;
}

export function NumberField({
  value,
  onCommit,
  onText,
  min,
  max,
  step = 1,
  integer,
  unit,
  placeholder,
  nullable = true,
  label,
  labelledBy,
  testId,
  disabled,
  size = 'sm',
  invalid,
  className,
  noStepper,
  format,
}: NumberFieldProps) {
  const [text, setText] = useState(show(value, format));
  const focused = useRef(false);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!focused.current) setText(show(value, format));
  }, [value, format]);

  const n = parseNumber(text);
  const bad =
    text.trim() === ''
      ? !nullable
      : !Number.isFinite(n) || (min !== undefined && n < min) || (max !== undefined && n > max) || (!!integer && !Number.isInteger(n));

  const setT = (t: string) => {
    setText(t);
    onText?.(t, parseNumber(t));
  };
  const commit = () => {
    if (bad) {
      if (onCommit) setT(show(value, format));
      return;
    }
    const v = text.trim() === '' ? null : integer ? Math.round(n) : n;
    if (v === (value ?? null)) return;
    onCommit?.(v);
  };
  const stepBy = (k: number) => {
    const base = Number.isFinite(n) ? n : typeof value === 'number' ? value : (min ?? 0);
    let next = base + k * step;
    const dp = Math.max(decimalsOf(step), decimalsOf(base));
    next = Number(next.toFixed(Math.min(dp, 6)));
    if (min !== undefined) next = Math.max(min, next);
    if (max !== undefined) next = Math.min(max, next);
    if (integer) next = Math.round(next);
    setT(String(next));
    if (next !== (value ?? null)) onCommit?.(next);
  };

  return (
    <span
      className={'numfield numfield-' + size + (bad || invalid ? ' is-invalid' : '') + (disabled ? ' is-disabled' : '') + (className ? ` ${className}` : '')}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) {
          e.preventDefault();
          input.current?.focus();
        }
      }}
    >
      <input
        ref={input}
        className="numfield-input"
        type="text"
        inputMode="decimal"
        autoComplete="off"
        spellCheck={false}
        value={text}
        placeholder={placeholder}
        aria-label={label}
        aria-labelledby={labelledBy}
        aria-invalid={bad || invalid || undefined}
        data-testid={testId}
        disabled={disabled}
        onFocus={() => (focused.current = true)}
        onChange={(e) => setT(e.target.value)}
        onBlur={() => {
          focused.current = false;
          commit();
        }}
        onKeyDown={(e) => {
          if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
            e.preventDefault();
            e.stopPropagation();
            stepBy((e.key === 'ArrowUp' ? 1 : -1) * (e.shiftKey ? 10 : 1));
          } else if (e.key === 'Enter') {
            // Inside a form Enter also submits it (the dialog reads the live text); elsewhere
            // it commits by leaving the field.
            if ((e.target as HTMLInputElement).form) commit();
            else {
              e.preventDefault();
              input.current?.blur();
            }
          } else if (e.key === 'Escape' && onCommit && text !== show(value, format)) {
            // The first Escape reverts the edit; a second one reaches the dialog.
            e.preventDefault();
            e.stopPropagation();
            setT(show(value, format));
          }
        }}
      />
      {unit && (
        <span className="numfield-unit" aria-hidden="true">
          {unit}
        </span>
      )}
      {!noStepper && !disabled && (
        <span className="numfield-steps" aria-hidden="true">
          <button type="button" tabIndex={-1} onMouseDown={(e) => e.preventDefault()} onClick={(e) => stepBy(e.shiftKey ? 10 : 1)}>
            <ChevronUp size={11} strokeWidth={2} />
          </button>
          <button type="button" tabIndex={-1} onMouseDown={(e) => e.preventDefault()} onClick={(e) => stepBy(e.shiftKey ? -10 : -1)}>
            <ChevronDown size={11} strokeWidth={2} />
          </button>
        </span>
      )}
    </span>
  );
}
