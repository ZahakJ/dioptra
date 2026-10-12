// Graph and layout actions: new graph, graph from an analysis, annotations, size, export.
import { Button, Checkbox, Field, NumberField, Segmented, Select, TextInput } from '../ui';
import { useState } from 'react';
import { DialogHeader, openDialog } from '../app/dialogs';
import * as obj from '../engine/objects';
import type { Annotation, GraphSpecT } from '../engine/objects';
import type { GraphFormat, Id, TableMeta } from '../engine/types';
import { datasetsOf, fieldsFor } from '../analysis/schema';
import { desktopSavePath, downloadBytes, isDesktop } from '../io/files';
import { withEngine } from '../store/engine';
import { notify, notifyError } from '../store/notices';
import { itemById, useProject } from '../store/project';
import { mutate, query } from '../store/sync';
import { useTables } from '../store/table';
import { bracketsFromSheet, nextFreePair, placeBracket } from './brackets';
import { ExportGraphDialog, type ExportChoice } from './ExportGraphDialog';
import { familiesFor, familyForAnalysis, familyName } from './families';
import { flushGraph, graphState, patchGraph, setGraphValue } from './store';
import { axisMap, tidy, valueAt } from './svgAxes';

// ---- sources -----------------------------------------------------------------------------------

/** The table behind a graph source (a table, or an analysis' table). */
export function sourceTableId(source: Id): Id | null {
  const it = itemById(source);
  if (!it) return null;
  if (it.kind === 'table') return it.id;
  return it.derived_from ?? null;
}

export async function tableMeta(id: Id | null): Promise<TableMeta | undefined> {
  if (!id) return undefined;
  return useTables.getState().metas[id] ?? (await useTables.getState().loadMeta(id));
}

/** Category names in plotting order for a spec (data sets, or rows of grouped tables). */
export function categoriesOf(spec: GraphSpecT, meta: TableMeta | undefined): string[] {
  if (!meta) return [];
  if (meta.kind === 'grouped' || meta.kind === 'contingency' || meta.kind === 'parts') {
    return Array.from({ length: meta.rows }, (_, r) => meta.row_titles[r]?.trim() || String(r + 1));
  }
  const ds = spec.mapping.datasets?.length ? spec.mapping.datasets : meta.datasets.map((_, i) => i);
  return ds.map((i) => meta.datasets[i]?.name ?? `Data set ${i + 1}`);
}

// ---- new graph ---------------------------------------------------------------------------------

function graphSource(): Id | null {
  const { activeId, view } = useProject.getState();
  const item = view?.items.find((i) => i.id === activeId);
  if (item?.kind === 'table' || item?.kind === 'analysis') return item.id;
  if (item?.kind === 'graph') return null;
  return view?.items.find((i) => i.kind === 'table')?.id ?? null;
}

export async function newGraph(source: Id | null = graphSource()): Promise<void> {
  if (!source) {
    notify('Open a table or a results sheet first; graphs draw data.');
    return;
  }
  const item = itemById(source);
  const meta = await tableMeta(sourceTableId(source));
  const families = familiesFor(meta?.kind);
  // An analysis names its natural graph (a paired t test: bars with connecting lines).
  const sheetFamily = item?.kind === 'analysis' ? (await query((e) => obj.getResults(e, source)))?.graph_family : undefined;
  const initial = item?.kind === 'analysis' ? (sheetFamily ?? familyForAnalysis(item.subtype ?? '')) : families[0]!;
  const family = await openDialog<string | null>(
    (close) => <FamilyDialog title={item?.title ?? ''} families={families} initial={families.includes(initial) ? initial : families[0]!} close={close} />,
    null,
  );
  if (!family) return;
  if (item?.kind === 'analysis') await makeGraphFromAnalysis(source, family);
  else await mutate((e) => obj.createGraph(e, source, family));
}

