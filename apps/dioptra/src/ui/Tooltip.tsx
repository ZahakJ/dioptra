// Tooltips: any element with `data-tip="..."` (and optionally `data-tip-kbd="Mod+K"`) shows a
// designed tooltip after 400 ms of hover or on keyboard focus. One host serves the whole app,
// so no native `title` attribute is needed anywhere. `<Tooltip>` is the component form.
import { cloneElement, useEffect, useLayoutEffect, useRef, useState, type ReactElement } from 'react';
import { createPortal } from 'react-dom';
import { formatShortcut } from '../app/keys';
import { placeBox } from './Popover';

const DELAY = 400;

export function Tooltip({
  label,
  shortcut,
  side,
  children,
}: {
  label: string;
  shortcut?: string;
  side?: 'top' | 'bottom';
  children: ReactElement<Record<string, unknown>>;
}) {
  return cloneElement(children, { 'data-tip': label, 'data-tip-kbd': shortcut, 'data-tip-side': side });
}

/** Tooltip props for a plain element: `<button {...tip('Zoom in', 'Mod+=')}>`. */
export function tip(label: string | undefined, shortcut?: string): Record<string, string | undefined> {
  return label ? { 'data-tip': label, 'data-tip-kbd': shortcut } : {};
}

interface Shown {
  el: HTMLElement;
  text: string;
  kbd?: string;
  side: 'top' | 'bottom';
}

export function TooltipHost() {
  const [shown, setShown] = useState<Shown | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const current = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const read = (el: HTMLElement): Shown | null => {
      const text = el.dataset.tip;
      if (!text) return null;
      return { el, text, kbd: el.dataset.tipKbd || undefined, side: el.dataset.tipSide === 'top' ? 'top' : 'bottom' };
    };
    const schedule = (el: HTMLElement, delay: number) => {
      window.clearTimeout(timer.current);
      current.current = el;
      timer.current = window.setTimeout(() => {
        if (current.current === el && el.isConnected) setShown(read(el));
      }, delay);
    };
    const hide = () => {
      window.clearTimeout(timer.current);
      current.current = null;
      setShown(null);
    };
    const over = (e: PointerEvent) => {
      const el = (e.target as Element | null)?.closest?.<HTMLElement>('[data-tip]');
      if (!el) {
        if (current.current) hide();
        return;
      }
      if (el !== current.current) {
        setShown(null);
        schedule(el, DELAY);
      }
    };
    const focusIn = (e: FocusEvent) => {
      const el = (e.target as Element | null)?.closest?.<HTMLElement>('[data-tip]');
      if (el && el.matches(':focus-visible')) schedule(el, DELAY);
    };
    const key = (e: KeyboardEvent) => e.key === 'Escape' && hide();
    document.addEventListener('pointerover', over);
    document.addEventListener('pointerdown', hide, true);
    document.addEventListener('focusin', focusIn);
    document.addEventListener('focusout', hide);
    document.addEventListener('keydown', key, true);
    document.addEventListener('scroll', hide, true);
    window.addEventListener('blur', hide);
    return () => {
      window.clearTimeout(timer.current);
      document.removeEventListener('pointerover', over);
      document.removeEventListener('pointerdown', hide, true);
      document.removeEventListener('focusin', focusIn);
      document.removeEventListener('focusout', hide);
      document.removeEventListener('keydown', key, true);
      document.removeEventListener('scroll', hide, true);
      window.removeEventListener('blur', hide);
    };
  }, []);

  if (!shown) return null;
  return <TooltipBubble {...shown} />;
}

function TooltipBubble({ el, text, kbd, side }: Shown) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  useLayoutEffect(() => {
    if (!ref.current) return;
    const b = ref.current.getBoundingClientRect();
    setPos(placeBox(el.getBoundingClientRect(), b.width, b.height, side, window.innerWidth, window.innerHeight));
  }, [el, text, side]);
  return createPortal(
    <div
      ref={ref}
      className={'tooltip' + (pos ? ' is-placed' : '')}
      role="tooltip"
      data-testid="tooltip"
      style={{ left: pos?.x ?? -9999, top: pos?.y ?? -9999 }}
    >
      <span>{text}</span>
      {kbd && <kbd className="kbd">{formatShortcut(kbd)}</kbd>}
    </div>,
    document.body,
  );
}
