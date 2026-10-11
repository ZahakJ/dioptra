// Small line icons drawn inline (14 px, 1.25 px strokes, currentColor).
import type { ItemKind } from '../engine/types';

type P = { size?: number; title?: string };

const Svg = ({ size = 14, title, children }: P & { children: React.ReactNode }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 14 14"
    fill="none"
    stroke="currentColor"
    strokeWidth={1.25}
    strokeLinecap="square"
    aria-hidden={title ? undefined : true}
    role={title ? 'img' : undefined}
    className="icon"
  >
    {title && <title>{title}</title>}
    {children}
  </svg>
);

export const TableIcon = (p: P) => (
  <Svg {...p}>
    <rect x="1.5" y="2.5" width="11" height="9" />
    <path d="M1.5 5.5h11M5.5 2.5v9" />
  </Svg>
);

export const AnalysisIcon = (p: P) => (
  <Svg {...p}>
    <path d="M2 12.5V7.5M5.5 12.5V3.5M9 12.5V6M12.5 12.5V9" />
  </Svg>
);

export const GraphIcon = (p: P) => (
  <Svg {...p}>
    <path d="M1.5 1.5v11h11" />
    <path d="M3.5 9.5l3-3.5 2 2 3.5-4.5" />
  </Svg>
);

export const LayoutIcon = (p: P) => (
  <Svg {...p}>
    <rect x="1.5" y="1.5" width="11" height="11" />
    <path d="M7 1.5v11M7 7h5.5" />
  </Svg>
);

export const PlusIcon = (p: P) => (
  <Svg {...p}>
    <path d="M7 2.5v9M2.5 7h9" />
  </Svg>
);

export const CloseIcon = (p: P) => (
  <Svg {...p}>
    <path d="M3.5 3.5l7 7M10.5 3.5l-7 7" />
  </Svg>
);

export const ChevronIcon = ({ open, ...p }: P & { open: boolean }) => (
  <Svg {...p}>
    <path d={open ? 'M3.5 5.5L7 9l3.5-3.5' : 'M5.5 3.5L9 7l-3.5 3.5'} />
  </Svg>
);

export const SearchIcon = (p: P) => (
  <Svg {...p}>
    <circle cx="6" cy="6" r="3.75" />
    <path d="M9 9l3.5 3.5" />
  </Svg>
);

export const FormulaIcon = (p: P) => (
  <Svg {...p}>
    <path d="M9.5 2.5c-1.5-.5-2.5.2-2.8 1.8L5.3 10c-.3 1.6-1.3 2.2-2.8 1.7M4 6h5" />
  </Svg>
);

export const ITEM_ICONS: Record<ItemKind, (p: P) => React.ReactElement> = {
  table: TableIcon,
  analysis: AnalysisIcon,
  graph: GraphIcon,
  layout: LayoutIcon,
};