function FamilyDialog({
  title,
  families,
  initial,
  close,
}: {
  title: string;
  families: string[];
  initial: string;
  close: (f: string | null) => void;
}) {
  const [f, setF] = useState(initial);
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="new-graph-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        close(f);
      }}
    >
      <DialogHeader title="New graph" sub={`Of "${title}". Every choice can be changed later.`} />
      <div className="family-grid" role="radiogroup" aria-label="Graph family">
        {families.map((x) => (
          <button
            key={x}
            type="button"
            role="radio"
            aria-checked={x === f}
            className="family-card"
            data-testid={`family-${x}`}
            onClick={() => setF(x)}
            onDoubleClick={() => close(x)}
          >
            {familyName(x)}
          </button>
        ))}
      </div>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" testId="new-graph-create">
          Create graph
        </Button>
      </footer>
    </form>
  );
}

/** The data sets an analysis uses, from its spec (empty: all). */
async function analysisDatasets(id: Id, meta: TableMeta | undefined): Promise<number[]> {
  const spec = await query((e) => obj.getAnalysisSpec(e, id));
  if (!spec) return [];
  const f = fieldsFor(spec.kind).find((x) => x.type === 'columns');
  if (!f || f.type !== 'columns') return [];
  return datasetsOf(meta, spec[f.path], f.unit);
}

/**
 * The natural graph for an analysis. The engine maps the analysed data sets and adds the
 * analysis's own significance brackets (in the project's p style), so nothing is added here
 * except the data sets when an older engine left them out. `family` undefined lets the engine
 * pick the analysis's default family.
 */
export async function makeGraphFromAnalysis(id: Id, family?: string): Promise<void> {
  const item = itemById(id);
  if (!item) return;
  const changed = await mutate((e) => obj.createGraph(e, id, family ?? 'auto'));
  const gid = changed?.created;
  if (!gid) return;
  try {
    const spec = await withEngine((e) => obj.getGraphSpec(e, gid));
    if (spec.mapping.datasets?.length) return;
    const meta = await tableMeta(item.derived_from ?? null);
    const ds = await analysisDatasets(id, meta);
    if (ds.length && meta && ds.length < meta.datasets.length) await patchGraph(gid, { mapping: { datasets: ds } });
  } catch (err) {
    notifyError(err);
  }
}

// ---- annotations -------------------------------------------------------------------------------

export async function addBracket(graph: Id): Promise<void> {
  const st = graphState(graph);
  const spec = st.spec;
  if (!spec) return;
  const meta = await tableMeta(sourceTableId(spec.source));
  const names = categoriesOf(spec, meta);
  if (names.length < 2) {
    notify('Brackets join two categories; this graph has fewer than two.');
    return;
  }
  const fromAnalysis = itemById(spec.source)?.kind === 'analysis';
  const style = useProject.getState().view?.p_value_style ?? 'apa';
  const ann = spec.annotations ?? [];
  const choice = await openDialog<{ from: number; to: number; text: string } | 'analysis' | null>(
    (close) => <BracketDialog names={names} annotations={ann} fromAnalysis={fromAnalysis} close={close} />,
    null,
  );
  if (!choice) return;
  let next: Annotation[];
  if (choice === 'analysis') {
    const sheet = await query((e) => obj.getResults(e, spec.source));
    if (!sheet) return;
    next = bracketsFromSheet(ann, sheet, names, style);
    if (next === ann) {
      notify('The analysis reports no comparison between the plotted categories.');
      return;
    }
  } else next = placeBracket(ann, choice.from, choice.to, choice.text, names.length);
  await setGraphValue(graph, '/annotations', next as never);
}

function BracketDialog({
  names,
  annotations,
  fromAnalysis,
  close,
}: {
  names: string[];
  annotations: Annotation[];
  fromAnalysis: boolean;
  close: (v: { from: number; to: number; text: string } | 'analysis' | null) => void;
}) {
  const pair = nextFreePair(annotations, names.length) ?? [0, 1];
  const [a, setA] = useState(pair[0]);
  const [b, setB] = useState(pair[1]);
  const [text, setText] = useState('*');
  const ok = a !== b && text.trim().length > 0;
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="bracket-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        if (ok) close({ from: a, to: b, text: text.trim() });
      }}
    >
      <DialogHeader title="Add significance bracket" sub="Between two categories; overlapping brackets stack." />
      {fromAnalysis && (
        <Button testId="bracket-from-analysis" onClick={() => close('analysis')}>
          Brackets from the analysis
        </Button>
      )}
      <div className="form-row">
        <Field label="From">
          {(lid) => <Select size="md" labelledBy={lid} value={a} onChange={setA} testId="bracket-from" options={names.map((n, i) => ({ value: i, label: n }))} />}
        </Field>
        <Field label="To">
          {(lid) => <Select size="md" labelledBy={lid} value={b} onChange={setB} testId="bracket-to" options={names.map((n, i) => ({ value: i, label: n }))} />}
        </Field>
      </div>
      <Field label="Label" hint="Asterisks, “ns”, or a p value such as p = .003.">
        {(lid) => <TextInput aria-labelledby={lid} value={text} onChange={(e) => setText(e.target.value)} testId="bracket-text" />}
      </Field>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!ok} testId="bracket-add">
          Add bracket
        </Button>
      </footer>
    </form>
  );
}

