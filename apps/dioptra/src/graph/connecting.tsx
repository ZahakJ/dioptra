// Inspector sections for the categorical families (column, strip, box, violin): "Connecting
// lines" (subject lines and the mean/median line, Prism's before–after lines) and "Bars and
// points" (orientation, summary, jitter, bar fill and widths, error-bar direction and caps).
// Every control writes one path under `/options`; each write is one undo step.
import type { InspectorContext } from '../app/inspector';
import { Segmented, Select } from '../ui';
import type { GraphSpecT } from '../engine/objects';
import type { Id, JsonValue, TableMeta } from '../engine/types';
import { PALETTES, paletteColours } from './families';
import { ColourField, NumField, Row } from './fields';
import { setGraphValue, useGraph } from './store';

/** Families that draw connecting lines and the bar options. */
export const CATEGORICAL = ['column', 'bar', 'strip', 'box', 'violin'];
export const isCategorical = (spec: GraphSpecT | null | undefined) => !!spec && CATEGORICAL.includes(spec.family);

export type Connect = 'none' | 'individuals' | 'means' | 'medians' | 'both';

export interface ConnectingT {
  connect?: Connect;
  style?: 'solid' | 'dashed' | 'dotted' | 'dash_dot' | 'none';
  width_pt?: number;
  colour_by?: 'single' | 'dataset' | 'subject' | 'group';
  colour?: string;
  palette?: string;
  group_column?: number;
  opacity?: number;
  missing?: 'gap' | 'skip';
  direction?: 'within_groups' | 'across_groups';
  summary_style?: 'solid' | 'dashed' | 'dotted' | 'dash_dot' | 'none';
  summary_width_pt?: number;
  summary_colour?: string;
}

export function connectingOf(spec: GraphSpecT): ConnectingT {
  return ((spec.options as Record<string, unknown>).connecting ?? {}) as ConnectingT;
}

const opt = <T,>(spec: GraphSpecT, key: string, fallback: T): T => ((spec.options as Record<string, unknown>)[key] as T | undefined) ?? fallback;

export const CONNECT_CHOICES: { value: Connect; label: string; hint: string }[] = [
  { value: 'none', label: 'Off', hint: 'No connecting lines' },
  { value: 'individuals', label: 'Subjects', hint: 'One line per row (subject) across the bars' },
  { value: 'means', label: 'Mean', hint: 'Join the means' },
  { value: 'medians', label: 'Median', hint: 'Join the medians' },
  { value: 'both', label: 'Both', hint: 'Subject lines and the mean (or median) line' },
];

const STYLES = [
  { value: 'solid' as const, label: 'Solid' },
  { value: 'dashed' as const, label: 'Dashed' },
  { value: 'dotted' as const, label: 'Dotted' },
];

const P = '/options/connecting';

