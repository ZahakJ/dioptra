// Graph inspector sections (orders 40–58): the selected element, then Graph, Axes, Data sets,
// Legend, Annotations and Export. Every control writes one path of the spec.
import { useEffect, useState } from 'react';
import { registerInspectorSection, type InspectorContext } from '../app/inspector';
import { Segmented } from '../analysis/OptionsPanel';
import type { Annotation, AxisSpec, DatasetStyle, GraphSpecT } from '../engine/objects';
import type { Id, JsonValue, TableMeta } from '../engine/types';
import { useTableMeta } from '../store/table';
import { addBracket, addReferenceLine, addText, copySvg, exportGraphDialog, sourceTableId, tableMeta } from './actions';
import {
  ERROR_BARS,
  familiesFor,
  familyName,
  LEGEND_POSITIONS,
  LINE_STYLES,
  MARKERS,
  PALETTES,
  paletteColours,
  SIZE_PRESETS,
  THEMES,
} from './families';
import { CheckField, ColourField, NumField, Row, SelectField, TextField } from './fields';
import { selectedPointer, setGraphValue, useGraph } from './store';
import { describeTarget } from './targets';

const isGraph = (c: InspectorContext) => c.item?.kind === 'graph';

function useSourceMeta(spec: GraphSpecT | null): TableMeta | undefined {
  const tid = spec ? sourceTableId(spec.source) : null;
  const meta = useTableMeta(tid);
  useEffect(() => {
    if (tid && !meta) void tableMeta(tid);
  }, [tid, meta]);
  return meta;
}

// ---- selection ---------------------------------------------------------------------------------

function SelectionSection({ ctx }: { ctx: InspectorContext }) {
  const id = ctx.item!.id;
  const st = useGraph(id);
  const meta = useSourceMeta(st.spec);
  const spec = st.spec;
  const pointer = selectedPointer(st);
  const target = st.rendered?.targets.find((t) => t.id === st.selected);
  if (!spec) return null;
  if (!pointer || !target) {
    return <p className="muted small">Click any part of the graph to edit it here. Double-click text to rewrite it.</p>;
  }
  let body: React.ReactNode = null;
  const ax = pointer.match(/^\/axes\/(x|y)/);
  const ds = pointer.match(/^\/style\/datasets\/(\d+)/);
  const an = pointer.match(/^\/annotations\/(\d+)/);
  if (pointer === '/title') body = <TextField graph={id} path="/title" value={spec.title} label="Title" placeholder="None" testId="sel-title" autoFocus />;
  else if (ax) body = <AxisFields graph={id} axis={ax[1] as 'x' | 'y'} spec={spec} focusTitle={pointer.endsWith('/title')} />;
  else if (ds) body = <DatasetFields graph={id} spec={spec} dataset={Number(ds[1])} meta={meta} />;
  else if (an) body = <AnnotationFields graph={id} spec={spec} index={Number(an[1])} />;
  else if (pointer === '/legend') body = <LegendFields graph={id} spec={spec} />;
  else if (pointer === '/axes/frame' || pointer === '/axes/grid') body = <FrameFields graph={id} spec={spec} />;
  else if (pointer === '/style/palette') body = <PaletteField graph={id} spec={spec} />;
  else body = <MappingFields graph={id} spec={spec} />;
  return (
    <div className="g-sel" data-testid="graph-selection" data-target={pointer}>
      <div className="g-sel-head">
        <span className="g-sel-name">{describeTarget(target)}</span>
        <span className="mono g-sel-path">{pointer}</span>
      </div>
      {body}
    </div>
  );
}

// ---- graph -------------------------------------------------------------------------------------

