// The graph editor: the engine's SVG is the hero, scaled to fit (or 50–400%, or actual size
// at 96 dpi), with a transparent overlay of the engine's hit-test targets. Click selects (the
// inspector follows), double-click rewrites text in place, drag moves the legend and text.
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { showContextMenu } from '../app/ContextMenu';
import type { WorkAreaProps } from '../app/workArea';
import type { Annotation, GraphSpecT } from '../engine/objects';
import type { Id, JsonValue, Target } from '../engine/types';
import { useTableMeta } from '../store/table';
import { addBracket, addReferenceLine, addText, customSize, exportGraphDialog, sourceTableId, tableMeta } from './actions';
import { ERROR_BARS, familiesFor, familyName, PALETTES, SIZE_PRESETS, THEMES } from './families';
import { getPath } from './patch';
import { loadGraph, listenForGraphChanges, selectTarget, setGraphValue, setZoom, useGraph, type Zoom } from './store';
import { axisMap, moveBy, tidy } from './svgAxes';
import { byAreaDescending, describeTarget, isDraggable, textPathOf } from './targets';
import { boxToPx, clampZoom, fitZoom, mmToPt, pxPerPt, pxToPt, ZOOM_MAX, ZOOM_MIN } from './units';

export function GraphEditor({ item }: WorkAreaProps) {
  const id = item.id;
  const st = useGraph(id);
  useEffect(() => {
    listenForGraphChanges();
    void loadGraph(id);
  }, [id]);
  return (
    <div className="graph-editor" data-testid="graph-editor">
      <GraphToolbar id={id} spec={st.spec} />
      {st.error && !st.rendered ? (
        <div className="graph-error text-error" data-testid="graph-error">
          {st.error}
        </div>
      ) : (
        <Canvas id={id} />
      )}
    </div>
  );
}

// ---- toolbar -----------------------------------------------------------------------------------

function GraphToolbar({ id, spec }: { id: Id; spec: GraphSpecT | null }) {
  const meta = useTableMeta(spec ? sourceTableId(spec.source) : null);
  useEffect(() => {
    const t = spec ? sourceTableId(spec.source) : null;
    if (t && !meta) void tableMeta(t);
  }, [spec, meta]);
  if (!spec) return <div className="graph-toolbar" />;
  const families = familiesFor(meta?.kind);
  const fam = families.includes(spec.family) ? families : [spec.family, ...families];
  const s = spec.style.size_mm;
  const preset = SIZE_PRESETS.find((p) => p.width === s.width && p.height === s.height);
  const addMenu = (e: React.MouseEvent) => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    showContextMenu(r.left, r.bottom + 4, [
      { label: 'Significance bracket…', testId: 'add-bracket', run: () => addBracket(id) },
      { label: 'Reference line…', testId: 'add-refline', run: () => addReferenceLine(id) },
      { label: 'Text…', testId: 'add-text', run: () => addText(id) },
    ], 'Add');
  };
  return (
    <div className="graph-toolbar" role="toolbar" aria-label="Graph">
      <select className="input input-compact tb-family" aria-label="Graph family" title="Graph family" value={spec.family} data-testid="tb-family" onChange={(e) => void setGraphValue(id, '/family', e.target.value)}>
        {fam.map((f) => (
          <option key={f} value={f}>
            {familyName(f)}
          </option>
        ))}
      </select>
      <select
        className="input input-compact"
        aria-label="Size"
        title="Size"
        data-testid="tb-size"
        value={preset?.id ?? 'custom'}
        onChange={(e) => {
          const p = SIZE_PRESETS.find((x) => x.id === e.target.value);
          if (p) void setGraphValue(id, '/style/size_mm', { width: p.width, height: p.height });
          else void customSize(id);
        }}
      >
        {SIZE_PRESETS.map((p) => (
          <option key={p.id} value={p.id}>
            {p.width} × {p.height} mm
          </option>
        ))}
        <option value="custom">{preset ? 'Custom…' : `${s.width} × ${s.height} mm`}</option>
      </select>
      <span className="tb-sep" />
      <select className="input input-compact" aria-label="Theme" title="Theme" value={spec.style.theme} data-testid="tb-theme" onChange={(e) => void setGraphValue(id, '/style/theme', e.target.value)}>
        {THEMES.map((t) => (
          <option key={t.value} value={t.value}>
            {t.label}
          </option>
        ))}
      </select>
      <select
        className="input input-compact"
        aria-label="Palette"
        title="Palette"
        value={spec.style.palette ?? ''}
        data-testid="tb-palette"
        onChange={(e) => void setGraphValue(id, '/style/palette', e.target.value || null)}
      >
        <option value="">Okabe–Ito (default)</option>
        {PALETTES.filter((p) => p.value !== 'okabe_ito').map((p) => (
          <option key={p.value} value={p.value}>
            {p.label}
          </option>
        ))}
      </select>
      <span className="tb-sep" />
      <label className="tb-labelled">
        <span>Error bars</span>
        <select className="input input-compact" aria-label="Error bars" value={spec.mapping.error_bars} data-testid="tb-errorbars" onChange={(e) => void setGraphValue(id, '/mapping/error_bars', e.target.value)}>
          {ERROR_BARS.map((b) => (
            <option key={b.value} value={b.value}>
              {b.label}
            </option>
          ))}
        </select>
      </label>
      <label className="tb-check">
        <input type="checkbox" checked={spec.mapping.points} data-testid="tb-points" onChange={(e) => void setGraphValue(id, '/mapping/points', e.target.checked)} />
        Points
      </label>
      <span className="tb-gap" />
      <button type="button" className="btn btn-small" onClick={addMenu} data-testid="tb-add" aria-haspopup="menu">
        Add
        <svg width="8" height="8" viewBox="0 0 8 8" aria-hidden="true">
          <path d="M1.5 3l2.5 2.5L6.5 3" fill="none" stroke="currentColor" strokeWidth="1.2" />
        </svg>
      </button>
      <button type="button" className="btn btn-small" onClick={() => void exportGraphDialog(id)} data-testid="tb-export">
        Export…
      </button>
    </div>
  );
}