/** The connecting-line controls. `meta` names the columns for "by grouping column". */
export function ConnectingFields({ graph, spec, meta }: { graph: Id; spec: GraphSpecT; meta: TableMeta | undefined }) {
  const c = connectingOf(spec);
  const connect = c.connect ?? 'none';
  const set = (key: keyof ConnectingT, v: JsonValue | undefined) => void setGraphValue(graph, `${P}/${key}`, v ?? null);
  const subjects = connect === 'individuals' || connect === 'both';
  const summary = connect !== 'none' && connect !== 'individuals';
  const colourBy = c.colour_by ?? 'single';
  const grouped = meta?.kind === 'grouped';
  const stacked = opt(spec, 'arrangement', 'grouped') !== 'grouped' && grouped && (spec.family === 'column' || spec.family === 'bar');
  const colours = paletteColours(spec.style.palette);
  return (
    <div className="g-list" data-testid="connecting-fields">
      <Segmented
        label="Connect"
        className="seg-full seg-tight"
        testId="conn-mode"
        choices={CONNECT_CHOICES.map((x) => ({ value: x.value, label: x.label, hint: x.hint, testId: `conn-${x.value}` }))}
        value={connect}
        onChange={(v) => set('connect', v === 'none' ? null : v)}
      />
      {stacked && connect !== 'none' && <p className="muted small">Stacked bars draw no connecting lines.</p>}
      {connect !== 'none' && (
        <Row label="Missing" hint="A missing value leaves a gap in the line, or the line joins the values either side">
          {(lid) => (
            <Segmented
              labelledBy={lid}
              className="seg-full"
              testId="conn-missing"
              choices={[
                { value: 'gap' as const, label: 'Gap' },
                { value: 'skip' as const, label: 'Join across' },
              ]}
              value={c.missing ?? 'gap'}
              onChange={(v) => set('missing', v === 'gap' ? null : v)}
            />
          )}
        </Row>
      )}
      {connect !== 'none' && grouped && (
        <Row label="Direction" hint="Within each group across its data sets (matched subcolumns), or each data set across the groups">
          {(lid) => (
            <Segmented
              labelledBy={lid}
              className="seg-full"
              testId="conn-direction"
              choices={[
                { value: 'within_groups' as const, label: 'Within' },
                { value: 'across_groups' as const, label: 'Across' },
              ]}
              value={c.direction ?? 'within_groups'}
              onChange={(v) => set('direction', v === 'within_groups' ? null : v)}
            />
          )}
        </Row>
      )}
      {subjects && (
        <>
          <div className="g-subhead">Subject lines</div>
          <Row label="Style">
            {(lid) => (
              <Segmented labelledBy={lid} className="seg-full" testId="conn-style" choices={STYLES} value={c.style ?? 'solid'} onChange={(v) => set('style', v === 'solid' ? null : v)} />
            )}
          </Row>
          <NumField graph={graph} path={`${P}/width_pt`} value={c.width_pt} label="Width" unit="pt" min={0.1} max={6} placeholder="Theme" testId="conn-width" />
          <Row label="Colour by">
            <Select
              label="Colour lines by"
              testId="conn-colour-by"
              value={colourBy}
              options={[
                { value: 'single', label: 'One colour' },
                { value: 'dataset', label: 'Data set', hint: 'Each segment takes the colour of the bar it starts from' },
                { value: 'subject', label: 'Subject', hint: 'Each row its own colour' },
                { value: 'group', label: 'Grouping column', hint: 'Rows coloured by the value of a column, with a legend', disabled: grouped },
              ]}
              onChange={(v) => set('colour_by', v === 'single' ? null : v)}
            />
          </Row>
          {colourBy === 'single' && (
            <Row label="Colour">
              <ColourField value={c.colour} fallback="#222222" swatches={['#222222', '#777777', ...colours]} label="Line colour" testId="conn-colour" onChange={(h) => set('colour', h)} />
            </Row>
          )}
          {(colourBy === 'subject' || colourBy === 'group') && (
            <Row label="Palette">
              <Select
                label="Line palette"
                testId="conn-palette"
                value={c.palette ?? ''}
                options={[{ value: '', label: 'Graph palette', swatches: colours }, ...PALETTES.map((p) => ({ value: p.value, label: p.label, swatches: p.colours }))]}
                onChange={(v) => set('palette', v || null)}
              />
            </Row>
          )}
          {colourBy === 'group' && (
            <Row label="Column">
              <Select
                label="Grouping column"
                testId="conn-group-column"
                value={c.group_column ?? -1}
                options={[{ value: -1, label: 'Choose a column' }, ...(meta?.datasets ?? []).map((d, i) => ({ value: i, label: d.name || `Data set ${i + 1}` }))]}
                onChange={(v) => set('group_column', v < 0 ? null : v)}
              />
            </Row>
          )}
          <NumField
            graph={graph}
            path=""
            value={Math.round((c.opacity ?? 0.55) * 100)}
            label="Opacity"
            unit="%"
            min={0}
            max={100}
            step={5}
            placeholder="55"
            testId="conn-opacity"
            onCommit={(v) => set('opacity', v === null ? null : Math.max(0, Math.min(100, v)) / 100)}
          />
        </>
      )}
      {summary && (
        <>
          <div className="g-subhead">{connect === 'medians' ? 'Median line' : connect === 'means' ? 'Mean line' : 'Summary line'}</div>
          <Row label="Style">
            {(lid) => (
              <Segmented
                labelledBy={lid}
                className="seg-full"
                testId="conn-summary-style"
                choices={STYLES}
                value={c.summary_style ?? 'solid'}
                onChange={(v) => set('summary_style', v === 'solid' ? null : v)}
              />
            )}
          </Row>
          <NumField graph={graph} path={`${P}/summary_width_pt`} value={c.summary_width_pt} label="Width" unit="pt" min={0.1} max={8} placeholder="Theme" testId="conn-summary-width" />
          <Row label="Colour">
            <ColourField value={c.summary_colour} fallback="#222222" swatches={['#222222', '#777777', ...colours]} label="Summary line colour" onChange={(h) => set('summary_colour', h)} />
          </Row>
        </>
      )}
    </div>
  );
}

