// The export dialog for graphs and layouts: format, DPI for PNG, size in mm or inches, the
// transparent background for PNG, the file name, and exactly what will be written.
import { useState } from 'react';
import { DialogHeader } from '../app/dialogs';
import { Segmented, Switch } from '../analysis/OptionsPanel';
import type { SizeMm } from '../engine/types';
import { inToMm, mmToIn, pngPixels, trim } from './units';

export type ExportFormat = 'svg' | 'pdf' | 'png';

export interface ExportChoice {
  format: ExportFormat;
  dpi: number;
  /** Null: the graph's own size. */
  size: SizeMm | null;
  transparent: boolean;
  filename: string;
}

const DPIS = [150, 300, 600];

export function fileStem(title: string): string {
  return title.replace(/[\\/:*?"<>|]+/g, ' ').replace(/\s+/g, ' ').trim() || 'graph';
}

export function ExportGraphDialog({
  title,
  size,
  transparent,
  allowSize = true,
  desktop,
  close,
}: {
  title: string;
  size: SizeMm;
  transparent: boolean;
  /** Layouts take their size from their panels. */
  allowSize?: boolean;
  desktop: boolean;
  close: (c: ExportChoice | null) => void;
}) {
  const [format, setFormat] = useState<ExportFormat>('pdf');
  const [dpiChoice, setDpiChoice] = useState<number | 'custom'>(300);
  const [customDpi, setCustomDpi] = useState('1200');
  const [unit, setUnit] = useState<'mm' | 'in'>('mm');
  const [w, setW] = useState(trim(size.width, 2));
  const [h, setH] = useState(trim(size.height, 2));
  const [trans, setTrans] = useState(transparent);
  const [name, setName] = useState(fileStem(title));
  const dpi = dpiChoice === 'custom' ? Number(customDpi) : dpiChoice;
  const toMm = (s: string) => {
    const v = Number(s.replace(',', '.'));
    return unit === 'mm' ? v : inToMm(v);
  };
  const wMm = toMm(w);
  const hMm = toMm(h);
  const sizeOk = !allowSize || (wMm >= 10 && wMm <= 1000 && hMm >= 10 && hMm <= 1000);
  const dpiOk = format !== 'png' || (Number.isFinite(dpi) && dpi >= 36 && dpi <= 2400);
  const nameOk = name.trim().length > 0;
  const filename = `${name.trim()}.${format}`;
  const effW = allowSize ? wMm : size.width;
  const effH = allowSize ? hMm : size.height;
  const detail =
    format === 'png'
      ? `${pngPixels(effW, dpi)} × ${pngPixels(effH, dpi)} pixels at ${dpi} dpi, ${trim(effW, 1)} × ${trim(effH, 1)} mm${trans ? ', transparent background' : ''}.`
      : format === 'pdf'
        ? `Vector PDF, ${trim(effW, 1)} × ${trim(effH, 1)} mm, fonts embedded.`
        : `SVG, ${trim(effW, 1)} × ${trim(effH, 1)} mm, the same file the editor shows.`;
  const switchUnit = (u: 'mm' | 'in') => {
    if (u === unit) return;
    const conv = (s: string) => {
      const v = Number(s.replace(',', '.'));
      if (!Number.isFinite(v)) return s;
      return u === 'in' ? trim(mmToIn(v), 3) : trim(inToMm(v), 2);
    };
    setW(conv(w));
    setH(conv(h));
    setUnit(u);
  };
  const ok = sizeOk && dpiOk && nameOk;
  return (
    <form
      className="modal-body export-dialog"
      data-testid="graph-export-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        if (!ok) return;
        const changed = allowSize && (Math.abs(wMm - size.width) > 1e-6 || Math.abs(hMm - size.height) > 1e-6);
        close({ format, dpi, size: changed ? { width: wMm, height: hMm } : null, transparent: trans, filename });
      }}
    >
      <DialogHeader title="Export" sub={`"${title}"`} />
      <div className="ex-grid">
        <span className="ex-label">Format</span>
        <Segmented
          testId="export-format"
          label="Format"
          choices={[
            { value: 'pdf', label: 'PDF', hint: 'Vector, fonts embedded; for journals.' },
            { value: 'svg', label: 'SVG', hint: 'Vector; for editing in other tools.' },
            { value: 'png', label: 'PNG', hint: 'Pixels at a chosen resolution.' },
          ]}
          value={format}
          onChange={(v) => setFormat(v as ExportFormat)}
        />
        {format === 'png' && (
          <>
            <span className="ex-label">Resolution</span>
            <div className="ex-row">
              <Segmented
                testId="export-dpi"
                label="Resolution"
                choices={[...DPIS.map((d) => ({ value: d, label: `${d} dpi` })), { value: 'custom', label: 'Custom' }]}
                value={dpiChoice}
                onChange={(v) => setDpiChoice(v as number | 'custom')}
              />
              {dpiChoice === 'custom' && (
                <input
                  className="input input-compact num ex-num"
                  aria-label="Custom dpi"
                  value={customDpi}
                  aria-invalid={!dpiOk}
                  onChange={(e) => setCustomDpi(e.target.value)}
                />
              )}
            </div>
          </>
        )}
        {allowSize && (
          <>
            <span className="ex-label">Size</span>
            <div className="ex-row">
              <input
                className="input input-compact num ex-num"
                aria-label="Width"
                data-testid="export-width"
                value={w}
                aria-invalid={!sizeOk}
                onChange={(e) => setW(e.target.value)}
              />
              <span className="ex-times">×</span>
              <input
                className="input input-compact num ex-num"
                aria-label="Height"
                data-testid="export-height"
                value={h}
                aria-invalid={!sizeOk}
                onChange={(e) => setH(e.target.value)}
              />
              <Segmented
                label="Unit"
                choices={[
                  { value: 'mm', label: 'mm' },
                  { value: 'in', label: 'in' },
                ]}
                value={unit}
                onChange={(u) => switchUnit(u as 'mm' | 'in')}
              />
            </div>
          </>
        )}
        {format === 'png' && allowSize && (
          <>
            <span className="ex-label">Background</span>
            <div className="ex-row">
              <Switch checked={trans} onChange={setTrans} label="Transparent background" testId="export-transparent" />
              <span className="muted small">Transparent (saved with the graph)</span>
            </div>
          </>
        )}
        <span className="ex-label">File name</span>
        <div className="ex-row">
          <input
            className="input input-compact ex-name"
            aria-label="File name"
            data-testid="export-name"
            value={name}
            spellCheck={false}
            onChange={(e) => setName(e.target.value)}
          />
          <span className="muted">.{format}</span>
        </div>
      </div>
      <p className="ex-detail" data-testid="export-detail">
        <span className="mono">{filename}</span>: {detail}
      </p>
      <footer className="modal-foot">
        <button type="button" className="btn" onClick={() => close(null)}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary" disabled={!ok} data-testid="export-go">
          {desktop ? 'Choose location…' : 'Download'}
        </button>
      </footer>
    </form>
  );
}