/** The data value at the middle of the plot along an axis, read from the rendered SVG. */
function middleValue(graph: Id, axis: 'x' | 'y'): number {
  const st = graphState(graph);
  const r = st.rendered;
  const plot = r?.targets.find((t) => t.id === 'plot-area')?.bbox;
  if (!r || !plot) return 0;
  const doc = new DOMParser().parseFromString(r.svg, 'image/svg+xml');
  const map = axisMap(doc.documentElement, axis, st.spec?.axes[axis].scale);
  if (!map) return axis === 'x' ? 1 : 0;
  const pt = axis === 'x' ? plot.x + plot.width / 2 : plot.y + plot.height / 2;
  return tidy(valueAt(map, pt));
}

export async function addReferenceLine(graph: Id): Promise<void> {
  const spec = graphState(graph).spec;
  if (!spec) return;
  const r = await openDialog<{ axis: 'x' | 'y'; value: number; label: string } | null>(
    (close) => <RefLineDialog initial={middleValue(graph, 'y')} close={close} />,
    null,
  );
  if (!r) return;
  const a: Annotation = { kind: 'reference_line', axis: r.axis, value: r.value, dashed: true, ...(r.label ? { label: r.label } : {}) };
  await setGraphValue(graph, '/annotations', [...(spec.annotations ?? []), a] as never);
}

function RefLineDialog({ initial, close }: { initial: number; close: (v: { axis: 'x' | 'y'; value: number; label: string } | null) => void }) {
  const [axis, setAxis] = useState<'x' | 'y'>('y');
  const [value, setValue] = useState(String(initial));
  const [label, setLabel] = useState('');
  const v = Number(value.replace(',', '.'));
  const ok = value.trim() !== '' && Number.isFinite(v);
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="refline-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        if (ok) close({ axis, value: v, label: label.trim() });
      }}
    >
      <DialogHeader title="Add reference line" sub="A dashed line across the plot at a value, e.g. a detection limit." />
      <div className="form-row">
        <Field label="Line">
          {(lid) => (
            <Segmented
              size="md"
              labelledBy={lid}
              value={axis}
              onChange={setAxis}
              choices={[
                { value: 'y' as const, label: 'Horizontal', hint: 'At a Y value' },
                { value: 'x' as const, label: 'Vertical', hint: 'At an X value' },
              ]}
            />
          )}
        </Field>
        <Field label="Value" className="field-num">
          {(lid) => <NumberField size="md" labelledBy={lid} value={Number(value)} format={() => value} onText={(t) => setValue(t)} nullable={false} invalid={!ok} testId="refline-value" />}
        </Field>
      </div>
      <Field label="Label (optional)">{(lid) => <TextInput aria-labelledby={lid} value={label} onChange={(e) => setLabel(e.target.value)} placeholder="e.g. LLOQ" />}</Field>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!ok}>
          Add line
        </Button>
      </footer>
    </form>
  );
}

export async function addText(graph: Id): Promise<void> {
  const spec = graphState(graph).spec;
  if (!spec) return;
  const text = await openDialog<string | null>((close) => <TextDialog close={close} />, null);
  if (!text) return;
  const a: Annotation = { kind: 'text_label', text, x: middleValue(graph, 'x'), y: middleValue(graph, 'y') };
  await setGraphValue(graph, '/annotations', [...(spec.annotations ?? []), a] as never);
}

