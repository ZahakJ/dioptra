// Phase 5B registrations: the analysis flow, results sheets, the graph editor, layouts,
// transforms. Called once from the app shell after the core registrations.
import { ChartSpline, Sigma } from 'lucide-react';
import { Button } from './ui';
import './styles/features.css';
import { analyze, analyzableTable, editAnalysisOptions } from './analysis/actions';
import { kindTitle } from './analysis/AnalyzeDialog';
import { commands } from './app/commands';
import { registerInspectorSection, type InspectorContext } from './app/inspector';
import { KV } from './app/fields';
import { registerRailActions } from './app/ProjectRail';
import { registerTableTool } from './app/tableTools';
import { registerWorkArea } from './app/workArea';
import {
  addBracket,
  addReferenceLine,
  addText,
  copySvg,
  exportGraphDialog,
  makeGraphFromAnalysis,
  newGraph,
  newLayout,
} from './graph/actions';
import { GraphEditor } from './graph/GraphEditor';
import { registerGraphInspector } from './graph/inspector';
import { LayoutView } from './graph/LayoutView';
import { ResultsSheetView, useSheet } from './results/ResultsSheetView';
import { itemById, useProject } from './store/project';
import { OPS, transformTable } from './transform/TransformDialog';
import type { Id } from './engine/types';

const active = () => {
  const { view, activeId } = useProject.getState();
  return view?.items.find((i) => i.id === activeId);
};
const activeOf = (kind: string) => {
  const a = active();
  return a?.kind === kind ? a.id : null;
};
const hasTable = () => !!useProject.getState().view?.items.some((i) => i.kind === 'table');
const hasGraph = () => !!useProject.getState().view?.items.some((i) => i.kind === 'graph');
const onGraph = (fn: (id: Id) => unknown) => () => {
  const id = activeOf('graph');
  return id ? fn(id) : undefined;
};

function AnalysisSection({ ctx }: { ctx: InspectorContext }) {
  const item = ctx.item!;
  const table = item.derived_from ? itemById(item.derived_from) : undefined;
  const { sheet } = useSheet(item.id);
  return (
    <div className="kv-list">
      <KV label="Analysis">{kindTitle(item.subtype ?? '')}</KV>
      <KV label="Data">{table?.title ?? 'missing'}</KV>
      <KV label="State">{item.stale || sheet?.stale ? 'Out of date' : 'Up to date'}</KV>
      {sheet && <KV label="Tables">{sheet.sections.length}</KV>}
      <div className="btn-row">
        <Button size="sm" onClick={() => void editAnalysisOptions(item.id)} testId="insp-analysis-options">
          Options…
        </Button>
        <Button size="sm" onClick={() => void makeGraphFromAnalysis(item.id)} testId="insp-make-graph" tipText={sheet?.graph_family === 'paired' ? 'Bars of the mean with a line joining each subject’s values' : undefined}>
          {sheet?.graph_family === 'paired' ? 'Graph with connecting lines' : 'Make graph'}
        </Button>
      </div>
    </div>
  );
}

function AnalyzeTool({ tableId }: { tableId: Id }) {
  return (
    <>
      <Button size="sm" icon={<Sigma size={16} strokeWidth={1.5} aria-hidden="true" />} onClick={() => void analyze(tableId)} testId="tool-analyze" aria-label="Analyze" tipText="Analyze this table" shortcut="Mod+Shift+A">
        Analyze…
      </Button>
      <Button size="sm" icon={<ChartSpline size={16} strokeWidth={1.5} aria-hidden="true" />} onClick={() => void newGraph(tableId)} testId="tool-graph" aria-label="New graph" tipText="New graph of this table" shortcut="Mod+Shift+G">
        Graph…
      </Button>
    </>
  );
}