function GraphSection({ ctx }: { ctx: InspectorContext }) {
  const id = ctx.item!.id;
  const { spec } = useGraph(id);
  const meta = useSourceMeta(spec);
  if (!spec) return null;
  const s = spec.style.size_mm;
  const families = familiesFor(meta?.kind);
  const fam = families.includes(spec.family) ? families : [spec.family, ...families];
  return (
    <div className="g-list">
      <TextField graph={id} path="/title" value={spec.title} label="Title" placeholder="None" testId="g-title" />
      <SelectField graph={id} path="/family" value={spec.family} label="Family" options={fam.map((f) => ({ value: f, label: familyName(f) }))} testId="g-family" />
      <NumField graph={id} path="/style/size_mm/width" value={s.width} label="Width" unit="mm" min={10} max={1000} placeholder="" />
      <NumField graph={id} path="/style/size_mm/height" value={s.height} label="Height" unit="mm" min={10} max={1000} placeholder="" />
      <Row label="Preset">
        <select
          className="input input-compact"
          aria-label="Size preset"
          value={SIZE_PRESETS.find((p) => p.width === s.width && p.height === s.height)?.id ?? 'custom'}
          onChange={(e) => {
            const p = SIZE_PRESETS.find((x) => x.id === e.target.value);
            if (p) void setGraphValue(id, '/style/size_mm', { width: p.width, height: p.height });
          }}
        >
          {SIZE_PRESETS.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label}
            </option>
          ))}
          <option value="custom">Custom</option>
        </select>
      </Row>
      <SelectField graph={id} path="/style/theme" value={spec.style.theme} label="Theme" options={THEMES} testId="g-theme" />
      <PaletteField graph={id} spec={spec} />
      <NumField graph={id} path="/style/font_pt" value={spec.style.font_pt} label="Font size" unit="pt" min={4} max={36} placeholder="Theme" />
      <NumField graph={id} path="/style/line_pt" value={spec.style.line_pt} label="Axis lines" unit="pt" min={0.1} max={6} placeholder="Theme" />
    </div>
  );
}

function PaletteField({ graph, spec }: { graph: Id; spec: GraphSpecT }) {
  return (
    <Row label="Palette">
      <span className="palette-pick">
        <select
          className="input input-compact"
          aria-label="Palette"
          data-testid="g-palette"
          value={spec.style.palette ?? ''}
          onChange={(e) => void setGraphValue(graph, '/style/palette', e.target.value || null)}
        >
          <option value="">Theme default</option>
          {PALETTES.map((p) => (
            <option key={p.value} value={p.value}>
              {p.label}
            </option>
          ))}
        </select>
        <span className="palette-strip" aria-hidden="true">
          {paletteColours(spec.style.palette).slice(0, 6).map((c) => (
            <i key={c} style={{ background: c }} />
          ))}
        </span>
      </span>
    </Row>
  );
}

// ---- axes --------------------------------------------------------------------------------------

function AxesSection({ ctx }: { ctx: InspectorContext }) {
  const id = ctx.item!.id;
  const { spec } = useGraph(id);
  const [axis, setAxis] = useState<'x' | 'y'>('y');
  if (!spec) return null;
  return (
    <div className="g-list">
      <Segmented
        label="Axis"
        choices={[
          { value: 'x', label: 'X axis' },
          { value: 'y', label: 'Y axis' },
        ]}
        value={axis}
        onChange={(a) => setAxis(a as 'x' | 'y')}
      />
      <AxisFields graph={id} axis={axis} spec={spec} />
      <FrameFields graph={id} spec={spec} />
    </div>
  );
}

const SCALES = [
  { value: 'linear', label: 'Linear' },
  { value: 'log10', label: 'Log 10' },
  { value: 'log2', label: 'Log 2' },
  { value: 'categorical', label: 'Categories' },
];
const TICK_FORMATS = [
  { value: 'auto', label: 'Automatic' },
  { value: 'decimal', label: 'Decimal' },
  { value: 'scientific', label: 'Scientific' },
  { value: 'power', label: 'Powers (log)' },
];
const DIRECTIONS = [
  { value: 'out', label: 'Outside' },
  { value: 'in', label: 'Inside' },
  { value: 'cross', label: 'Crossing' },
];

function breaksText(a: AxisSpec): string {
  return (a.breaks ?? []).map((b) => `${b.from} to ${b.to}`).join('; ');
}

function parseBreaks(s: string): { from: number; to: number }[] | null {
  if (!s.trim()) return [];
  const out: { from: number; to: number }[] = [];
  for (const part of s.split(/[;\n]+/)) {
    const m = part.trim().match(/^(-?[\d.]+(?:e-?\d+)?)\s*(?:to|-|–|,)\s*(-?[\d.]+(?:e-?\d+)?)$/i);
    if (!m) return null;
    const a = Number(m[1]);
    const b = Number(m[2]);
    if (!(a < b)) return null;
    out.push({ from: a, to: b });
  }
  return out;
}

