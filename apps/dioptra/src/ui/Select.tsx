// A select: a button showing the chosen option and a popover list. Keyboard: arrows, Home and
// End, typeahead, Enter or Space to choose, Escape to close; it closes on an outside press.
// Options can carry a colour swatch, a strip of swatches or a secondary label.
import { Check, ChevronsUpDown } from 'lucide-react';
import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { Popover } from './Popover';

export interface SelectOption<T extends string | number> {
  value: T;
  label: string;
  /** A second, quieter label on the right (a unit, a count, a hint). */
  secondary?: string;
  /** A colour shown before the label. */
  swatch?: string;
  /** Several colours (a palette) shown after the label. */
  swatches?: string[];
  /** One sentence shown under the label in the list. */
  hint?: string;
  disabled?: boolean;
}

export function Select<T extends string | number>({
  value,
  options,
  onChange,
  label,
  labelledBy,
  testId,
  disabled,
  size = 'sm',
  placeholder = 'Choose',
  className,
  display,
  tipText,
}: {
  value: T;
  options: SelectOption<T>[];
  onChange: (v: T) => void;
  /** Accessible name (or use `labelledBy`). */
  label?: string;
  labelledBy?: string;
  testId?: string;
  disabled?: boolean;
  size?: 'sm' | 'md';
  placeholder?: string;
  className?: string;
  /** What the closed button shows, when not the option's label. */
  display?: ReactNode;
  /** Tooltip on the closed button. */
  tipText?: string;
}) {
  const [open, setOpen] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);
  const current = options.find((o) => o.value === value);
  const close = (refocus = true) => {
    setOpen(false);
    if (refocus) requestAnimationFrame(() => btn.current?.focus());
  };
  return (
    <>
      <button
        ref={btn}
        type="button"
        className={'select select-' + size + (open ? ' is-open' : '') + (className ? ` ${className}` : '')}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={label}
        aria-labelledby={labelledBy}
        data-testid={testId}
        data-value={String(value)}
        data-tip={tipText}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        {current?.swatch && <span className="sw" style={{ background: current.swatch }} aria-hidden="true" />}
        <span className={'select-value' + (current ? '' : ' is-placeholder')}>{display ?? current?.label ?? placeholder}</span>
        {!display && current?.swatches && <Strip colours={current.swatches} />}
        <ChevronsUpDown className="select-chev" size={14} strokeWidth={1.5} aria-hidden="true" />
      </button>
      {open && (
        <Popover anchor={btn.current} open onClose={() => close(false)} matchWidth className="select-pop">
          <SelectList
            options={options}
            value={value}
            label={label}
            labelledBy={labelledBy}
            testId={testId}
            onPick={(v) => {
              close();
              if (v !== value) onChange(v);
            }}
            onCancel={() => close()}
          />
        </Popover>
      )}
    </>
  );
}

function Strip({ colours }: { colours: string[] }) {
  return (
    <span className="sw-strip" aria-hidden="true">
      {colours.slice(0, 8).map((c, i) => (
        <i key={i} style={{ background: c }} />
      ))}
    </span>
  );
}

function SelectList<T extends string | number>({
  options,
  value,
  label,
  labelledBy,
  testId,
  onPick,
  onCancel,
}: {
  options: SelectOption<T>[];
  value: T;
  label?: string;
  labelledBy?: string;
  testId?: string;
  onPick: (v: T) => void;
  onCancel: () => void;
}) {
  const id = useId();
  const ref = useRef<HTMLDivElement>(null);
  const enabled = options.map((o, i) => (o.disabled ? -1 : i)).filter((i) => i >= 0);
  const [active, setActive] = useState(() => Math.max(0, options.findIndex((o) => o.value === value)));
  const typed = useRef({ text: '', at: 0 });
  useEffect(() => ref.current?.focus(), []);
  useEffect(() => {
    ref.current?.querySelector(`[data-idx="${active}"]`)?.scrollIntoView?.({ block: 'nearest' });
  }, [active]);
  const move = (d: number) => {
    const pos = enabled.indexOf(active);
    const next = enabled[Math.max(0, Math.min(enabled.length - 1, (pos < 0 ? 0 : pos) + d))];
    if (next !== undefined) setActive(next);
  };
  return (
    <div
      ref={ref}
      className="select-list"
      role="listbox"
      tabIndex={-1}
      aria-label={label}
      aria-labelledby={labelledBy}
      aria-activedescendant={`${id}-${active}`}
      onKeyDown={(e) => {
        if (e.key === 'ArrowDown') move(1);
        else if (e.key === 'ArrowUp') move(-1);
        else if (e.key === 'PageDown') move(8);
        else if (e.key === 'PageUp') move(-8);
        else if (e.key === 'Home') setActive(enabled[0] ?? 0);
        else if (e.key === 'End') setActive(enabled[enabled.length - 1] ?? 0);
        else if (e.key === 'Enter' || e.key === ' ') {
          const o = options[active];
          if (o && !o.disabled) onPick(o.value);
        } else if (e.key === 'Escape') onCancel();
        else if (e.key === 'Tab') {
          onCancel();
          return;
        } else if (e.key.length === 1) {
          const now = Date.now();
          const t = typed.current;
          t.text = (now - t.at > 600 ? '' : t.text) + e.key.toLowerCase();
          t.at = now;
          const order = [...enabled.slice(enabled.indexOf(active)), ...enabled.slice(0, enabled.indexOf(active))];
          const from = t.text.length > 1 ? order : [...order.slice(1), ...order.slice(0, 1)];
          const hit = from.find((i) => options[i]!.label.toLowerCase().startsWith(t.text));
          if (hit !== undefined) setActive(hit);
        } else return;
        e.preventDefault();
        e.stopPropagation();
      }}
    >
      {options.map((o, i) => (
        <div
          key={String(o.value)}
          id={`${id}-${i}`}
          data-idx={i}
          role="option"
          aria-selected={o.value === value}
          aria-disabled={o.disabled || undefined}
          data-testid={testId ? `${testId}-option-${o.value}` : undefined}
          className={'select-option' + (i === active ? ' is-active' : '') + (o.disabled ? ' is-disabled' : '')}
          onMouseMove={() => !o.disabled && setActive(i)}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => !o.disabled && onPick(o.value)}
        >
          <span className="select-check" aria-hidden="true">
            {o.value === value && <Check size={14} strokeWidth={2} />}
          </span>
          {o.swatch && <span className="sw" style={{ background: o.swatch }} aria-hidden="true" />}
          <span className="select-option-text">
            <span className="select-option-label">{o.label}</span>
            {o.hint && <span className="select-option-hint">{o.hint}</span>}
          </span>
          {o.swatches && <Strip colours={o.swatches} />}
          {o.secondary && <span className="select-secondary">{o.secondary}</span>}
        </div>
      ))}
    </div>
  );
}
