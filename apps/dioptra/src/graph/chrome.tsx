// The editor's chrome around a figure: millimetre rulers that track the zoom, the zoom control
// (Fit · 50 · 100 · 200, −/+ and a readout) and the paper the figure sits on. Shared by the
// graph editor and the layout view.
import { Minus, Plus } from 'lucide-react';
import { Button, Segmented } from '../ui';
import type { Paper } from '../store/ui';
import { presetOf, pxPerMm, rulerTicks, zoomLabel, zoomStep, type ZoomPreset } from './units';

export type ZoomState = number | 'fit';

const RULER = 18;

/** Top and left rulers in mm: ticks every 5 mm, labels every 10 mm (20 when they would touch). */
export function Rulers({ wMm, hMm, zoom }: { wMm: number; hMm: number; zoom: number }) {
  const k = pxPerMm(zoom);
  // Labels every 10 mm unless they would crowd: a label needs its digits plus a gap.
  const need = String(Math.floor(Math.max(wMm, hMm))).length * 6.5 + 14;
  const every = k * 10 >= need ? 10 : k * 20 >= need ? 20 : 50;
  const wPx = wMm * k;
  const hPx = hMm * k;
  const top = rulerTicks(wMm, zoom);
  const left = rulerTicks(hMm, zoom);
  const len = (t: { mm: number; major: boolean }) => (t.mm % every === 0 ? 7 : t.major ? 5 : 3);
  return (
    <>
      <svg className="ruler ruler-top" width={wPx} height={RULER} viewBox={`0 0 ${wPx} ${RULER}`} aria-hidden="true" data-testid="ruler-top">
        {top.map((t) => {
          const x = Math.min(Math.round(t.px) + 0.5, wPx - 0.5);
          return <line key={t.mm} x1={x} x2={x} y1={RULER} y2={RULER - len(t)} />;
        })}
        {top
          .filter((t) => t.mm % every === 0 && t.mm > 0 && t.px < wPx - 12)
          .map((t) => (
            <text key={t.mm} x={Math.round(t.px) + 3} y={RULER - 7}>
              {t.mm}
            </text>
          ))}
      </svg>
      <svg className="ruler ruler-left" width={RULER} height={hPx} viewBox={`0 0 ${RULER} ${hPx}`} aria-hidden="true" data-testid="ruler-left">
        {left.map((t) => {
          const y = Math.min(Math.round(t.px) + 0.5, hPx - 0.5);
          return <line key={t.mm} x1={RULER} x2={RULER - len(t)} y1={y} y2={y} />;
        })}
        {left
          .filter((t) => t.mm % every === 0 && t.mm > 0 && t.px < hPx - 12)
          .map((t) => (
            <text key={t.mm} x={RULER - 7} y={Math.round(t.px) + 3} transform={`rotate(-90 ${RULER - 7} ${Math.round(t.px) + 3})`} textAnchor="end">
              {t.mm}
            </text>
          ))}
      </svg>
    </>
  );
}

const PRESET_CHOICES: { value: ZoomPreset; label: string; hint: string; testId: string }[] = [
  { value: 'fit', label: 'Fit', hint: 'Fit the window (Ctrl 0)', testId: 'zoom-fit' },
  { value: 0.5, label: '50', hint: 'Half size', testId: 'zoom-50' },
  { value: 1, label: '100', hint: 'Actual size at 96 dpi', testId: 'zoom-actual' },
  { value: 2, label: '200', hint: 'Twice actual size', testId: 'zoom-200' },
];

/** Fit · 50 · 100 · 200 as a segmented control, then −, the readout and +. */
export function ZoomControl({ state, shown, onChange }: { state: ZoomState; shown: number; onChange: (z: ZoomState) => void }) {
  const preset = presetOf(state);
  return (
    <div className="zoom" role="group" aria-label="Zoom">
      <Segmented
        label="Zoom preset"
        value={preset ?? ('none' as unknown as ZoomPreset)}
        choices={PRESET_CHOICES}
        onChange={(v) => onChange(v)}
      />
      <span className="zoom-steps">
        <Button variant="quiet" size="sm" iconOnly="Zoom out" shortcut="Mod+-" icon={<Minus size={16} strokeWidth={1.5} />} onClick={() => onChange(zoomStep(shown, -1))} testId="zoom-out" />
        <span className="zoom-val mono" data-testid="zoom-value">
          {zoomLabel(shown)}
        </span>
        <Button variant="quiet" size="sm" iconOnly="Zoom in" shortcut="Mod+=" icon={<Plus size={16} strokeWidth={1.5} />} onClick={() => onChange(zoomStep(shown, 1))} testId="zoom-in" />
      </span>
    </div>
  );
}

const PAPER_CHOICES: { value: Paper; label: React.ReactNode; aria: string; hint: string; testId: string }[] = [
  { value: 'white', label: <i className="paper-sw is-white" />, aria: 'White paper', hint: 'White paper, as most figures are printed', testId: 'paper-white' },
  { value: 'theme', label: <i className="paper-sw is-theme" />, aria: 'Theme paper', hint: 'The app’s sheet colour, without the figure background', testId: 'paper-theme' },
  { value: 'checker', label: <i className="paper-sw is-checker" />, aria: 'Transparent', hint: 'A checkerboard: shows what is transparent', testId: 'paper-checker' },
];

export const PAPER_LABELS: Record<Paper, string> = { white: 'White', theme: 'Theme', checker: 'Transparent' };

/** The paper the figure sits on, in the toolbar. */
export function PaperControl({ value, onChange }: { value: Paper; onChange: (p: Paper) => void }) {
  return (
    <span className="tb-labelled tb-paper">
      <span className="tb-pre">Paper</span>
      <Segmented label="Paper" testId="tb-paper" value={value} choices={PAPER_CHOICES} onChange={onChange} />
    </span>
  );
}
