// Graph families (docs/GRAPHS.md), their display names, and which suit each table kind.
import type { TableKind } from '../engine/types';

export const FAMILY_NAMES: Record<string, string> = {
  scatter: 'Scatter (XY)',
  line: 'Line',
  column: 'Column (bar)',
  box: 'Box',
  violin: 'Violin',
  strip: 'Strip (points)',
  raincloud: 'Raincloud',
  before_after: 'Before–after',
  estimation: 'Estimation',
  superplot: 'SuperPlot',
  histogram: 'Histogram',
  density: 'Density',
  heatmap: 'Heatmap',
  kaplan_meier: 'Kaplan–Meier',
  roc: 'ROC curve',
  bland_altman: 'Bland–Altman',
  volcano: 'Volcano',
  forest: 'Forest',
  correlation_matrix: 'Correlation matrix',
  pie: 'Pie',
  donut: 'Donut',
  qq: 'QQ plot',
  residuals: 'Residuals',
};

const BY_KIND: Record<TableKind, string[]> = {
  column: ['column', 'box', 'violin', 'strip', 'raincloud', 'before_after', 'estimation', 'histogram', 'density', 'qq', 'roc', 'bland_altman', 'pie', 'donut'],
  xy: ['scatter', 'line', 'residuals'],
  grouped: ['column', 'box', 'strip', 'heatmap', 'forest'],
  contingency: ['column', 'heatmap', 'pie', 'donut'],
  survival: ['kaplan_meier'],
  parts: ['pie', 'donut', 'column'],
  variables: ['scatter', 'correlation_matrix', 'heatmap', 'volcano', 'histogram', 'density', 'box', 'column', 'qq', 'forest'],
  nested: ['superplot', 'column', 'box', 'strip'],
};

/** Families that draw a table of this kind, the natural one first. */
export function familiesFor(kind: TableKind | string | undefined): string[] {
  return BY_KIND[kind as TableKind] ?? Object.keys(FAMILY_NAMES);
}

export const familyName = (f: string) => FAMILY_NAMES[f] ?? f.replace(/_/g, ' ');

/** The graph that best shows an analysis' result. */
export function familyForAnalysis(kind: string): string {
  switch (kind) {
    case 't_test':
    case 'one_way_anova':
    case 'two_way_anova':
    case 'descriptive':
    case 'chi_square':
    case 'fisher_exact':
      return 'column';
    case 'mann_whitney':
    case 'kruskal_wallis':
      return 'box';
    case 'wilcoxon':
    case 'repeated_measures_anova':
    case 'friedman':
      return 'before_after';
    case 'correlation':
    case 'linear_regression':
    case 'nonlinear_regression':
    case 'pca':
      return 'scatter';
    case 'normality':
      return 'qq';
    case 'outliers':
      return 'strip';
    case 'survival':
      return 'kaplan_meier';
    case 'roc':
      return 'roc';
    case 'bland_altman':
      return 'bland_altman';
    default:
      return 'column';
  }
}

export const THEMES: { value: string; label: string }[] = [
  { value: 'instrument', label: 'Instrument' },
  { value: 'night', label: 'Night' },
  { value: 'journal', label: 'Journal' },
];

export const PALETTES: { value: string; label: string; colours: string[] }[] = [
  { value: 'okabe_ito', label: 'Okabe–Ito', colours: ['#000000', '#E69F00', '#56B4E9', '#009E73', '#0072B2', '#D55E00', '#CC79A7', '#F0E442'] },
  { value: 'tol_bright', label: 'Tol bright', colours: ['#4477AA', '#EE6677', '#228833', '#CCBB44', '#66CCEE', '#AA3377', '#BBBBBB'] },
  { value: 'tol_muted', label: 'Tol muted', colours: ['#332288', '#88CCEE', '#44AA99', '#117733', '#999933', '#DDCC77', '#CC6677', '#882255', '#AA4499'] },
  { value: 'tol_light', label: 'Tol light', colours: ['#77AADD', '#EE8866', '#EEDD88', '#FFAABB', '#99DDFF', '#44BB99', '#BBCC33', '#AAAA00', '#DDDDDD'] },
  { value: 'viridis', label: 'Viridis', colours: ['#440154', '#482878', '#3E4A89', '#31688E', '#26828E', '#1F9E89', '#35B779', '#6DCD59'] },
  { value: 'cividis', label: 'Cividis', colours: ['#00224E', '#123570', '#3B496C', '#575D6D', '#707173', '#8A8779', '#A69D75', '#C4B56C'] },
  { value: 'tol_sunset', label: 'Tol sunset', colours: ['#364B9A', '#4A7BB7', '#6EA6CD', '#98CAE1', '#C2E4EF', '#FEDA8B', '#FDB366', '#F67E4B', '#DD3D2D', '#A50026'] },
  { value: 'grey', label: 'Grey', colours: ['#000000', '#666666', '#999999', '#BBBBBB', '#333333'] },
];

export function paletteColours(name: string | undefined): string[] {
  return (PALETTES.find((p) => p.value === name) ?? PALETTES[0]!).colours;
}

export const SIZE_PRESETS: { id: string; label: string; width: number; height: number }[] = [
  { id: 'single', label: 'Single column, 85 × 70 mm', width: 85, height: 70 },
  { id: 'double', label: 'Double column, 174 × 100 mm', width: 174, height: 100 },
  { id: 'slide', label: 'Slide, 120 × 90 mm', width: 120, height: 90 },
];

export const MARKERS = ['circle', 'square', 'triangle', 'triangle_down', 'diamond', 'plus', 'cross', 'none'];
export const LINE_STYLES = ['solid', 'dashed', 'dotted', 'dash_dot', 'none'] as const;
export const ERROR_BARS: { value: string; label: string }[] = [
  { value: 'none', label: 'None' },
  { value: 'sd', label: 'SD' },
  { value: 'sem', label: 'SEM' },
  { value: 'ci95', label: '95% CI' },
  { value: 'range', label: 'Range' },
  { value: 'iqr', label: 'IQR' },
];
export const LEGEND_POSITIONS: { value: string; label: string }[] = [
  { value: 'auto', label: 'Automatic' },
  { value: 'right', label: 'Right' },
  { value: 'top', label: 'Top' },
  { value: 'bottom', label: 'Bottom' },
  { value: 'inside_top_left', label: 'Inside, top left' },
  { value: 'inside_top_right', label: 'Inside, top right' },
  { value: 'inside_bottom_left', label: 'Inside, bottom left' },
  { value: 'inside_bottom_right', label: 'Inside, bottom right' },
  { value: 'hidden', label: 'Hidden' },
];
