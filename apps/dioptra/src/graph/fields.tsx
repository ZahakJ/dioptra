// Inspector controls bound to a graph spec path. Text sends after 150 ms of quiet; numbers
// commit on Enter, blur or a step; selects and switches commit at once.
import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import type { Id, JsonValue } from '../engine/types';
import { ColorSwatch, NumberField, Select, TextInput, Toggle, type SelectOption } from '../ui';
import { setGraphValue, setGraphValueDebounced } from './store';

/** A label and its control. `children` may take the label's id to name the control. */
export function Row({ label, children, hint }: { label: string; children: ReactNode | ((labelId: string) => ReactNode); hint?: string }) {
  const id = useId();
  return (
    <div className="g-row">
      <span className={'g-label' + (hint ? ' has-tip' : '')} id={id} data-tip={hint}>
        {label}
      </span>
      <span className="g-control">{typeof children === 'function' ? children(id) : children}</span>
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
      <TextInput
        ref={ref}
        size="sm"
        value={v}
        placeholder={placeholder}
        aria-label={label}
        testId={testId}
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
  step,
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
  step?: number;
  integer?: boolean;
  unit?: string;
  testId?: string;
  /** Instead of patching `path` (e.g. to send an array). */
  onCommit?: (v: number | null) => void;
}) {
  return (
    <Row label={label}>
      <NumberField
        value={value}
        label={label}
        placeholder={placeholder}
        min={min}
        max={max}
        step={step ?? (unit === 'pt' ? 0.25 : 1)}
        integer={integer}
        unit={unit}
        testId={testId}
        onCommit={(v) => (onCommit ? onCommit(v) : void setGraphValue(graph, path, v))}
      />
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
  options: SelectOption<string>[];
  testId?: string;
  onCommit?: (v: string) => void;
}) {
  return (
    <Row label={label}>
      <Select
        value={value ?? ''}
        label={label}
        testId={testId}
        options={options}
        onChange={(v) => (onCommit ? onCommit(v) : void setGraphValue(graph, path, v as JsonValue))}
      />
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
      <Toggle checked={!!value} label={label} testId={testId} onChange={(on) => void setGraphValue(graph, path, on ? true : falseIsNull ? null : false)} />
    </Row>
  );
}

/** A colour: the palette's swatches and a hex field, in a popover. */
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
  return <ColorSwatch value={value} fallback={fallback} palette={swatches} onChange={onChange} label={label} testId={testId} />;
}
