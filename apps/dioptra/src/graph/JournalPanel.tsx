// The Journal control of the graph editor and the layout view: a preset and column width
// applied in one step, the layout's final-size settings, and the figure check. Dioptra Lab.
import { useCallback, useEffect, useRef, useState } from 'react';
import { create } from 'zustand';
import { CircleX, Info, Newspaper, TriangleAlert } from 'lucide-react';
import { Button, NumberField, Popover, Segmented, Select } from '../ui';
import type { Id } from '../engine/types';
import { useLab } from '../store/lab';
import { withEngine } from '../store/engine';
import { notifyError } from '../store/notices';
import { applyChanged, onChanged } from '../store/sync';
import {
  applyJournalPreset,
  checkFigure,
  JOURNAL_WHAT,
  loadPresets,
  widthOf,
  type ColumnWidth,
  type FigureCheck,
  type FigureIssue,
  type JournalPreset,
  type LayoutStyle,
} from './journal';
import './journal.css';

/** The journal and width last chosen, for every figure of the session. */
export const useJournalChoice = create<{ preset: string; width: ColumnWidth; set(p: Partial<{ preset: string; width: ColumnWidth }>): void }>(
  (set) => ({ preset: 'nature', width: 'single', set: (p) => set(p) }),
);

const WIDTHS: { value: ColumnWidth; label: string }[] = [
  { value: 'single', label: 'Single' },
  { value: 'onehalf', label: '1.5' },
  { value: 'double', label: 'Double' },
];

export function JournalButton({
  id,
  kind,
  onSelect,
  layoutStyle,
  onLayoutStyle,
}: {
  id: Id;
  kind: 'graph' | 'layout';
  /** Select an element of the figure (the graph editor's selection). */
  onSelect?: (element: string) => boolean;
  layoutStyle?: LayoutStyle;
  onLayoutStyle?: (s: LayoutStyle) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLButtonElement>(null);
  return (
    <>
      <Button
        ref={ref}
        size="sm"
        icon={<Newspaper size={16} strokeWidth={1.5} aria-hidden="true" />}
        onClick={() => setOpen((o) => !o)}
        testId={`${kind}-journal`}
        aria-haspopup="dialog"
        aria-expanded={open}
        tipText="Journal size and figure check"
      >
        Journal
      </Button>
      <Popover anchor={ref.current} open={open} onClose={() => setOpen(false)} placement="bottom-end" label="Journal" role="dialog" testId="journal-panel" className="jp">
        <JournalPanel id={id} kind={kind} onSelect={onSelect} layoutStyle={layoutStyle} onLayoutStyle={onLayoutStyle} />
      </Popover>
    </>
  );
}

function LabOff() {
  return (
    <div className="jp-off" data-testid="journal-lab-off">
      <p>{JOURNAL_WHAT}</p>
      <p className="muted small">Part of Dioptra Lab.</p>
      <Button size="sm" variant="primary" onClick={() => useLab.getState().setDialogOpen(true)} testId="journal-lab-open">
        Dioptra Lab…
      </Button>
    </div>
  );
}

export function JournalPanel({
  id,
  kind,
  onSelect,
  layoutStyle,
  onLayoutStyle,
}: {
  id: Id;
  kind: 'graph' | 'layout';
  onSelect?: (element: string) => boolean;
  layoutStyle?: LayoutStyle;
  onLayoutStyle?: (s: LayoutStyle) => void;
}) {
  const enabled = useLab((s) => s.view.lab_enabled);
  const [presets, setPresets] = useState<JournalPreset[]>([]);
  const choice = useJournalChoice();
  const [check, setCheck] = useState<FigureCheck | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    loadPresets().then(setPresets, notifyError);
  }, []);
  const preset = presets.find((p) => p.id === choice.preset) ?? presets[0];
  const run = useCallback(() => {
    if (!enabled || !preset) return;
    withEngine((e) => checkFigure(e, id, preset.id)).then(setCheck, notifyError);
  }, [enabled, preset, id]);
  useEffect(() => run(), [run]);
  useEffect(() => onChanged((c) => c.ids.includes(id) && run()), [id, run]);
  if (!enabled) return <LabOff />;
  if (!preset) return <div className="jp-body muted small">Loading…</div>;
  const width = widthOf(preset, choice.width) === null ? 'single' : choice.width;
  const apply = async (targets: 'all' | Id[]) => {
    setBusy(true);
    try {
      const ids = await withEngine((e) => applyJournalPreset(e, preset.id, width, targets));
      await applyChanged({ ids, tree: false });
    } catch (err) {
      notifyError(err);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="jp-body">
      <div className="jp-grid">
        <span className="jp-label" id={`jp-journal-${id}`}>
          Journal
        </span>
        <Select
          labelledBy={`jp-journal-${id}`}
          testId="journal-preset"
          value={preset.id}
          options={presets.map((p) => ({ value: p.id, label: p.name }))}
          onChange={(v) => choice.set({ preset: v })}
        />
        <span className="jp-label" id={`jp-width-${id}`}>
          Width
        </span>
        <Segmented
          labelledBy={`jp-width-${id}`}
          testId="journal-width"
          value={width}
          onChange={(w) => choice.set({ width: w })}
          choices={WIDTHS.filter((w) => widthOf(preset, w.value) !== null).map((w) => ({
            value: w.value,
            label: w.label,
            hint: `${widthOf(preset, w.value)} mm`,
            testId: `journal-width-${w.value}`,
          }))}
        />
      </div>
      <p className="jp-spec small muted" data-testid="journal-spec">
        {widthOf(preset, width)} mm wide{preset.max_height_mm ? `, at most ${preset.max_height_mm} mm tall` : ''}; {preset.font}, {preset.min_font_pt}
        {preset.max_font_pt ? `–${preset.max_font_pt}` : ''} pt; lines at least {preset.min_line_pt} pt; {preset.file_types.join(', ')}.
      </p>
      <div className="jp-actions">
        <Button size="sm" variant="primary" disabled={busy} onClick={() => void apply([id])} testId="journal-apply">
          {kind === 'graph' ? 'Apply to this graph' : 'Apply to this layout'}
        </Button>
        <Button size="sm" disabled={busy} onClick={() => void apply('all')} testId="journal-apply-all">
          Apply to every figure
        </Button>
      </div>
      {kind === 'layout' && layoutStyle && onLayoutStyle && <LayoutFinalSize style={layoutStyle} onChange={onLayoutStyle} />}
      <CheckList check={check} onSelect={onSelect} />
    </div>
  );
}