// ---- canvas ------------------------------------------------------------------------------------

function useSize(ref: React.RefObject<HTMLElement | null>): { w: number; h: number } {
  const [size, setSize] = useState({ w: 800, h: 600 });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const set = () => setSize({ w: el.clientWidth, h: el.clientHeight });
    set();
    const ro = new ResizeObserver(set);
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return size;
}

interface Drag {
  target: Target;
  x0: number;
  y0: number;
  dx: number;
  dy: number;
  moved: boolean;
}

function Canvas({ id }: { id: Id }) {
  const st = useGraph(id);
  const viewRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<HTMLDivElement>(null);
  const view = useSize(viewRef);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [editing, setEditing] = useState<{ target: Target; path: string; value: string } | null>(null);
  const r = st.rendered;
  const spec = st.spec;
  const annotations = (spec?.annotations ?? []) as Annotation[];
  const zoom = r ? (st.zoom === 'fit' ? fitZoom(r.width_mm, r.height_mm, view.w, view.h) : st.zoom) : 1;
  const k = pxPerPt(zoom);
  const wPx = r ? mmToPt(r.width_mm) * k : 0;
  const hPx = r ? mmToPt(r.height_mm) * k : 0;
  const ordered = useMemo(() => (r ? byAreaDescending(r.targets) : []), [r]);
  const selected = r?.targets.find((t) => t.id === st.selected);

  // Live drag: move the SVG group itself so the figure follows the pointer.
  useEffect(() => {
    if (!drag) return;
    const g = svgRef.current?.querySelector(`#${CSS.escape(drag.target.id)}`);
    g?.setAttribute('transform', `translate(${pxToPt(drag.dx, zoom)} ${pxToPt(drag.dy, zoom)})`);
  }, [drag, zoom]);

  const dragRef = useRef<Drag | null>(null);
  const dragging = drag !== null;
  useEffect(() => {
    if (!dragging) return;
    const move = (e: MouseEvent) => {
      const d = dragRef.current;
      if (!d) return;
      const next = { ...d, dx: e.clientX - d.x0, dy: e.clientY - d.y0, moved: d.moved || Math.hypot(e.clientX - d.x0, e.clientY - d.y0) > 3 };
      dragRef.current = next;
      setDrag(next);
    };
    const up = () => {
      const d = dragRef.current;
      dragRef.current = null;
      setDrag(null);
      if (d) void finishDrag(d);
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
    return () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
    };
  }, [dragging]); // eslint-disable-line react-hooks/exhaustive-deps

  const finishDrag = async (d: Drag) => {
    const ok = await commitDrag(d);
    if (!ok) svgRef.current?.querySelector(`#${CSS.escape(d.target.id)}`)?.removeAttribute('transform');
  };

  const commitDrag = async (d: Drag): Promise<boolean> => {
    if (!d.moved || !r || !spec) return false;
    const dxPt = pxToPt(d.dx, zoom);
    const dyPt = pxToPt(d.dy, zoom);
    if (d.target.id === 'legend') {
      const plot = r.targets.find((t) => t.id === 'plot-area')?.bbox;
      if (!plot || plot.width <= 0 || plot.height <= 0) return false;
      // Keep the whole legend inside the plot area (fractions of it, top-left corner).
      const clamp = (v: number, hi: number) => Math.min(Math.max(v, 0), Math.max(0, hi));
      const at = {
        x: tidy(clamp((d.target.bbox.x + dxPt - plot.x) / plot.width, 1 - d.target.bbox.width / plot.width), 4),
        y: tidy(clamp((d.target.bbox.y + dyPt - plot.y) / plot.height, 1 - d.target.bbox.height / plot.height), 4),
      };
      return setGraphValue(id, '/legend/at', at);
    }
    const m = d.target.target.match(/^\/annotations\/(\d+)$/);
    const a = m ? annotations[Number(m[1])] : undefined;
    if (!m || !a || a.kind !== 'text_label') return false;
    const root = svgRef.current?.querySelector('svg');
    if (!root) return false;
    const mx = axisMap(root, 'x', spec.axes.x.scale);
    const my = axisMap(root, 'y', spec.axes.y.scale);
    if (!mx || !my) return false;
    const next = annotations.map((x, i) =>
      i === Number(m[1]) ? { ...a, x: tidy(moveBy(mx, a.x, dxPt), 4), y: tidy(moveBy(my, a.y, dyPt), 4) } : x,
    );
    return setGraphValue(id, '/annotations', next as unknown as JsonValue);
  };

  const startEdit = (t: Target) => {
    const path = textPathOf(t, annotations);
    if (!path) return;
    const fromSpec = getPath(spec as unknown as JsonValue, path);
    const shown = svgRef.current?.querySelector(`#${CSS.escape(t.id)} text`)?.textContent ?? '';
    setEditing({ target: t, path, value: typeof fromSpec === 'string' ? fromSpec : shown });
  };

  const setZ = (z: Zoom) => setZoom(id, z === 'fit' ? 'fit' : clampZoom(z));
  const steps = [0.5, 0.67, 0.75, 1, 1.25, 1.5, 2, 3, 4];
  const zoomIn = () => setZ(steps.find((s) => s > zoom + 1e-3) ?? ZOOM_MAX);
  const zoomOut = () => setZ([...steps].reverse().find((s) => s < zoom - 1e-3) ?? ZOOM_MIN);

  return (
    <div className="graph-stage">
      <div
        className="graph-view"
        ref={viewRef}
        tabIndex={0}
        data-testid="graph-view"
        onMouseDown={(e) => {
          if (e.target === e.currentTarget) selectTarget(id, null);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Escape') selectTarget(id, null);
          else if ((e.key === 'Delete' || e.key === 'Backspace') && selected?.target.startsWith('/annotations/')) {
            const i = Number(selected.target.split('/')[2]);
            const next = annotations.filter((_, j) => j !== i);
            void setGraphValue(id, '/annotations', next.length ? (next as unknown as JsonValue) : null);
            selectTarget(id, null);
          } else if ((e.ctrlKey || e.metaKey) && (e.key === '=' || e.key === '+')) {
            e.preventDefault();
            zoomIn();
          } else if ((e.ctrlKey || e.metaKey) && e.key === '-') {
            e.preventDefault();
            zoomOut();
          } else if ((e.ctrlKey || e.metaKey) && e.key === '0') {
            e.preventDefault();
            setZ('fit');
          }
        }}
      >
        {r && (
          <div className="graph-paper" style={{ width: wPx, height: hPx }} data-testid="graph-paper">
            <div className="graph-svg" ref={svgRef} data-testid="graph-svg" data-version={st.version} dangerouslySetInnerHTML={{ __html: r.svg }} />
            <div className="graph-overlay" onMouseDown={(e) => e.target === e.currentTarget && selectTarget(id, null)}>
              {ordered.map((t) => {
                const b = boxToPx(t.bbox, zoom);
                const isSel = t.id === st.selected;
                const draggable = isDraggable(t, annotations);
                const isDragged = drag?.target.id === t.id && drag.moved;
                return (
                  <div
                    key={t.id}
                    className={'tg' + (isSel ? ' is-selected' : '') + (draggable ? ' is-draggable' : '') + (isDragged ? ' is-dragging' : '')}
                    data-testid={`target-${t.id}`}
                    data-target={t.target}
                    title={describeTarget(t)}
                    style={{
                      left: b.x - 2,
                      top: b.y - 2,
                      width: Math.max(6, b.width + 4),
                      height: Math.max(6, b.height + 4),
                      transform: isDragged ? `translate(${drag.dx}px, ${drag.dy}px)` : undefined,
                    }}
                    onMouseDown={(e) => {
                      if (e.button !== 0) return;
                      e.stopPropagation();
                      selectTarget(id, t.id);
                      viewRef.current?.focus({ preventScroll: true });
                      if (draggable) {
                        e.preventDefault();
                        const d0 = { target: t, x0: e.clientX, y0: e.clientY, dx: 0, dy: 0, moved: false };
                        dragRef.current = d0;
                        setDrag(d0);
                      }
                    }}
                    onDoubleClick={(e) => {
                      e.stopPropagation();
                      startEdit(t);
                    }}
                  />
                );
              })}
            </div>
            {editing && (
              <InlineEditor
                key={editing.target.id}
                box={boxToPx(editing.target.bbox, zoom)}
                zoom={zoom}
                value={editing.value}
                vertical={editing.target.id === 'axis-y-title'}
                onDone={(v) => {
                  setEditing(null);
                  if (v !== null && v !== editing.value) void setGraphValue(id, editing.path, v === '' ? null : v);
                  viewRef.current?.focus({ preventScroll: true });
                }}
              />
            )}
          </div>
        )}
        {!r && <div className="loading-line">Drawing…</div>}
      </div>
      <div className="graph-foot">
        <span className="gf-sel" data-testid="graph-selected-label">
          {selected ? describeTarget(selected) : 'Click an element to edit it. Double-click text to rewrite it; drag the legend or text to move it.'}
        </span>
        <span className="tb-gap" />
        {r && (
          <span className="gf-size mono">
            {r.width_mm} × {r.height_mm} mm
          </span>
        )}
        <div className="zoom" role="group" aria-label="Zoom">
          <button type="button" className="icon-btn" aria-label="Zoom out" onClick={zoomOut} data-testid="zoom-out">
            −
          </button>
          <span className="zoom-val" data-testid="zoom-value">
            {Math.round(zoom * 100)}%
          </span>
          <button type="button" className="icon-btn" aria-label="Zoom in" onClick={zoomIn} data-testid="zoom-in">
            +
          </button>
          <button type="button" className={'btn btn-small btn-ghost' + (st.zoom === 'fit' ? ' is-on' : '')} onClick={() => setZ('fit')} data-testid="zoom-fit">
            Fit
          </button>
          <button type="button" className={'btn btn-small btn-ghost' + (st.zoom === 1 ? ' is-on' : '')} onClick={() => setZ(1)} title="Actual size at 96 dpi" data-testid="zoom-actual">
            1:1
          </button>
        </div>
      </div>
    </div>
  );
}

