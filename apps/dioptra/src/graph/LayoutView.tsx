// A layout: several graphs on one page with panel letters, rendered by the engine.
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { WorkAreaProps } from '../app/workArea';
import * as obj from '../engine/objects';
import type { Rendered } from '../engine/objects';
import type { Id } from '../engine/types';
import { withEngine } from '../store/engine';
import { useProject } from '../store/project';
import { onChanged } from '../store/sync';
import { exportLayoutDialog, layoutGraphs } from './actions';
import { boxToPx, clampZoom, fitZoom, mmToPt, pxPerPt } from './units';

export function LayoutView({ item }: WorkAreaProps) {
  const id = item.id;
  const [r, setR] = useState<Rendered | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState<number | 'fit'>('fit');
  const viewRef = useRef<HTMLDivElement>(null);
  const [view, setView] = useState({ w: 800, h: 600 });
  const seq = useRef(0);
  const load = useCallback(() => {
    const n = ++seq.current;
    withEngine((e) => obj.renderLayout(e, id))
      .then((x) => n === seq.current && (setR(x), setError(null)))
      .catch((err: unknown) => n === seq.current && setError(err instanceof Error ? err.message : String(err)));
  }, [id]);
  useEffect(() => load(), [load]);
  useEffect(
    () =>
      onChanged((c) => {
        const items = useProject.getState().view?.items ?? [];
        if (c.ids.some((x) => x === id || items.find((i) => i.id === x)?.kind !== 'table')) load();
      }),
    [id, load],
  );
  useLayoutEffect(() => {
    const el = viewRef.current;
    if (!el) return;
    const set = () => setView({ w: el.clientWidth, h: el.clientHeight });
    set();
    const ro = new ResizeObserver(set);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const z = r ? (zoom === 'fit' ? fitZoom(r.width_mm, r.height_mm, view.w, view.h) : zoom) : 1;
  const k = pxPerPt(z);
  const panels = r?.targets.filter((t) => /^panel-\d+$/.test(t.id)) ?? [];
  const graphs: Id[] = layoutGraphs[id] ?? [];
  return (
    <div className="graph-editor layout-view" data-testid="layout-view">
      <div className="graph-toolbar" role="toolbar" aria-label="Layout">
        <span className="tb-title">{item.title}</span>
        <span className="muted small">{panels.length ? `${panels.length} panels, lettered in reading order` : ''}</span>
        <span className="tb-gap" />
        <button
          type="button"
          className="btn btn-small"
          disabled={!r}
          onClick={() => r && void exportLayoutDialog(id, { width: r.width_mm, height: r.height_mm })}
          data-testid="layout-export"
        >
          Export…
        </button>
      </div>
      <div className="graph-stage">
        <div className="graph-view" ref={viewRef}>
          {error && <div className="graph-error text-error">{error}</div>}
          {r && (
            <div className="graph-paper" style={{ width: mmToPt(r.width_mm) * k, height: mmToPt(r.height_mm) * k }} data-testid="layout-paper">
              <div className="graph-svg" data-testid="layout-svg" dangerouslySetInnerHTML={{ __html: r.svg }} />
              <div className="graph-overlay">
                {panels.map((t) => {
                  const b = boxToPx(t.bbox, z);
                  const i = Number(t.id.split('-')[1]);
                  const gid = graphs[i];
                  return (
                    <div
                      key={t.id}
                      className="tg tg-panel"
                      data-testid={`layout-${t.id}`}
                      title={gid ? 'Double-click to open this graph' : `Panel ${String.fromCharCode(65 + i)}`}
                      style={{ left: b.x, top: b.y, width: b.width, height: b.height }}
                      onDoubleClick={() => gid && useProject.getState().open(gid)}
                    />
                  );
                })}
              </div>
            </div>
          )}
        </div>
        <div className="graph-foot">
          <span className="gf-sel">Panels follow their graphs: edit a graph and the layout redraws.</span>
          <span className="tb-gap" />
          {r && (
            <span className="gf-size mono">
              {Math.round(r.width_mm * 10) / 10} × {Math.round(r.height_mm * 10) / 10} mm
            </span>
          )}
          <div className="zoom" role="group" aria-label="Zoom">
            <button type="button" className="icon-btn" aria-label="Zoom out" onClick={() => setZoom(clampZoom(z / 1.25))}>
              −
            </button>
            <span className="zoom-val">{Math.round(z * 100)}%</span>
            <button type="button" className="icon-btn" aria-label="Zoom in" onClick={() => setZoom(clampZoom(z * 1.25))}>
              +
            </button>
            <button type="button" className="btn btn-small btn-ghost" onClick={() => setZoom('fit')}>
              Fit
            </button>
            <button type="button" className="btn btn-small btn-ghost" onClick={() => setZoom(1)} title="Actual size at 96 dpi">
              1:1
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
