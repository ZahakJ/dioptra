// Inspector controls bound to a graph spec path. Text sends after 150 ms of quiet; numbers
// commit on Enter or blur; selects and switches commit at once.
import { useEffect, useRef, useState } from 'react';
import type { Id, JsonValue } from '../engine/types';
import { setGraphValue, setGraphValueDebounced } from './store';

export function Row({ label, children, hint }: { label: string; children: React.ReactNode; hint?: string }) {
  return (
    <div className="g-row" title={hint}>
      <span className="g-label">{label}</span>
      <span className="g-control">{children}</span>
    </div>
  );
}

export function TextField({
  graph,
  path,
  value,
  label,
  placeholder,
  testId,
  autoFocus,
}: {
  graph: Id;
  path: string;
  value: string | undefined;
  label: string;
  placeholder?: string;
  testId?: string;
  autoFocus?: boolean;
}) {
  const [v, setV] = useState(value ?? '');
  const focused = useRef(false);
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!focused.current) setV(value ?? '');
  }, [value]);
  useEffect(() => {
    if (autoFocus) ref.current?.focus();
  }, [autoFocus]);
  return (
    <Row label={label}>
      <input
        ref={ref}
        className="input input-compact"
        value={v}
        placeholder={placeholder}
        aria-label={label}
        data-testid={testId}
        spellCheck={false}
        onFocus={() => (focused.current = true)}
        onBlur={() => (focused.current = false)}
        onChange={(e) => {
          setV(e.target.value);
          setGraphValueDebounced(graph, path, e.target.value === '' ? null : e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === 'Escape') (e.target as HTMLInputElement).blur();
          e.stopPropagation();
        }}
      />
    </Row>
  );
}

export function NumField({
  graph,
  path,
  value,
  label,
  placeholder = 'Auto',
  min,
  max,
  integer,
  unit,
  testId,
  onCommit,
}: {
  graph: Id;
  path: string;
  value: number | undefined | null;
  label: string;
  placeholder?: string;
  min?: number;
  max?: number;
  integer?: boolean;
  unit?: string;
  testId?: string;
  /** Instead of patching `path` (e.g. to send an array). */
  onCommit?: (v: number | null) => void;
}) {
  const show = (x: number | undefined | null) => (typeof x === 'number' && Number.isFinite(x) ? String(Math.round(x * 1e6) / 1e6) : '');
  const [v, setV] = useState(show(value));
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setV(show(value));
  }, [value]);
  const bad = (() => {
    if (v.trim() === '') return false;
    const n = Number(v.replace(',', '.'));
    return !Number.isFinite(n) || (min !== undefined && n < min) || (max !== undefined && n > max);
  })();
  const commit = () => {
    focused.current = false;
    if (bad) return setV(show(value));
    const n = v.trim() === '' ? null : Number(v.replace(',', '.'));
    const val = n === null ? null : integer ? Math.round(n) : n;
    if (val === (value ?? null)) return;
    if (onCommit) onCommit(val);
    else void setGraphValue(graph, path, val);
  };
  return (
    <Row label={label}>
      <span className="g-num">
        <input
          className="input input-compact num"
          value={v}
          placeholder={placeholder}
          aria-label={label}
          aria-invalid={bad}
          data-testid={testId}
          inputMode="decimal"
          onFocus={() => (focused.current = true)}
          onChange={(e) => setV(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
            else if (e.key === 'Escape') {
              setV(show(value));
              requestAnimationFrame(() => (e.target as HTMLInputElement).blur());
            }
            e.stopPropagation();
          }}
        />
        {unit && <span className="g-unit">{unit}</span>}
      </span>
    </Row>
  );
}

export function SelectField({
  graph,
  path,
  value,
  label,
  options,
  testId,
  onCommit,
}: {
  graph: Id;
  path: string;
  value: string | undefined;
  label: string;
  options: { value: string; label: string }[];
  testId?: string;
  onCommit?: (v: string) => void;
}) {
  return (
    <Row label={label}>
      <select
        className="input input-compact"
        value={value ?? ''}
        aria-label={label}
        data-testid={testId}
        onChange={(e) => (onCommit ? onCommit(e.target.value) : void setGraphValue(graph, path, e.target.value as JsonValue))}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </Row>
  );
}

export function CheckField({
  graph,
  path,
  value,
  label,
  testId,
  falseIsNull,
}: {
  graph: Id;
  path: string;
  value: boolean | undefined;
  label: string;
  testId?: string;
  /** Remove the field instead of writing false (fields that are absent by default). */
  falseIsNull?: boolean;
}) {
  return (
    <Row label={label}>
      <label className="g-check">
        <input
          type="checkbox"
          checked={!!value}
          data-testid={testId}
          aria-label={label}
          onChange={(e) => void setGraphValue(graph, path, e.target.checked ? true : falseIsNull ? null : false)}
        />
        <span>{value ? 'On' : 'Off'}</span>
      </label>
    </Row>
  );
}

/** A colour: swatches of the palette and a hex field. */
export function ColourField({
  value,
  fallback,
  swatches,
  onChange,
  label,
  testId,
}: {
  value: string | undefined;
  fallback: string;
  swatches: string[];
  onChange: (hex: string | null) => void;
  label: string;
  testId?: string;
}) {
  const [hex, setHex] = useState(value ?? '');
  useEffect(() => setHex(value ?? ''), [value]);
  const valid = (s: string) => /^#[0-9a-fA-F]{6}$/.test(s);
  return (
    <div className="colour-field" data-testid={testId}>
      <div className="swatches" role="radiogroup" aria-label={label}>
        {swatches.map((c) => (
          <button
            key={c}
            type="button"
            role="radio"
            aria-checked={(value ?? '').toLowerCase() === c.toLowerCase()}
            className="swatch"
            style={{ background: c }}
            title={c}
            aria-label={c}
            onClick={() => onChange(c)}
          />
        ))}
      </div>
      <div className="colour-hex">
        <span className="swatch is-current" style={{ background: valid(hex) ? hex : fallback }} aria-hidden="true" />
        <input
          className="input input-compact mono"
          value={hex}
          placeholder={`${fallback} (palette)`}
          aria-label={`${label} hex`}
          aria-invalid={hex !== '' && !valid(hex)}
          spellCheck={false}
          onChange={(e) => setHex(e.target.value)}
          onBlur={() => {
            if (hex === '') onChange(null);
            else if (valid(hex)) onChange(hex.toUpperCase());
            else setHex(value ?? '');
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
            e.stopPropagation();
          }}
        />
        {value && (
          <button type="button" className="btn btn-small btn-ghost" onClick={() => onChange(null)}>
            Palette
          </button>
        )}
      </div>
    </div>
  );
}