function AxisFields({ graph, axis, spec, focusTitle }: { graph: Id; axis: 'x' | 'y'; spec: GraphSpecT; focusTitle?: boolean }) {
  const a = spec.axes[axis] ?? {};
  const t = a.ticks ?? {};
  const p = `/axes/${axis}`;
  const [brk, setBrk] = useState(breaksText(a));
  useEffect(() => setBrk(breaksText(a)), [a]); // eslint-disable-line react-hooks/exhaustive-deps
  const brkOk = parseBreaks(brk) !== null;
  return (
    <div className="g-list" data-testid={`axis-fields-${axis}`}>
      <TextField graph={graph} path={`${p}/title`} value={a.title} label="Title" placeholder="From the data" testId="axis-title" autoFocus={focusTitle} />
      <SelectField graph={graph} path={`${p}/scale`} value={a.scale ?? 'linear'} label="Scale" options={SCALES} testId="axis-scale" />
      <NumField graph={graph} path={`${p}/min`} value={a.min} label="Minimum" testId="axis-min" />
      <NumField graph={graph} path={`${p}/max`} value={a.max} label="Maximum" testId="axis-max" />
      <NumField graph={graph} path={`${p}/ticks/major`} value={t.major} label="Tick interval" min={0} />
      <NumField graph={graph} path={`${p}/ticks/minor`} value={t.minor ?? 0} label="Minor ticks" min={0} max={20} integer placeholder="0" />
      <SelectField graph={graph} path={`${p}/ticks/direction`} value={t.direction ?? 'out'} label="Ticks" options={DIRECTIONS} />
      <SelectField graph={graph} path={`${p}/ticks/format`} value={t.format ?? 'auto'} label="Format" options={TICK_FORMATS} />
      <NumField graph={graph} path={`${p}/ticks/decimals`} value={t.decimals} label="Decimals" min={0} max={10} integer />
      <NumField graph={graph} path={`${p}/ticks/label_angle`} value={t.label_angle} label="Label angle" unit="°" min={-90} max={90} />
      <Row label="Breaks" hint="Gaps cut out of the axis, e.g. 20 to 80; 120 to 400">
        <input
          className="input input-compact mono"
          value={brk}
          placeholder="None, e.g. 20 to 80"
          aria-label="Axis breaks"
          aria-invalid={!brkOk}
          onChange={(e) => setBrk(e.target.value)}
          onBlur={() => {
            const b = parseBreaks(brk);
            if (b === null) return setBrk(breaksText(a));
            void setGraphValue(graph, `${p}/breaks`, b.length ? (b as unknown as JsonValue) : null);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
            e.stopPropagation();
          }}
        />
      </Row>
      <CheckField graph={graph} path={`${p}/hidden`} value={a.hidden} label="Hidden" falseIsNull />
    </div>
  );
}

function FrameFields({ graph, spec }: { graph: Id; spec: GraphSpecT }) {
  return (
    <>
      <SelectField
        graph={graph}
        path="/axes/frame"
        value={spec.axes.frame ?? ''}
        label="Frame"
        options={[
          { value: '', label: 'Theme default' },
          { value: 'l', label: 'Left and bottom' },
          { value: 'box', label: 'Box' },
          { value: 'none', label: 'No lines' },
        ]}
        onCommit={(v) => void setGraphValue(graph, '/axes/frame', v || null)}
      />
      <SelectField
        graph={graph}
        path="/axes/origin"
        value={spec.axes.origin ?? 'frame'}
        label="Axes cross"
        options={[
          { value: 'frame', label: 'At the edges' },
          { value: 'zero', label: 'At zero' },
        ]}
      />
      <SelectField
        graph={graph}
        path="/axes/grid"
        value={spec.axes.grid === undefined ? '' : String(spec.axes.grid)}
        label="Gridlines"
        options={[
          { value: '', label: 'Theme default' },
          { value: 'true', label: 'Major' },
          { value: 'false', label: 'None' },
        ]}
        onCommit={(v) => void setGraphValue(graph, '/axes/grid', v === '' ? null : v === 'true')}
      />
    </>
  );
}

// ---- data sets ---------------------------------------------------------------------------------

function datasetNames(meta: TableMeta | undefined, spec: GraphSpecT): { index: number; name: string }[] {
  if (!meta) return [];
  const all = meta.datasets.map((d, i) => ({ index: i, name: d.name }));
  return spec.mapping.datasets?.length ? all.filter((d) => spec.mapping.datasets!.includes(d.index)) : all;
}

