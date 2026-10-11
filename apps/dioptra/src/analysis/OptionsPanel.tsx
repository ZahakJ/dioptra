// The compact options panel, generated from the spec JSON and the schema: one row per choice
// that matters, each with one plain sentence of explanation.
import { useEffect, useState } from 'react';
import type { JsonObject, JsonValue, TableMeta } from '../engine/types';
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
      return <Switch checked={value === true} onChange={onValue} labelledBy={labelId} />;
    case 'enum':
      return field.control === 'seg' && field.choices.length <= 4 ? (
        <Segmented choices={field.choices} value={(value ?? null) as Primitive} onChange={onValue} labelledBy={labelId} />
      ) : (
        <select
          className="input input-compact"
          aria-labelledby={labelId}
          value={keyOf((value ?? null) as Primitive)}
          title={field.choices.find((c) => keyOf(c.value) === keyOf((value ?? null) as Primitive))?.hint}
          onChange={(e) => onValue(JSON.parse(e.target.value) as Primitive)}
        >
          {field.choices.map((c) => (
            <option key={keyOf(c.value)} value={keyOf(c.value)}>
              {c.label}
              {c.hint ? ` — ${c.hint}` : ''}
            </option>
          ))}
        </select>
      );
    case 'number':
      return <NumberInput field={field} value={value} onValue={onValue} labelledBy={labelId} />;
    case 'text':
      return (
        <TextInput
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
            <label key={String(c.value)} className="opt-check">
              <input
                type="checkbox"
                checked={set.has(String(c.value))}
                onChange={(e) => {
                  const next = field.choices
                    .map((x) => x.value)
                    .filter((v) => (v === c.value ? e.target.checked : set.has(String(v))));
                  onValue(next as JsonValue);
                }}
              />
              {c.label}
            </label>
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
            choices={field.variants.map((v) => ({ value: v.value, label: v.label, hint: v.hint }))}
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

export function Switch({
  checked,
  onChange,
  labelledBy,
  label,
  testId,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  labelledBy?: string;
  label?: string;
  testId?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-labelledby={labelledBy}
      aria-label={label}
      className="switch"
      data-testid={testId}
      onClick={() => onChange(!checked)}
    >
      <span className="switch-knob" />
      <span className="switch-text">{checked ? 'On' : 'Off'}</span>
    </button>
  );
}

export function Segmented<T extends Primitive>({
  choices,
  value,
  onChange,
  labelledBy,
  label,
  testId,
}: {
  choices: { value: T; label: string; hint?: string }[] | Choice[];
  value: T;
  onChange: (v: T) => void;
  labelledBy?: string;
  label?: string;
  testId?: string;
}) {
  return (
    <div className="seg" role="radiogroup" aria-labelledby={labelledBy} aria-label={label} data-testid={testId}>
      {(choices as { value: T; label: string; hint?: string }[]).map((c) => (
        <button
          key={keyOf(c.value)}
          type="button"
          role="radio"
          aria-checked={keyOf(c.value) === keyOf(value)}
          className="seg-btn"
          title={c.hint}
          onClick={() => onChange(c.value)}
        >
          {c.label}
        </button>
      ))}
    </div>
  );
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
  const show = (v: JsonValue | undefined) =>
    typeof v === 'number' ? String(field.percent ? Math.round(v * 100 * 1e6) / 1e6 : v) : '';
  const [text, setText] = useState(show(value));
  useEffect(() => setText(show(value)), [value]); // eslint-disable-line react-hooks/exhaustive-deps
  const commit = (t: string) => {
    const s = t.trim();
    if (s === '') {
      if (field.nullable) onValue(null);
      else setText(show(value));
      return;
    }
    const n = Number(s.replace(',', '.'));
    if (!Number.isFinite(n)) return setText(show(value));
    const v = field.integer ? Math.round(n) : n;
    onValue(field.percent ? v / 100 : v);
  };
  return (
    <span className="opt-num">
      <input
        className="input input-compact num"
        inputMode="decimal"
        aria-labelledby={labelledBy}
        value={text}
        placeholder={field.nullable}
        onChange={(e) => setText(e.target.value)}
        onBlur={(e) => commit(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            commit((e.target as HTMLInputElement).value);
          }
        }}
      />
      {field.percent && <span className="opt-unit">%</span>}
    </span>
  );
}

function TextInput({
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
  useEffect(() => setText(value), [value]);
  const listId = suggestions ? `${labelledBy}-list` : undefined;
  return (
    <>
      <input
        className={'input input-compact' + (mono ? ' mono' : '')}
        aria-labelledby={labelledBy}
        value={text}
        list={listId}
        placeholder={placeholder}
        spellCheck={false}
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
        <datalist id={listId}>
          {suggestions.map((s) => (
            <option key={s} value={s} />
          ))}
        </datalist>
      )}
    </>
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
    <input
      className="input input-compact num"
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
          <label key={i} className={'ds-chip' + (on || all ? ' is-on' : '')} title={d.name}>
            <input
              type="checkbox"
              checked={on}
              onChange={(e) => onChange(e.target.checked ? [...picked, i].sort((a, b) => a - b) : picked.filter((x) => x !== i))}
            />
            <span className="ds-letter">{d.letter}</span>
            <span className="ds-name">{d.name}</span>
          </label>
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
        <Switch checked={on} label="Compare groups after the overall test" testId="posthoc-on" onChange={(v) => onValue(v ? (mc as unknown as JsonValue) : null)} />
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
            <select
              className="input input-compact"
              value={mc.method.name}
              data-testid="posthoc-method"
              aria-label="Multiple comparisons method"
              onChange={(e) => setMethod(e.target.value)}
            >
              {field.methods
                .filter((m) => !(m.value === 'tukey' && kind !== 'all_pairs') && !(m.value === 'dunnett' && kind === 'selected'))
                .map((m) => (
                  <option key={String(m.value)} value={String(m.value)}>
                    {m.label}
                  </option>
                ))}
            </select>
          </div>
          <p className="opt-help">{field.methods.find((m) => m.value === mc.method.name)?.hint}</p>
          {kind === 'vs_control' && (
            <div className="opt-sub">
              <span className="opt-sub-label">Control</span>
              <select
                className="input input-compact"
                value={control}
                aria-label="Control group"
                data-testid="posthoc-control"
                onChange={(e) => setControl(Number(e.target.value))}
              >
                {pool.map((i) => (
                  <option key={i} value={i}>
                    {name(i)}
                  </option>
                ))}
              </select>
            </div>
          )}
          {kind === 'selected' && (
            <div className="opt-checks" role="group" aria-label="Pairs to compare">
              {pairs.map(([a, b]) => {
                const k = `${a}:${b}`;
                return (
                  <label key={k} className="opt-check">
                    <input
                      type="checkbox"
                      checked={chosen.has(k)}
                      onChange={(e) => {
                        const next = pairs.filter(([x, y]) => {
                          const kk = `${x}:${y}`;
                          return kk === k ? e.target.checked : chosen.has(kk);
                        });
                        set({ method: mc.method, comparisons: { kind: 'selected', pairs: next } });
                      }}
                    />
                    {name(a)} vs {name(b)}
                  </label>
                );
              })}
            </div>
          )}
        </>
      )}
    </div>
  );
}