function TextDialog({ close }: { close: (v: string | null) => void }) {
  const [t, setT] = useState('');
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="text-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        if (t.trim()) close(t.trim());
      }}
    >
      <DialogHeader title="Add text" sub="Placed in the middle of the plot; drag it where it belongs." />
      <Field label="Text" hint="^ and _ for super- and subscripts, \alpha for Greek.">
        {(lid) => <TextInput aria-labelledby={lid} value={t} onChange={(e) => setT(e.target.value)} placeholder="R^2 = 0.98" testId="text-value" />}
      </Field>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!t.trim()}>
          Add text
        </Button>
      </footer>
    </form>
  );
}

export async function customSize(graph: Id): Promise<void> {
  const spec = graphState(graph).spec;
  if (!spec) return;
  const r = await openDialog<{ width: number; height: number } | null>(
    (close) => <SizeDialog width={spec.style.size_mm.width} height={spec.style.size_mm.height} close={close} />,
    null,
  );
  if (r) await setGraphValue(graph, '/style/size_mm', r);
}

function SizeDialog({ width, height, close }: { width: number; height: number; close: (v: { width: number; height: number } | null) => void }) {
  const [w, setW] = useState(String(width));
  const [h, setH] = useState(String(height));
  const wv = Number(w);
  const hv = Number(h);
  const ok = wv >= 10 && wv <= 1000 && hv >= 10 && hv <= 1000;
  return (
    <form
      className="modal-body modal-narrow"
      onSubmit={(e) => {
        e.preventDefault();
        if (ok) close({ width: wv, height: hv });
      }}
    >
      <DialogHeader title="Graph size" sub="In millimetres; fonts and lines keep their point sizes." />
      <div className="form-row">
        <Field label="Width" className="field-num">
          {(lid) => <NumberField size="md" labelledBy={lid} value={Number(w)} onText={(t) => setW(t)} unit="mm" min={10} max={1000} nullable={false} invalid={!ok} testId="size-width" />}
        </Field>
        <Field label="Height" className="field-num">
          {(lid) => <NumberField size="md" labelledBy={lid} value={Number(h)} onText={(t) => setH(t)} unit="mm" min={10} max={1000} nullable={false} invalid={!ok} testId="size-height" />}
        </Field>
      </div>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!ok}>
          Set size
        </Button>
      </footer>
    </form>
  );
}

// ---- export ------------------------------------------------------------------------------------

const FILTERS: Record<string, { name: string; extensions: string[] }> = {
  svg: { name: 'SVG image', extensions: ['svg'] },
  pdf: { name: 'PDF document', extensions: ['pdf'] },
  png: { name: 'PNG image', extensions: ['png'] },
};

function formatOf(c: ExportChoice): GraphFormat {
  return c.format === 'png' ? { format: 'png', dpi: Math.round(c.dpi) } : { format: c.format };
}

export async function exportGraphDialog(graph: Id): Promise<void> {
  await flushGraph(graph);
  const item = itemById(graph);
  const spec = graphState(graph).spec ?? (await query((e) => obj.getGraphSpec(e, graph)));
  if (!item || !spec) return;
  const desktop = isDesktop();
  const c = await openDialog<ExportChoice | null>(
    (close) => (
      <ExportGraphDialog title={item.title} size={spec.style.size_mm} transparent={!!spec.style.transparent} desktop={desktop} close={close} />
    ),
    null,
  );
  if (!c) return;
  if (c.format === 'png' && c.transparent !== !!spec.style.transparent) {
    await setGraphValue(graph, '/style/transparent', c.transparent ? true : null);
  }
  const format = formatOf(c);
  try {
    if (desktop) {
      const path = await desktopSavePath(c.filename, [FILTERS[c.format]!]);
      if (!path) return;
      await withEngine((e) => obj.exportGraphPath(e, graph, format, c.size, path));
      notify(`Exported ${path.split(/[\\/]/).pop()}.`);
    } else {
      const b = await withEngine((e) => obj.exportGraph(e, graph, format, c.size));
      downloadBytes(c.filename, b.mime, b.base64);
    }
  } catch (err) {
    notifyError(err);
  }
}