function InlineEditor({
  box,
  zoom,
  value,
  vertical,
  onDone,
}: {
  box: { x: number; y: number; width: number; height: number };
  zoom: number;
  value: string;
  vertical: boolean;
  onDone: (v: string | null) => void;
}) {
  const [v, setV] = useState(value);
  const ref = useRef<HTMLInputElement>(null);
  const done = useRef(false);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  const finish = (x: string | null) => {
    if (done.current) return;
    done.current = true;
    onDone(x);
  };
  const width = Math.max(160, (vertical ? box.height : box.width) + 40);
  const fontPx = Math.max(12, Math.min(18, 9 * pxPerPt(zoom)));
  const left = vertical ? box.x + box.width + 6 : box.x + box.width / 2 - width / 2;
  const top = vertical ? box.y + box.height / 2 - 14 : box.y + box.height / 2 - 14;
  return (
    <input
      ref={ref}
      className="input inline-edit"
      data-testid="inline-edit"
      aria-label="Edit text"
      style={{ left: Math.max(0, left), top, width, fontSize: fontPx }}
      value={v}
      spellCheck={false}
      onChange={(e) => setV(e.target.value)}
      onBlur={() => finish(v)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          finish(v);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          finish(null);
        }
        e.stopPropagation();
      }}
    />
  );
}