/** Orientation, summary, jitter, bars, error bars. */
export function BarFields({ graph, spec }: { graph: Id; spec: GraphSpecT }) {
  const set = (key: string, v: JsonValue | null) => void setGraphValue(graph, `/options/${key}`, v);
  const fam = spec.family;
  const bars = fam === 'column' || fam === 'bar';
  const pct = (v: number | undefined) => (v === undefined ? undefined : Math.round(v * 100));
  return (
    <div className="g-list" data-testid="bar-fields">
      <Row label="Orientation">
        {(lid) => (
          <Segmented
            labelledBy={lid}
            className="seg-full"
            testId="bar-orientation"
            choices={[
              { value: 'vertical' as const, label: 'Vertical' },
              { value: 'horizontal' as const, label: 'Horizontal' },
            ]}
            value={opt(spec, 'orientation', 'vertical')}
            onChange={(v) => set('orientation', v === 'vertical' ? null : v)}
          />
        )}
      </Row>
      {fam !== 'violin' && (
        <Row label="Summary">
          {(lid) => (
            <Segmented
              labelledBy={lid}
              className="seg-full"
              testId="bar-summary"
              choices={[
                { value: 'mean' as const, label: 'Mean' },
                { value: 'median' as const, label: 'Median' },
              ]}
              value={opt(spec, 'summary', 'mean')}
              onChange={(v) => set('summary', v)}
            />
          )}
        </Row>
      )}
      <NumField
        graph={graph}
        path=""
        value={pct(opt<number | undefined>(spec, 'jitter', 0.5))}
        label="Jitter"
        unit="%"
        min={0}
        max={100}
        step={10}
        placeholder="50"
        testId="bar-jitter"
        onCommit={(v) => set('jitter', v === null ? null : Math.max(0, Math.min(100, v)) / 100)}
      />
      {bars && (
        <Row label="Fill">
          {(lid) => (
            <Segmented
              labelledBy={lid}
              className="seg-full"
              testId="bar-fill"
              choices={[
                { value: 'tint' as const, label: 'Tint' },
                { value: 'solid' as const, label: 'Solid' },
                { value: 'empty' as const, label: 'Outline' },
              ]}
              value={opt(spec, 'bar_fill', 'tint')}
              onChange={(v) => set('bar_fill', v === 'tint' ? null : v)}
            />
          )}
        </Row>
      )}
      <NumField
        graph={graph}
        path=""
        value={pct(opt<number | undefined>(spec, 'bar_width', undefined))}
        label={bars ? 'Bar width' : 'Width'}
        unit="%"
        min={5}
        max={100}
        step={5}
        placeholder="Auto"
        testId="bar-width"
        onCommit={(v) => set('bar_width', v === null ? null : v / 100)}
      />
      <NumField
        graph={graph}
        path=""
        value={pct(opt<number | undefined>(spec, 'group_width', undefined))}
        label="Slot width"
        unit="%"
        min={20}
        max={100}
        step={5}
        placeholder="84"
        testId="bar-group-width"
        onCommit={(v) => set('group_width', v === null ? null : v / 100)}
      />
      {fam !== 'box' && fam !== 'violin' && (
        <>
          <Row label="Error bars" hint="Which half of each error bar is drawn">
            {(lid) => (
              <Segmented
                labelledBy={lid}
                className="seg-full"
                testId="bar-error-dir"
                choices={[
                  { value: 'both' as const, label: 'Both' },
                  { value: 'above' as const, label: 'Above' },
                  { value: 'below' as const, label: 'Below' },
                ]}
                value={opt(spec, 'error_direction', 'both')}
                onChange={(v) => set('error_direction', v === 'both' ? null : v)}
              />
            )}
          </Row>
          <NumField graph={graph} path="/options/cap_pt" value={opt<number | undefined>(spec, 'cap_pt', undefined)} label="Cap width" unit="pt" min={0} max={20} placeholder="Auto" testId="bar-cap" />
        </>
      )}
      {fam === 'strip' && (
        <Row label="Mean line">
          {(lid) => (
            <Segmented
              labelledBy={lid}
              className="seg-full"
              choices={[
                { value: true, label: 'Show' },
                { value: false, label: 'Hide' },
              ]}
              value={opt(spec, 'summary_line', true)}
              onChange={(v) => set('summary_line', v ? null : false)}
            />
          )}
        </Row>
      )}
    </div>
  );
}

export function ConnectingSection({ ctx, meta }: { ctx: InspectorContext; meta: TableMeta | undefined }) {
  const id = ctx.item!.id;
  const { spec } = useGraph(id);
  if (!spec || !isCategorical(spec)) return null;
  return <ConnectingFields graph={id} spec={spec} meta={meta} />;
}

export function BarsSection({ ctx }: { ctx: InspectorContext }) {
  const id = ctx.item!.id;
  const { spec } = useGraph(id);
  if (!spec || !isCategorical(spec)) return null;
  return <BarFields graph={id} spec={spec} />;
}
