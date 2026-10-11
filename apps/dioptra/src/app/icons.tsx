// Icons. Generic glyphs come from Lucide; the project glyphs (table kinds, analysis, graph,
// layout) are Dioptra's own, drawn on Lucide's 24-unit grid with the same 1.5 stroke, round
// caps and joins, so the rail and the rest of the interface read as one set. Shown at 16 px.
import {
  BetweenHorizontalStart,
  ChartSpline,
  ChevronDown,
  ChevronRight,
  CircleHelp,
  Eye,
  FileText,
  Pencil,
  Plus,
  Search,
  Sigma,
  SquareFunction,
  TableProperties,
  X,
} from 'lucide-react';
import type { MenuName } from './registry';
import type { ItemKind, ProjectItem, TableKind } from '../engine/types';

type P = { size?: number; title?: string; className?: string };

const Glyph = ({ size = 16, title, className, children }: P & { children: React.ReactNode }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={1.5}
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden={title ? undefined : true}
    role={title ? 'img' : undefined}
    className={'icon glyph' + (className ? ` ${className}` : '')}
  >
    {title && <title>{title}</title>}
    {children}
  </svg>
);

const dot = (cx: number, cy: number, r = 1.1) => <circle cx={cx} cy={cy} r={r} fill="currentColor" stroke="none" />;

// ---- project objects ------------------------------------------------------------------------

/** A sheet: a frame, a header rule and one column rule. */
export const TableIcon = (p: P) => (
  <Glyph {...p}>
    <rect x="3.5" y="4.5" width="17" height="15" rx="1.5" />
    <path d="M3.5 9h17M9.5 9v10.5" />
  </Glyph>
);

/** An analysis: a results sheet, a heading rule and rows of values. */
export const AnalysisIcon = (p: P) => (
  <Glyph {...p}>
    <path d="M14.5 3.5H6.5a1.5 1.5 0 0 0-1.5 1.5v14a1.5 1.5 0 0 0 1.5 1.5h11a1.5 1.5 0 0 0 1.5-1.5V8z" />
    <path d="M14.5 3.5V8H19" />
    <path d="M8.5 12h7M8.5 15h4.5M8.5 18h6" />
  </Glyph>
);

/** A graph: two axes and a rising curve with two measured points. */
export const GraphIcon = (p: P) => (
  <Glyph {...p}>
    <path d="M4 3.5v16.5h16.5" />
    <path d="M7 16.5c3.5 0 4-8 7.5-8.5 2-.3 3.5-.8 5-2.5" />
    {dot(10.5, 13.2)}
    {dot(15.5, 8.6)}
  </Glyph>
);

/** A layout: a page of lettered panels. */
export const LayoutIcon = (p: P) => (
  <Glyph {...p}>
    <rect x="3.5" y="3.5" width="17" height="17" rx="1.5" />
    <path d="M12 3.5v17M3.5 12h8.5" />
    <path d="M6 6.5v2.5M15 6.5v2.5" strokeWidth={1.25} />
  </Glyph>
);

// ---- table kinds ------------------------------------------------------------------------------

const KindColumn = (p: P) => (
  <Glyph {...p}>
    <path d="M3.5 20.5h17" />
    <path d="M6.5 6v11M12 9v8M17.5 5v12" />
    {dot(6.5, 8.5)}
    {dot(6.5, 13)}
    {dot(12, 12)}
    {dot(17.5, 7.5)}
    {dot(17.5, 12.5)}
  </Glyph>
);

const KindXY = (p: P) => (
  <Glyph {...p}>
    <path d="M4 3.5v16.5h16.5" />
    {dot(8, 15.5, 1.3)}
    {dot(11, 12.5, 1.3)}
    {dot(14, 11, 1.3)}
    {dot(17.5, 7, 1.3)}
  </Glyph>
);