function LayoutFinalSize({ style, onChange }: { style: LayoutStyle; onChange: (s: LayoutStyle) => void }) {
  const set = (p: Partial<LayoutStyle>) => onChange({ ...style, ...p });
  return (
    <section className="jp-section" aria-label="Final size" data-testid="layout-final-size">
      <h3 className="jp-h">Final size</h3>
      <div className="jp-grid">
        <span className="jp-label" id="jp-letters">
          Letters
        </span>
        <Segmented
          labelledBy="jp-letters"
          testId="layout-letters"
          value={style.letters ?? 'upper'}
          onChange={(v) => set({ letters: v })}
          choices={[
            { value: 'upper' as const, label: 'A B C' },
            { value: 'lower' as const, label: 'a b c' },
            { value: 'none' as const, label: 'None' },
          ]}
        />
        <span className="jp-label" id="jp-letter-pt">
          Letter size
        </span>
        <NumberField labelledBy="jp-letter-pt" testId="layout-letter-pt" value={style.letter_pt ?? null} placeholder="10" unit="pt" min={2} max={72} step={0.5} onCommit={(v) => set({ letter_pt: v })} />
        <span className="jp-label" id="jp-width-mm">
          Page width
        </span>
        <NumberField labelledBy="jp-width-mm" testId="layout-width" value={style.width_mm ?? null} unit="mm" min={20} max={1000} step={1} onCommit={(v) => set({ width_mm: v })} />
        <span className="jp-label" id="jp-font-pt">
          Text
        </span>
        <NumberField labelledBy="jp-font-pt" testId="layout-font" value={style.font_pt ?? null} unit="pt" min={2} max={72} step={0.5} onCommit={(v) => set({ font_pt: v })} />
        <span className="jp-label" id="jp-line-pt">
          Lines
        </span>
        <NumberField labelledBy="jp-line-pt" testId="layout-line" value={style.line_pt ?? null} unit="pt" min={0.05} max={10} step={0.05} onCommit={(v) => set({ line_pt: v })} />
      </div>
      <p className="jp-spec small muted">Empty fields keep each graph’s own size, text and lines.</p>
    </section>
  );
}

const ICON = { error: CircleX, warning: TriangleAlert, info: Info };
const WORD = { error: 'Error', warning: 'Warning', info: 'Note' };

function CheckList({ check, onSelect }: { check: FigureCheck | null; onSelect?: (element: string) => boolean }) {
  if (!check) return null;
  const counts = (s: FigureIssue['severity']) => check.issues.filter((i) => i.severity === s).length;
  return (
    <section className="jp-section" aria-label="Figure check" data-testid="figure-check">
      <h3 className="jp-h">
        Figure check
        <span className="muted small jp-sum" data-testid="figure-check-summary">
          {check.issues.length === 0
            ? 'Nothing to fix.'
            : [counts('error') && `${counts('error')} to fix`, counts('warning') && `${counts('warning')} to review`, counts('info') && `${counts('info')} ${counts('info') === 1 ? 'note' : 'notes'}`]
                .filter(Boolean)
                .join(', ')}
        </span>
      </h3>
      {check.issues.length > 0 && (
        <ul className="jp-issues">
          {check.issues.map((i, n) => {
            const Icon = ICON[i.severity];
            const body = (
              <>
                <Icon size={14} strokeWidth={1.5} aria-label={WORD[i.severity]} className={`jp-ic is-${i.severity}`} />
                <span>{i.message}</span>
              </>
            );
            return (
              <li key={n} data-testid={`figure-issue-${i.rule}`} data-severity={i.severity}>
                {i.element && onSelect ? (
                  <button type="button" className="jp-issue is-link" onClick={() => onSelect(i.element!)}>
                    {body}
                  </button>
                ) : (
                  <div className="jp-issue">{body}</div>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