export async function copySvg(graph: Id): Promise<void> {
  await flushGraph(graph);
  try {
    const r = graphState(graph).rendered ?? (await withEngine((e) => obj.renderGraph(e, graph)));
    await navigator.clipboard.writeText(r.svg);
    notify('Copied the SVG.');
  } catch (err) {
    notifyError(err);
  }
}

export async function exportLayoutDialog(layout: Id, size: { width: number; height: number }): Promise<void> {
  const item = itemById(layout);
  if (!item) return;
  const desktop = isDesktop();
  const c = await openDialog<ExportChoice | null>(
    (close) => <ExportGraphDialog title={item.title} size={size} transparent={false} allowSize={false} desktop={desktop} close={close} />,
    null,
  );
  if (!c) return;
  try {
    if (desktop) {
      const path = await desktopSavePath(c.filename, [FILTERS[c.format]!]);
      if (!path) return;
      await withEngine((e) => obj.exportLayoutPath(e, layout, formatOf(c), path));
      notify(`Exported ${path.split(/[\\/]/).pop()}.`);
    } else {
      const b = await withEngine((e) => obj.exportLayout(e, layout, formatOf(c)));
      downloadBytes(c.filename, b.mime, b.base64);
    }
  } catch (err) {
    notifyError(err);
  }
}

// ---- layouts -----------------------------------------------------------------------------------

export async function newLayout(): Promise<void> {
  const graphs = useProject.getState().view?.items.filter((i) => i.kind === 'graph') ?? [];
  if (graphs.length === 0) {
    notify('Make a graph first; a layout arranges graphs on a page.');
    return;
  }
  const r = await openDialog<{ graphs: Id[]; columns: number; title: string } | null>(
    (close) => <LayoutDialog graphs={graphs.map((g) => ({ id: g.id, title: g.title }))} close={close} />,
    null,
  );
  if (!r) return;
  await mutate((e) => obj.createLayout(e, r.title || null, r.graphs, r.columns));
}

function LayoutDialog({
  graphs,
  close,
}: {
  graphs: { id: Id; title: string }[];
  close: (v: { graphs: Id[]; columns: number; title: string } | null) => void;
}) {
  const active = useProject.getState().activeId;
  const [picked, setPicked] = useState<Id[]>(() => (graphs.length <= 4 ? graphs.map((g) => g.id) : graphs.filter((g) => g.id === active).map((g) => g.id)));
  const [columns, setColumns] = useState(Math.min(2, Math.max(1, picked.length)));
  const [title, setTitle] = useState('');
  const ok = picked.length > 0 && columns >= 1 && columns <= 6;
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="layout-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        if (ok) close({ graphs: picked, columns, title: title.trim() });
      }}
    >
      <DialogHeader title="New layout" sub="Graphs on one page in a grid, lettered A, B, C in reading order." />
      <div className="field">
        <span className="field-label">Graphs, in order</span>
        <div className="check-list">
          {graphs.map((g) => (
            <Checkbox
              key={g.id}
              className="check-row"
              checked={picked.includes(g.id)}
              testId={`layout-pick-${g.title}`}
              onChange={(on) => setPicked(on ? [...picked, g.id] : picked.filter((x) => x !== g.id))}
            >
              <span className="layout-letter" aria-hidden="true">
                {picked.includes(g.id) ? String.fromCharCode(65 + picked.indexOf(g.id)) : ''}
              </span>
              {g.title}
            </Checkbox>
          ))}
        </div>
      </div>
      <div className="form-row">
        <Field label="Columns" className="field-num">
          {(lid) => (
            <NumberField
              size="md"
              labelledBy={lid}
              value={columns}
              min={1}
              max={6}
              integer
              nullable={false}
              testId="layout-columns"
              onText={(_, v) => Number.isFinite(v) && setColumns(Math.max(1, Math.min(6, Math.round(v))))}
              onCommit={(v) => v !== null && setColumns(Math.max(1, Math.min(6, v)))}
            />
          )}
        </Field>
        <Field label="Title (optional)">{(lid) => <TextInput aria-labelledby={lid} value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Figure 1" />}</Field>
      </div>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!ok} testId="layout-create">
          Create layout
        </Button>
      </footer>
    </form>
  );
}