let done = false;
export function registerFeatures(): void {
  if (done) return;
  done = true;
  registerWorkArea('analysis', ResultsSheetView);
  registerWorkArea('graph', GraphEditor);
  registerWorkArea('layout', LayoutView);
  registerGraphInspector();
  registerInspectorSection({ id: 'analysis', title: 'Analysis', order: 40, when: (c) => c.item?.kind === 'analysis', component: AnalysisSection });
  registerTableTool('analyze', AnalyzeTool);
  registerRailActions((item) => {
    if (item.kind === 'table')
      return [
        { label: 'Analyze...', shortcut: 'Mod+Shift+A', testId: 'rail-analyze', run: () => analyze(item.id) },
        { label: 'New graph...', testId: 'rail-graph', run: () => newGraph(item.id) },
        { label: 'Transform...', run: () => transformTable() },
      ];
    if (item.kind === 'analysis')
      return [
        { label: 'Options...', run: () => editAnalysisOptions(item.id) },
        { label: 'Make graph', run: () => makeGraphFromAnalysis(item.id) },
      ];
    if (item.kind === 'graph')
      return [
        { label: 'Export...', run: () => exportGraphDialog(item.id) },
        { label: 'New layout...', run: () => newLayout() },
      ];
    return [];
  });

  commands.register(
    // ---- Analyze ----
    {
      id: 'analyze.new',
      title: 'Analyze...',
      menu: 'Analyze',
      group: 'new',
      shortcut: 'Mod+Shift+A',
      keywords: ['statistics', 't test', 'anova', 'test', 'descriptive', 'regression', 'correlation'],
      enabled: () => !!analyzableTable(),
      run: () => analyze(),
    },
    {
      id: 'analyze.options',
      title: 'Analysis options...',
      menu: 'Analyze',
      group: 'sheet',
      keywords: ['rerun', 'change', 'settings'],
      enabled: () => !!activeOf('analysis'),
      run: () => {
        const id = activeOf('analysis');
        if (id) void editAnalysisOptions(id);
      },
    },
    {
      id: 'analyze.makeGraph',
      title: 'Make graph from results',
      menu: 'Analyze',
      group: 'sheet',
      keywords: ['plot', 'chart', 'figure'],
      enabled: () => !!activeOf('analysis'),
      run: () => {
        const id = activeOf('analysis');
        if (id) void makeGraphFromAnalysis(id);
      },
    },
    // ---- Graph ----
    {
      id: 'graph.new',
      title: 'New graph...',
      menu: 'Graph',
      group: 'new',
      shortcut: 'Mod+Shift+G',
      keywords: ['plot', 'chart', 'figure', 'column', 'scatter', 'box'],
      enabled: hasTable,
      run: () => newGraph(),
    },
    { id: 'layout.new', title: 'New layout...', menu: 'Graph', group: 'new', keywords: ['figure', 'panels', 'multi-panel'], enabled: hasGraph, run: () => newLayout() },
    { id: 'graph.addBracket', title: 'Add significance bracket...', menu: 'Graph', group: 'annotate', keywords: ['asterisks', 'p value', 'stars'], enabled: () => !!activeOf('graph'), run: onGraph(addBracket) },
    { id: 'graph.addRefLine', title: 'Add reference line...', menu: 'Graph', group: 'annotate', keywords: ['threshold', 'limit'], enabled: () => !!activeOf('graph'), run: onGraph(addReferenceLine) },
    { id: 'graph.addText', title: 'Add text...', menu: 'Graph', group: 'annotate', keywords: ['label', 'note'], enabled: () => !!activeOf('graph'), run: onGraph(addText) },
    {
      id: 'graph.export',
      title: 'Export graph...',
      menu: 'Graph',
      group: 'export',
      shortcut: 'Mod+Alt+E',
      keywords: ['pdf', 'png', 'svg', 'save', 'figure'],
      enabled: () => !!activeOf('graph'),
      run: onGraph(exportGraphDialog),
    },
    { id: 'graph.copySvg', title: 'Copy graph as SVG', menu: 'Graph', group: 'export', keywords: ['clipboard'], enabled: () => !!activeOf('graph'), run: onGraph(copySvg) },
    // ---- Data ----
    {
      id: 'data.transform',
      title: 'Transform...',
      menu: 'Data',
      group: 'transform',
      keywords: ['transpose', 'pivot', 'filter', 'sort', 'normalize', 'baseline', 'reshape'],
      enabled: hasTable,
      run: () => transformTable(),
    },
    ...OPS.map((o) => ({
      id: `data.transform.${o.op}`,
      title: `${o.label}...`,
      group: 'ops',
      keywords: ['transform', o.hint],
      enabled: hasTable,
      run: () => transformTable(o.op),
    })),
  );
}
