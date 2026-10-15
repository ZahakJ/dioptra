// The export dialog for graphs and layouts: format, DPI for PNG, size in mm or inches, the
// transparent background for PNG, the file name, and exactly what will be written.
import { useState } from 'react';
import { DialogHeader } from '../app/dialogs';
import { Button, NumberField, Segmented, TextInput, Toggle } from '../ui';
import type { SizeMm } from '../engine/types';
import { inToMm, mmToIn, pngPixels, trim } from './units';
import { useLab } from '../store/lab';
import { TIFF_WHAT } from './journal';
import './journal.css';

export type ExportFormat = 'svg' | 'pdf' | 'png' | 'tiff';

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
  const lab = useLab((st) => st.view.lab_enabled);
  const raster = format === 'png' || format === 'tiff';
  const toMm = (s: string) => {
    const v = Number(s.replace(',', '.'));
    return unit === 'mm' ? v : inToMm(v);
  };
  const wMm = toMm(w);
  const hMm = toMm(h);
  const sizeOk = !allowSize || (wMm >= 10 && wMm <= 1000 && hMm >= 10 && hMm <= 1000);
  const dpiOk = !raster || (Number.isFinite(dpi) && dpi >= 36 && dpi <= 2400);
  const nameOk = name.trim().length > 0;
  const filename = `${name.trim()}.${format}`;
  const effW = allowSize ? wMm : size.width;
  const effH = allowSize ? hMm : size.height;
  const detail =
    format === 'tiff'
      ? `${pngPixels(effW, dpi)} × ${pngPixels(effH, dpi)} pixels at ${dpi} dpi, ${trim(effW, 1)} × ${trim(effH, 1)} mm, RGB with LZW compression on a white background.`
      : format === 'png'
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
  const ok = sizeOk && dpiOk && nameOk && (format !== 'tiff' || lab);
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
      <DialogHeader title="Export" sub={`"${title}"`} onClose={() => close(null)} />
      <div className="ex-grid">
        <span className="ex-label">Format</span>
        <Segmented
          testId="export-format"
          label="Format"
          choices={[
            { value: 'pdf' as ExportFormat, label: 'PDF', hint: 'Vector, fonts embedded; for journals.' },
            { value: 'svg' as ExportFormat, label: 'SVG', hint: 'Vector; for editing in other tools.' },
            { value: 'png' as ExportFormat, label: 'PNG', hint: 'Pixels at a chosen resolution.' },
            { value: 'tiff' as ExportFormat, label: 'TIFF', hint: `${TIFF_WHAT} Dioptra Lab.`, testId: 'export-format-tiff' },
          ]}
          value={format}
          onChange={setFormat}
        />
        {format === 'tiff' && !lab && (
          <>
            <span className="ex-label">TIFF</span>
            <div className="ex-lab" data-testid="export-tiff-lab">
              <span>{TIFF_WHAT} Part of Dioptra Lab.</span>
              <Button size="sm" onClick={() => useLab.getState().setDialogOpen(true)}>
                Dioptra Lab…
              </Button>
            </div>
          </>
        )}
        {raster && (
          <>
            <span className="ex-label">Resolution</span>
            <div className="ex-row">
              <Segmented
                testId="export-dpi"
                label="Resolution"
                choices={[...DPIS.map((d): { value: number | 'custom'; label: string } => ({ value: d, label: `${d} dpi` })), { value: 'custom', label: 'Custom' }]}
                value={dpiChoice}
                onChange={setDpiChoice}
              />
              {dpiChoice === 'custom' && (
                <NumberField
                  className="ex-num"
                  label="Custom dpi"
                  value={Number(customDpi)}
                  onText={(t) => setCustomDpi(t)}
                  unit="dpi"
                  min={36}
                  max={2400}
                  step={50}
                  integer
                  nullable={false}
                  invalid={!dpiOk}
                />
              )}
            </div>
          </>
        )}
        {allowSize && (
          <>
            <span className="ex-label">Size</span>
            <div className="ex-row">
              <NumberField
                key={`w-${unit}`}
                className="ex-num"
                label="Width"
                testId="export-width"
                value={Number(w)}
                format={() => w}
                onText={(t) => setW(t)}
                unit={unit}
                step={unit === 'mm' ? 1 : 0.1}
                nullable={false}
                invalid={!sizeOk}
              />
              <span className="ex-times">×</span>
              <NumberField
                key={`h-${unit}`}
                className="ex-num"
                label="Height"
                testId="export-height"
                value={Number(h)}
                format={() => h}
                onText={(t) => setH(t)}
                unit={unit}
                step={unit === 'mm' ? 1 : 0.1}
                nullable={false}
                invalid={!sizeOk}
              />
              <Segmented
                label="Unit"
                choices={[
                  { value: 'mm' as const, label: 'mm' },
                  { value: 'in' as const, label: 'in' },
                ]}
                value={unit}
                onChange={switchUnit}
              />
            </div>
          </>
        )}
        {format === 'png' && allowSize && (
          <>
            <span className="ex-label">Background</span>
            <div className="ex-row">
              <Toggle checked={trans} onChange={setTrans} label="Transparent background" testId="export-transparent" showState={false} />
              <span className="small">Transparent (saved with the graph)</span>
            </div>
          </>
        )}
        <span className="ex-label">File name</span>
        <div className="ex-row">
          <TextInput
            size="sm"
            className="ex-name"
            aria-label="File name"
            testId="export-name"
            value={name}
            invalid={!nameOk}
            onChange={(e) => setName(e.target.value)}
          />
          <span className="ex-ext mono">.{format}</span>
        </div>
      </div>
      <p className="ex-detail" data-testid="export-detail">
        <span className="mono">{filename}</span>: {detail}
      </p>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!ok} testId="export-go">
          {desktop ? 'Choose location…' : 'Download'}
        </Button>
      </footer>
    </form>
  );
}
