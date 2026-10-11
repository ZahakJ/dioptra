// The compact options panel, generated from the spec JSON and the schema: one row per choice
// that matters, each with one plain sentence of explanation.
import { ChevronDown } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { JsonObject, JsonValue, TableMeta } from '../engine/types';
import { Checkbox, Menu, NumberField, Segmented, Select, TextInput, Toggle } from '../ui';
import type { Spec } from '../engine/objects';
import { columnsOf, datasetsOf, formModel, getAt, setAt, type Choice, type Field, type Primitive } from './schema';

export function OptionsPanel({
  spec,
  meta,
  onChange,
}: {
  spec: Spec;
  meta: TableMeta | undefined;
  onChange: (spec: Spec) => void;
}) {
  const rows = formModel(spec);
  if (rows.length === 0) return <p className="muted">This analysis has no options.</p>;
  // Data sets the analysis includes, for post-hoc control and pair pickers.
  const colField = rows.find((r) => r.field.type === 'columns');
  const included =
    colField && colField.field.type === 'columns'
      ? datasetsOf(meta, colField.value, colField.field.unit)
      : [];
  const pool = included.length ? included : (meta?.datasets.map((_, i) => i) ?? []);
  return (
    <div className="opt-list" data-testid="options-panel">
      {rows.map(({ field, value }) => (
        <OptionRow
          key={field.path}
          field={field}
          value={value}
          meta={meta}
          pool={pool}
          onValue={(v) => onChange(setAt(spec, field.path, v))}
        />
      ))}
    </div>
  );
}

function OptionRow({
  field,
  value,
  meta,
  pool,
  onValue,
}: {
  field: Field;
  value: JsonValue | undefined;
  meta: TableMeta | undefined;
  pool: number[];
  onValue: (v: JsonValue) => void;
}) {
  const id = `opt-${field.path.replace(/\./g, '-')}`;
  const wide = field.type === 'columns' || field.type === 'postHoc' || field.type === 'variant' || field.type === 'multiEnum';
  return (
    <div className={'opt-row' + (wide ? ' is-wide' : '')} data-testid={id}>
      <div className="opt-label" id={`${id}-label`}>
        {field.label}
      </div>
      <div className="opt-control">
        <Control field={field} value={value} meta={meta} pool={pool} onValue={onValue} labelId={`${id}-label`} />
      </div>
      <p className="opt-help">{field.help}</p>
    </div>
  );
}

const keyOf = (v: Primitive) => JSON.stringify(v);

function Control({
  field,
  value,
  meta,
  pool,
  onValue,
  labelId,
}: {
  field: Field;
  value: JsonValue | undefined;
  meta: TableMeta | undefined;
  pool: number[];
  onValue: (v: JsonValue) => void;
  labelId: string;
}) {
  switch (field.type) {
    case 'bool':
      return <Toggle checked={value === true} onChange={onValue} labelledBy={labelId} />;
    case 'enum':
      return field.control === 'seg' && field.choices.length <= 4 ? (
        <Segmented
          choices={(field.choices as Choice[]).map((c) => ({ value: c.value, label: c.label, hint: c.hint }))}
          value={(value ?? null) as Primitive}
          onChange={onValue}
          labelledBy={labelId}
        />
      ) : (
        <Select
          labelledBy={labelId}
          value={keyOf((value ?? null) as Primitive)}
          options={field.choices.map((c) => ({ value: keyOf(c.value), label: c.label, hint: c.hint }))}
          onChange={(k) => onValue(JSON.parse(k) as Primitive)}
        />
      );
    case 'number':
      return <NumberInput field={field} value={value} onValue={onValue} labelledBy={labelId} />;
    case 'text':
      return (
        <OptionText
          value={typeof value === 'string' ? value : ''}
          placeholder={field.nullable}
          suggestions={field.suggestions}
          mono={field.mono}
          labelledBy={labelId}
          onValue={(s) => onValue(s === '' && field.nullable ? null : s)}
        />
      );
    case 'numberList':
      return <NumberListInput value={value} placeholder={field.placeholder} labelledBy={labelId} onValue={onValue} />;
    case 'columns': {
      const picked = datasetsOf(meta, value, field.unit);
      return (
        <DatasetPicker
          meta={meta}
          picked={picked}
          allHint={field.emptyAll}
          onChange={(ds) => onValue(columnsOf(meta, ds, field.unit))}
        />
      );
    }
    case 'multiEnum': {
      const set = new Set(Array.isArray(value) ? value.map(String) : []);
      return (
        <div className="opt-checks" role="group" aria-labelledby={labelId}>
          {field.choices.map((c) => (
            <Checkbox
              key={String(c.value)}
              className="opt-check"
              checked={set.has(String(c.value))}
              onChange={(on) => {
                const next = field.choices.map((x) => x.value).filter((v) => (v === c.value ? on : set.has(String(v))));
                onValue(next as JsonValue);
              }}
            >
              {c.label}
            </Checkbox>
          ))}
        </div>
      );
    }
    case 'postHoc':
      return <PostHoc field={field} value={value} meta={meta} pool={pool} onValue={onValue} />;
    case 'variant': {
      const obj = (value && typeof value === 'object' && !Array.isArray(value) ? value : {}) as JsonObject;
      const tag = String(obj[field.tag] ?? field.variants[0]!.value);
      const variant = field.variants.find((v) => v.value === tag) ?? field.variants[0]!;
      return (
        <div className="opt-stack">
          <Segmented
            choices={field.variants.map((v) => ({ value: v.value as string, label: v.label, hint: v.hint }))}
            value={tag}
            labelledBy={labelId}
            onChange={(t) => {
              const v = field.variants.find((x) => x.value === t)!;
              onValue({ [field.tag]: v.value, ...v.defaults });
            }}
          />
          {variant.fields.map((f) => (
            <div key={f.path} className="opt-sub">
              <span className="opt-sub-label">{f.label}</span>
              <Control field={f} value={getAt(obj, f.path)} meta={meta} pool={pool} labelId={labelId} onValue={(v) => onValue(setAt(obj, f.path, v))} />
            </div>
          ))}
        </div>
      );
    }
  }
}

