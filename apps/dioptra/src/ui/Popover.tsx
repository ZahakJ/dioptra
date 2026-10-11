// The floating-surface primitive: a portal positioned against an anchor element or a point,
// flipped and clamped to stay inside the window, closed by an outside press, Escape or the
// window losing focus. Menus, selects, colour pickers and tooltips are built on it.
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

export type Placement = 'bottom-start' | 'bottom-end' | 'top-start' | 'top' | 'bottom' | 'right-start';

export type Anchor = HTMLElement | { x: number; y: number } | null;

const GAP = 4;
const EDGE = 8;

function rectOf(a: Exclude<Anchor, null>): DOMRect {
  if (a instanceof HTMLElement) return a.getBoundingClientRect();
  return new DOMRect(a.x, a.y, 0, 0);
}

/** Where a floating box of size `w` x `h` goes against anchor rect `r`. Pure, for tests. */
export function placeBox(
  r: { left: number; top: number; right: number; bottom: number; width: number; height: number },
  w: number,
  h: number,
  placement: Placement,
  vw: number,
  vh: number,
): { x: number; y: number } {
  let x: number;
  let y: number;
  if (placement === 'right-start') {
    x = r.right - 2;
    y = r.top - GAP - 1;
    if (x + w > vw - EDGE) x = r.left - w + 2;
  } else {
    const above = placement.startsWith('top');
    x =
      placement.endsWith('-end') ? r.right - w : placement === 'top' || placement === 'bottom' ? r.left + r.width / 2 - w / 2 : r.left;
    y = above ? r.top - GAP - h : r.bottom + GAP;
    if (!above && y + h > vh - EDGE && r.top - GAP - h >= EDGE) y = r.top - GAP - h;
    if (above && y < EDGE && r.bottom + GAP + h <= vh - EDGE) y = r.bottom + GAP;
  }
  x = Math.max(EDGE, Math.min(x, vw - w - EDGE));
  y = Math.max(EDGE, Math.min(y, vh - h - EDGE));
  return { x: Math.round(x), y: Math.round(y) };
}

export function Popover({
  anchor,
  open,
  onClose,
  placement = 'bottom-start',
  matchWidth = false,
  className,
  children,
  role,
  label,
  testId,
  style,
  closeOnBlur = true,
}: {
  anchor: Anchor;
  open: boolean;
  onClose: () => void;
  placement?: Placement;
  /** At least as wide as the anchor (selects). */
  matchWidth?: boolean;
  className?: string;
  children: ReactNode;
  role?: string;
  label?: string;
  testId?: string;
  style?: CSSProperties;
  closeOnBlur?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ x: number; y: number; minW?: number } | null>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  useLayoutEffect(() => {
    if (!open || !anchor || !ref.current) return setPos(null);
    const measure = () => {
      if (!ref.current) return;
      const r = rectOf(anchor);
      const minW = matchWidth ? r.width : undefined;
      if (minW) ref.current.style.minWidth = `${minW}px`;
      const b = ref.current.getBoundingClientRect();
      setPos({ ...placeBox(r, b.width, b.height, placement, window.innerWidth, window.innerHeight), minW });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(ref.current);
    window.addEventListener('resize', measure);
    return () => {
      ro.disconnect();
      window.removeEventListener('resize', measure);
    };
  }, [open, anchor, placement, matchWidth]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (ref.current?.contains(t)) return;
      // A click inside a nested popover (a submenu) is not outside this one.
      if (t instanceof Element && t.closest('[data-popover]')) {
        const owner = t.closest('[data-popover]') as HTMLElement;
        if (owner !== ref.current && Number(owner.dataset.depth ?? 0) > Number(ref.current?.dataset.depth ?? 0)) return;
      }
      if (anchor instanceof HTMLElement && anchor.contains(t)) return;
      closeRef.current();
    };
    const onBlur = () => closeOnBlur && closeRef.current();
    window.addEventListener('mousedown', onDown, true);
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('mousedown', onDown, true);
      window.removeEventListener('blur', onBlur);
    };
  }, [open, anchor, closeOnBlur]);

  if (!open) return null;
  const depth = anchor instanceof HTMLElement ? Number(anchor.closest<HTMLElement>('[data-popover]')?.dataset.depth ?? -1) + 1 : 0;
  return createPortal(
    <div
      ref={ref}
      className={'popover' + (className ? ` ${className}` : '') + (pos ? ' is-placed' : '')}
      data-popover=""
      data-depth={depth}
      role={role}
      aria-label={label}
      data-testid={testId}
      style={{ ...style, left: pos?.x ?? -9999, top: pos?.y ?? -9999 }}
    >
      {children}
    </div>,
    document.body,
  );
}