/** Write one dataset's style: the whole `style.datasets` array (merge patches replace arrays). */
function setDatasetStyle(graph: Id, spec: GraphSpecT, dataset: number, change: Partial<DatasetStyle>) {
  const list = [...(spec.style.datasets ?? [])];
  const i = list.findIndex((d) => d.dataset === dataset);
  const cur: DatasetStyle = i >= 0 ? { ...list[i]! } : { dataset };
  for (const [k, v] of Object.entries(change)) {
    if (v === null || v === undefined || v === '') delete (cur as unknown as Record<string, unknown>)[k];
    else (cur as unknown as Record<string, unknown>)[k] = v;
  }
  const empty = Object.keys(cur).length === 1;
  if (i >= 0) {
    if (empty) list.splice(i, 1);
    else list[i] = cur;
  } else if (!empty) list.push(cur);
  list.sort((a, b) => a.dataset - b.dataset);
  return setGraphValue(graph, '/style/datasets', list.length ? (list as unknown as JsonValue) : null);
}

function DatasetsSection({ ctx }: { ctx: InspectorContext }) {
  const id = ctx.item!.id;
  const st = useGraph(id);
  const meta = useSourceMeta(st.spec);
  const [open, setOpen] = useState<number | null>(null);
  const spec = st.spec;
  if (!spec) return null;
  const names = datasetNames(meta, spec);
  const colours = paletteColours(spec.style.palette);
  return (
    <div className="g-list">
      <MappingFields graph={id} spec={spec} />
      <ul className="ds-rows">
        {names.map((d, k) => {
          const style = spec.style.datasets?.find((x) => x.dataset === d.index);
          const colour = style?.colour ?? colours[k % colours.length]!;
          return (
            <li key={d.index} className={'ds-row' + (open === d.index ? ' is-open' : '')}>
              <button type="button" className="ds-row-head" aria-expanded={open === d.index} onClick={() => setOpen(open === d.index ? null : d.index)} data-testid={`ds-row-${d.index}`}>
                <span className="swatch" style={{ background: colour }} aria-hidden="true" />
                <span className="ds-row-name">{style?.label ?? d.name}</span>
                <span className="ds-row-meta">{style?.marker ?? ''}</span>
              </button>
              {open === d.index && <DatasetFields graph={id} spec={spec} dataset={d.index} meta={meta} />}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function DatasetFields({ graph, spec, dataset, meta }: { graph: Id; spec: GraphSpecT; dataset: number; meta: TableMeta | undefined }) {
  const style = spec.style.datasets?.find((x) => x.dataset === dataset) ?? { dataset };
  const names = datasetNames(meta, spec);
  const k = Math.max(0, names.findIndex((d) => d.index === dataset));
  const colours = paletteColours(spec.style.palette);
  const set = (c: Partial<DatasetStyle>) => void setDatasetStyle(graph, spec, dataset, c);
  const fill = (style.marker ?? '').endsWith('_open') ? 'open' : 'filled';
  const shape = (style.marker ?? '').replace(/_open$/, '');
  return (
    <div className="g-list ds-fields" data-testid={`ds-fields-${dataset}`}>
      <Row label="Colour">
        <ColourField
          value={style.colour}
          fallback={colours[k % colours.length]!}
          swatches={colours}
          label="Colour"
          testId="ds-colour"
          onChange={(c) => set({ colour: c ?? undefined })}
        />
      </Row>
      <Row label="Marker">
        <span className="g-pair">
          <select className="input input-compact" aria-label="Marker shape" value={shape} onChange={(e) => set({ marker: e.target.value ? (fill === 'open' && e.target.value !== 'none' ? `${e.target.value}_open` : e.target.value) : undefined })}>
            <option value="">Theme</option>
            {MARKERS.map((m) => (
              <option key={m} value={m}>
                {m.replace(/_/g, ' ')}
              </option>
            ))}
          </select>
          <select
            className="input input-compact"
            aria-label="Marker fill"
            value={fill}
            disabled={!shape || shape === 'none' || shape === 'plus' || shape === 'cross'}
            onChange={(e) => set({ marker: e.target.value === 'open' ? `${shape}_open` : shape })}
          >
            <option value="filled">Filled</option>
            <option value="open">Open</option>
          </select>
        </span>
      </Row>
      <NumField graph={graph} path="" value={style.marker_pt} label="Marker size" unit="pt" min={0.5} max={30} placeholder="Theme" onCommit={(v) => set({ marker_pt: v ?? undefined })} />
      <Row label="Line">
        <select className="input input-compact" aria-label="Line style" value={style.line_style ?? ''} onChange={(e) => set({ line_style: (e.target.value || undefined) as DatasetStyle['line_style'] })}>
          <option value="">Theme</option>
          {LINE_STYLES.map((m) => (
            <option key={m} value={m}>
              {m.replace(/_/g, ' ')}
            </option>
          ))}
        </select>
      </Row>
      <NumField graph={graph} path="" value={style.line_pt} label="Line width" unit="pt" min={0.1} max={10} placeholder="Theme" onCommit={(v) => set({ line_pt: v ?? undefined })} />
      <DatasetLabel value={style.label} placeholder={names[k]?.name ?? ''} onCommit={(v) => set({ label: v || undefined })} />
    </div>
  );
}

function DatasetLabel({ value, placeholder, onCommit }: { value: string | undefined; placeholder: string; onCommit: (v: string) => void }) {
  const [v, setV] = useState(value ?? '');
  useEffect(() => setV(value ?? ''), [value]);
  return (
    <Row label="Label">
      <input
        className="input input-compact"
        value={v}
        placeholder={placeholder}
        aria-label="Legend label"
        onChange={(e) => setV(e.target.value)}
        onBlur={() => v !== (value ?? '') && onCommit(v.trim())}
        onKeyDown={(e) => {
          if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
          e.stopPropagation();
        }}
      />
    </Row>
  );
}

function MappingFields({ graph, spec }: { graph: Id; spec: GraphSpecT }) {
  return (
    <>
      <SelectField graph={graph} path="/mapping/error_bars" value={spec.mapping.error_bars} label="Error bars" options={ERROR_BARS} testId="g-error-bars" />
      <CheckField graph={graph} path="/mapping/points" value={spec.mapping.points} label="Points" testId="g-points" />
    </>
  );
}

// ---- legend ------------------------------------------------------------------------------------

function LegendSection({ ctx }: { ctx: InspectorContext }) {
  const id = ctx.item!.id;
  const { spec } = useGraph(id);
  if (!spec) return null;
  return <LegendFields graph={id} spec={spec} />;
}

function LegendFields({ graph, spec }: { graph: Id; spec: GraphSpecT }) {
  const l = spec.legend;
  return (
    <div className="g-list">
      <SelectField
        graph={graph}
        path="/legend/position"
        value={l.at ? 'manual' : l.position}
        label="Position"
        options={[...LEGEND_POSITIONS, ...(l.at ? [{ value: 'manual', label: 'Where you dragged it' }] : [])]}
        testId="legend-position"
        onCommit={(v) => v !== 'manual' && void setGraphValue(graph, '/legend', { position: v, at: null })}
      />
      <TextField graph={graph} path="/legend/title" value={l.title} label="Title" placeholder="None" />
    </div>
  );
}

// ---- annotations -------------------------------------------------------------------------------

function AnnotationsSection({ ctx }: { ctx: InspectorContext }) {
  const id = ctx.item!.id;
  const { spec } = useGraph(id);
  if (!spec) return null;
  const list = spec.annotations ?? [];
  return (
    <div className="g-list">
      <div className="btn-row">
        <button type="button" className="btn btn-small" onClick={() => void addBracket(id)} data-testid="insp-add-bracket">
          Bracket
        </button>
        <button type="button" className="btn btn-small" onClick={() => void addReferenceLine(id)}>
          Line
        </button>
        <button type="button" className="btn btn-small" onClick={() => void addText(id)}>
          Text
        </button>
      </div>
      {list.length === 0 && <p className="muted small">No annotations.</p>}
      {list.map((_, i) => (
        <AnnotationFields key={i} graph={id} spec={spec} index={i} compact />
      ))}
    </div>
  );
}

const ANN_NAME: Record<string, string> = {
  significance_bracket: 'Bracket',
  text_label: 'Text',
  line: 'Line',
  reference_line: 'Reference line',
};

function AnnotationFields({ graph, spec, index, compact }: { graph: Id; spec: GraphSpecT; index: number; compact?: boolean }) {
  const list = spec.annotations ?? [];
  const a = list[index];
  if (!a) return <p className="muted small">This annotation was removed.</p>;
  const p = `/annotations`;
  const write = (change: Partial<Annotation>) => {
    const next = list.map((x, i) => (i === index ? ({ ...x, ...change } as Annotation) : x));
    for (const [k, v] of Object.entries(change)) if (v === null || v === undefined) delete (next[index] as unknown as Record<string, unknown>)[k];
    return setGraphValue(graph, p, next as unknown as JsonValue);
  };
  const remove = () => setGraphValue(graph, p, list.length > 1 ? (list.filter((_, i) => i !== index) as unknown as JsonValue) : null);
  const num = (label: string, key: string, value: number | undefined, placeholder = '') => (
    <NumField graph={graph} path="" value={value} label={label} placeholder={placeholder} onCommit={(v) => void write({ [key]: v ?? undefined } as Partial<Annotation>)} />
  );
  return (
    <div className={'ann' + (compact ? ' is-compact' : '')} data-testid={`ann-${index}`}>
      <div className="ann-head">
        <span>
          {ANN_NAME[a.kind] ?? a.kind} {index + 1}
        </span>
        <button type="button" className="btn btn-small btn-ghost" onClick={() => void remove()} data-testid={`ann-delete-${index}`}>
          Delete
        </button>
      </div>
      {a.kind === 'significance_bracket' && (
        <>
          <AnnText value={a.text} onCommit={(t) => void write({ text: t })} />
          {num('From', 'from', a.from)}
          {num('To', 'to', a.to)}
          {num('Height', 'y', a.y, 'Auto')}
        </>
      )}
      {a.kind === 'text_label' && (
        <>
          <AnnText value={a.text} onCommit={(t) => void write({ text: t })} />
          {num('X', 'x', a.x)}
          {num('Y', 'y', a.y)}
          {num('Size', 'size_pt', a.size_pt, 'Theme')}
        </>
      )}
      {a.kind === 'reference_line' && (
        <>
          <Row label="Axis">
            <select className="input input-compact" value={a.axis} aria-label="Axis" onChange={(e) => void write({ axis: e.target.value as 'x' | 'y' })}>
              <option value="y">Horizontal (Y)</option>
              <option value="x">Vertical (X)</option>
            </select>
          </Row>
          {num('Value', 'value', a.value)}
          <AnnText label="Label" value={a.label ?? ''} onCommit={(t) => void write({ label: t || undefined })} />
          <Row label="Dashed">
            <input type="checkbox" checked={a.dashed !== false} aria-label="Dashed" onChange={(e) => void write({ dashed: e.target.checked })} />
          </Row>
        </>
      )}
      {a.kind === 'line' && (
        <>
          {num('X1', 'x1', a.x1)}
          {num('Y1', 'y1', a.y1)}
          {num('X2', 'x2', a.x2)}
          {num('Y2', 'y2', a.y2)}
        </>
      )}
    </div>
  );
}

function AnnText({ value, onCommit, label = 'Text' }: { value: string; onCommit: (v: string) => void; label?: string }) {
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  return (
    <Row label={label}>
      <input
        className="input input-compact"
        value={v}
        aria-label={label}
        onChange={(e) => setV(e.target.value)}
        onBlur={() => v !== value && onCommit(v)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
          e.stopPropagation();
        }}
      />
    </Row>
  );
}

// ---- export ------------------------------------------------------------------------------------

function ExportSection({ ctx }: { ctx: InspectorContext }) {
  const id = ctx.item!.id;
  const { spec } = useGraph(id);
  return (
    <div className="g-list">
      {spec && (
        <p className="muted small">
          {spec.style.size_mm.width} × {spec.style.size_mm.height} mm. The exported file is the graph you see.
        </p>
      )}
      <div className="btn-row">
        <button type="button" className="btn btn-small" onClick={() => void exportGraphDialog(id)} data-testid="insp-export">
          Export…
        </button>
        <button type="button" className="btn btn-small" onClick={() => void copySvg(id)} data-testid="insp-copy-svg">
          Copy SVG
        </button>
      </div>
    </div>
  );
}

export function registerGraphInspector(): void {
  registerInspectorSection({ id: 'graph-selection', title: 'Selected', order: 40, when: isGraph, component: SelectionSection });
  registerInspectorSection({ id: 'graph', title: 'Graph', order: 42, when: isGraph, component: GraphSection });
  registerInspectorSection({ id: 'graph-axes', title: 'Axes', order: 44, when: isGraph, component: AxesSection, collapsed: true });
  registerInspectorSection({ id: 'graph-datasets', title: 'Data sets', order: 46, when: isGraph, component: DatasetsSection, collapsed: true });
  registerInspectorSection({ id: 'graph-legend', title: 'Legend', order: 48, when: isGraph, component: LegendSection, collapsed: true });
  registerInspectorSection({ id: 'graph-annotations', title: 'Annotations', order: 50, when: isGraph, component: AnnotationsSection, collapsed: true });
  registerInspectorSection({ id: 'graph-export', title: 'Export', order: 58, when: isGraph, component: ExportSection });
}