function NumberInput({
  field,
  value,
  onValue,
  labelledBy,
}: {
  field: Extract<Field, { type: 'number' }>;
  value: JsonValue | undefined;
  onValue: (v: JsonValue) => void;
  labelledBy: string;
}) {
  const shown = typeof value === 'number' ? (field.percent ? Math.round(value * 100 * 1e6) / 1e6 : value) : null;
  return (
    <NumberField
      className="opt-num"
      labelledBy={labelledBy}
      value={shown}
      placeholder={field.nullable}
      nullable={!!field.nullable}
      integer={field.integer}
      step={field.percent ? 1 : field.integer ? 1 : 0.1}
      unit={field.percent ? '%' : undefined}
      onCommit={(v) => {
        if (v === null) return onValue(null);
        onValue(field.percent ? v / 100 : v);
      }}
    />
  );
}

function OptionText({
  value,
  placeholder,
  suggestions,
  mono,
  labelledBy,
  onValue,
}: {
  value: string;
  placeholder?: string;
  suggestions?: string[];
  mono?: boolean;
  labelledBy: string;
  onValue: (v: string) => void;
}) {
  const [text, setText] = useState(value);
  const [open, setOpen] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);
  useEffect(() => setText(value), [value]);
  return (
    <span className="opt-text">
      <TextInput
        size="sm"
        mono={mono}
        aria-labelledby={labelledBy}
        value={text}
        placeholder={placeholder}
        onChange={(e) => setText(e.target.value)}
        onBlur={() => text !== value && onValue(text.trim())}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            onValue(text.trim());
          }
        }}
      />
      {suggestions && (
        <>
          <button
            ref={btn}
            type="button"
            className="btn btn-sm btn-icon"
            aria-label="Suggestions"
            data-tip="Suggestions"
            aria-haspopup="menu"
            aria-expanded={open}
            onClick={() => setOpen((o) => !o)}
          >
            <ChevronDown size={16} strokeWidth={1.5} aria-hidden="true" />
          </button>
          <Menu
            anchor={btn.current}
            open={open}
            onClose={() => setOpen(false)}
            placement="bottom-end"
            label="Suggestions"
            items={suggestions.map((sug) => ({
              label: sug,
              run: () => {
                setText(sug);
                onValue(sug);
              },
            }))}
          />
        </>
      )}
    </span>
  );
}

function NumberListInput({
  value,
  placeholder,
  labelledBy,
  onValue,
}: {
  value: JsonValue | undefined;
  placeholder?: string;
  labelledBy: string;
  onValue: (v: JsonValue) => void;
}) {
  const show = Array.isArray(value) ? value.join(', ') : '';
  const [text, setText] = useState(show);
  useEffect(() => setText(show), [show]);
  const commit = () => {
    const nums = text
      .split(/[,;\s]+/)
      .map((s) => s.trim())
      .filter(Boolean)
      .map(Number)
      .filter((n) => Number.isFinite(n));
    onValue(nums);
  };
  return (
    <TextInput
      size="sm"
      mono
      aria-labelledby={labelledBy}
      value={text}
      placeholder={placeholder}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), commit())}
    />
  );
}

export function DatasetPicker({
  meta,
  picked,
  allHint,
  onChange,
}: {
  meta: TableMeta | undefined;
  picked: number[];
  allHint?: boolean;
  onChange: (datasets: number[]) => void;
}) {
  const ds = meta?.datasets ?? [];
  const all = allHint && picked.length === 0;
  return (
    <div className="ds-picker" role="group" data-testid="dataset-picker">
      {ds.map((d, i) => {
        const on = picked.includes(i);
        return (
          <button
            key={i}
            type="button"
            role="checkbox"
            aria-checked={on}
            className={'ds-chip' + (on || all ? ' is-on' : '')}
            data-tip={d.name.length > 14 ? d.name : undefined}
            data-testid={`ds-chip-${i}`}
            onClick={() => onChange(on ? picked.filter((x) => x !== i) : [...picked, i].sort((a, b) => a - b))}
          >
            <span className="ds-letter">{d.letter}</span>
            <span className="ds-name">{d.name}</span>
          </button>
        );
      })}
      {all && <span className="ds-all">All data sets</span>}
    </div>
  );
}