const KindGrouped = (p: P) => (
  <Glyph {...p}>
    <path d="M3.5 20.5h17" />
    <path d="M5.5 20.5v-9h3v9M8.5 20.5v-6h3v6M13 20.5v-12h3v12M16 20.5v-8h3v8" />
  </Glyph>
);

const KindContingency = (p: P) => (
  <Glyph {...p}>
    <rect x="3.5" y="3.5" width="17" height="17" rx="1.5" />
    <path d="M12 3.5v17M3.5 12h17" />
    {dot(7.75, 7.75, 1.3)}
    {dot(16.25, 16.25, 1.3)}
  </Glyph>
);

const KindSurvival = (p: P) => (
  <Glyph {...p}>
    <path d="M4 3.5v16.5h16.5" />
    <path d="M4 6h4v3.5h3.5V13H15v3h5" />
    <path d="M13 11.5v3" strokeWidth={1.25} />
  </Glyph>
);

const KindParts = (p: P) => (
  <Glyph {...p}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 3.5V12l6.5 5.5M12 12l-7.8 3.4" />
  </Glyph>
);

const KindVariables = (p: P) => (
  <Glyph {...p}>
    <rect x="3.5" y="4.5" width="17" height="15" rx="1.5" />
    <path d="M3.5 9h17M3.5 14.25h17M9.25 4.5v15M15 4.5v15" />
  </Glyph>
);

const KindNested = (p: P) => (
  <Glyph {...p}>
    <path d="M3.5 4.5h17" />
    <path d="M5 4.5v3M12 4.5v3M19 4.5v3" />
    <path d="M3.5 11h7M13.5 11h7" />
    <path d="M5 11v8.5M9 11v8.5M15 11v8.5M19 11v8.5" />
  </Glyph>
);

export const KIND_ICONS: Record<TableKind, (p: P) => React.ReactElement> = {
  column: KindColumn,
  xy: KindXY,
  grouped: KindGrouped,
  contingency: KindContingency,
  survival: KindSurvival,
  parts: KindParts,
  variables: KindVariables,
  nested: KindNested,
};

export const ITEM_ICONS: Record<ItemKind, (p: P) => React.ReactElement> = {
  table: TableIcon,
  analysis: AnalysisIcon,
  graph: GraphIcon,
  layout: LayoutIcon,
};

/** The glyph for one project object: tables by their kind. */
export function ItemIcon({ item, ...p }: P & { item: Pick<ProjectItem, 'kind' | 'subtype'> }) {
  const K = item.kind === 'table' && item.subtype ? KIND_ICONS[item.subtype as TableKind] : undefined;
  const C = K ?? ITEM_ICONS[item.kind];
  return <C {...p} />;
}

// ---- generic, from Lucide ---------------------------------------------------------------------

const lucide = { strokeWidth: 1.5, 'aria-hidden': true } as const;

export const PlusIcon = ({ size = 16 }: P) => <Plus size={size} {...lucide} />;
export const CloseIcon = ({ size = 16 }: P) => <X size={size} {...lucide} />;
export const SearchIcon = ({ size = 16 }: P) => <Search size={size} {...lucide} />;
export const FormulaIcon = ({ size = 16 }: P) => <SquareFunction size={size} {...lucide} />;
export const ChevronIcon = ({ open, size = 16 }: P & { open: boolean }) =>
  open ? <ChevronDown size={size} {...lucide} /> : <ChevronRight size={size} {...lucide} />;

/** One glyph per menu, for the palette and the Menu button's tree. */
export const MENU_ICONS: Record<MenuName, React.ReactElement> = {
  File: <FileText size={16} {...lucide} />,
  Edit: <Pencil size={16} {...lucide} />,
  Insert: <BetweenHorizontalStart size={16} {...lucide} />,
  Data: <TableProperties size={16} {...lucide} />,
  View: <Eye size={16} {...lucide} />,
  Analyze: <Sigma size={16} {...lucide} />,
  Graph: <ChartSpline size={16} {...lucide} />,
  Help: <CircleHelp size={16} {...lucide} />,
};