type Mc = { method: { name: string; control?: number }; comparisons: { kind: string; control?: number; pairs?: [number, number][] } };

function PostHoc({
  field,
  value,
  meta,
  pool,
  onValue,
}: {
  field: Extract<Field, { type: 'postHoc' }>;
  value: JsonValue | undefined;
  meta: TableMeta | undefined;
  pool: number[];
  onValue: (v: JsonValue) => void;
}) {
  const on = value !== null && value !== undefined;
  const mc = (on ? value : { method: { name: 'tukey' }, comparisons: { kind: 'all_pairs' } }) as unknown as Mc;
  const name = (i: number) => meta?.datasets[i]?.name ?? `Data set ${i + 1}`;
  const control = mc.comparisons.control ?? mc.method.control ?? pool[0] ?? 0;
  const set = (next: Mc) => onValue(next as unknown as JsonValue);
  const kind = mc.comparisons.kind;
  const pairs: [number, number][] = [];
  for (let a = 0; a < pool.length; a++) for (let b = a + 1; b < pool.length; b++) pairs.push([pool[a]!, pool[b]!]);
  const chosen = new Set((mc.comparisons.pairs ?? []).map(([a, b]) => `${a}:${b}`));

  const setKind = (k: string) => {
    if (k === 'all_pairs') set({ method: mc.method.name === 'dunnett' ? { name: 'tukey' } : mc.method, comparisons: { kind: 'all_pairs' } });
    else if (k === 'vs_control')
      set({
        method: mc.method.name === 'tukey' || mc.method.name === 'dunnett' ? { name: 'dunnett', control } : mc.method,
        comparisons: { kind: 'vs_control', control },
      });
    else
      set({
        method: mc.method.name === 'tukey' || mc.method.name === 'dunnett' ? { name: 'sidak' } : mc.method,
        comparisons: { kind: 'selected', pairs: pairs.slice(0, 1) },
      });
  };
  const setMethod = (m: string) => {
    if (m === 'dunnett') set({ method: { name: 'dunnett', control }, comparisons: { kind: 'vs_control', control } });
    else if (m === 'tukey') set({ method: { name: 'tukey' }, comparisons: { kind: 'all_pairs' } });
    else set({ method: { name: m }, comparisons: mc.comparisons });
  };
  const setControl = (c: number) =>
    set({
      method: mc.method.name === 'dunnett' ? { name: 'dunnett', control: c } : mc.method,
      comparisons: { kind: 'vs_control', control: c },
    });

  return (
    <div className="opt-stack" data-testid="posthoc">
      {field.nullable && (
        <Toggle checked={on} label="Compare groups after the overall test" testId="posthoc-on" onChange={(v) => onValue(v ? (mc as unknown as JsonValue) : null)} />
      )}
      {on && (
        <>
          <Segmented
            label="Which comparisons"
            testId="posthoc-kind"
            choices={[
              { value: 'all_pairs', label: 'All pairs' },
              { value: 'vs_control', label: 'Against a control' },
              { value: 'selected', label: 'Chosen pairs' },
            ]}
            value={kind}
            onChange={setKind}
          />
          <div className="opt-sub">
            <span className="opt-sub-label">Method</span>
            <Select
              value={mc.method.name}
              testId="posthoc-method"
              label="Multiple comparisons method"
              onChange={setMethod}
              options={field.methods
                .filter((m) => !(m.value === 'tukey' && kind !== 'all_pairs') && !(m.value === 'dunnett' && kind === 'selected'))
                .map((m) => ({ value: String(m.value), label: m.label }))}
            />
          </div>
          <p className="opt-help">{field.methods.find((m) => m.value === mc.method.name)?.hint}</p>
          {kind === 'vs_control' && (
            <div className="opt-sub">
              <span className="opt-sub-label">Control</span>
              <Select
                value={control}
                label="Control group"
                testId="posthoc-control"
                onChange={setControl}
                options={pool.map((i) => ({ value: i, label: name(i), secondary: meta?.datasets[i]?.letter }))}
              />
            </div>
          )}
          {kind === 'selected' && (
            <div className="opt-checks" role="group" aria-label="Pairs to compare">
              {pairs.map(([a, b]) => {
                const k = `${a}:${b}`;
                return (
                  <Checkbox
                    key={k}
                    className="opt-check"
                    checked={chosen.has(k)}
                    onChange={(onPair) => {
                      const next = pairs.filter(([x, y]) => {
                        const kk = `${x}:${y}`;
                        return kk === k ? onPair : chosen.has(kk);
                      });
                      set({ method: mc.method, comparisons: { kind: 'selected', pairs: next } });
                    }}
                  >
                    {name(a)} vs {name(b)}
                  </Checkbox>
                );
              })}
            </div>
          )}
        </>
      )}
    </div>
  );
}
