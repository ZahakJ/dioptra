// Design round 2: the bounded sheet, timestamps, density, the zoom model, paper, check marks.
import { beforeEach, describe, expect, it } from 'vitest';
import { datasetColour, headHeight, MIN_ROWS, OKABE_ITO, rowHeight, sheetRows, SPARE_ROWS } from '../grid/geometry';
import { presetOf, pxPerMm, rulerTicks, zoomLabel, zoomStep, ZOOM_MAX, ZOOM_MIN } from '../graph/units';
import { worstStatus } from '../results/sheetModel';
import { paperOf, useUi } from '../store/ui';
import { formatWhen, humanizeTimestamps } from './time';

describe('the bounded sheet', () => {
  it('ends 12 rows below the data and never shows fewer than 20 rows', () => {
    expect(SPARE_ROWS).toBe(12);
    expect(MIN_ROWS).toBe(20);
    expect(sheetRows(0)).toBe(20);
    expect(sheetRows(8)).toBe(20);
    expect(sheetRows(9)).toBe(21);
    expect(sheetRows(48)).toBe(60);
    expect(sheetRows(1000)).toBe(1012);
  });
  it('grows by the rows the user added', () => {
    expect(sheetRows(8, 20)).toBe(40);
    expect(sheetRows(100, 40)).toBe(152);
  });
  it('treats bad input as an empty sheet', () => {
    expect(sheetRows(-5)).toBe(20);
    expect(sheetRows(Number.NaN)).toBe(20);
    expect(sheetRows(3, -20)).toBe(20);
  });
});

describe('data set colours', () => {
  it('follow Okabe–Ito by data set index, wrapping', () => {
    expect(datasetColour(0)).toBe('#000000');
    expect(datasetColour(1)).toBe('#E69F00');
    expect(datasetColour(3)).toBe('#009E73');
    expect(datasetColour(8)).toBe(OKABE_ITO[0]);
    expect(datasetColour(2, ['#111', '#222'])).toBe('#111');
  });
});

describe('density', () => {
  beforeEach(() => useUi.setState({ density: 'comfortable' }));
  it('row and header heights', () => {
    expect(rowHeight('comfortable')).toBe(28);
    expect(rowHeight('compact')).toBe(22);
    expect(rowHeight(undefined)).toBe(28);
    expect(headHeight('compact')).toBeLessThanOrEqual(headHeight('comfortable'));
  });
  it('is a persisted preference in the ui store', () => {
    expect(useUi.getState().density).toBe('comfortable');
    useUi.getState().setDensity('compact');
    expect(useUi.getState().density).toBe('compact');
    const stored = JSON.parse(window.localStorage.getItem('dioptra-ui') ?? '{}');
    expect(stored.state.density).toBe('compact');
    useUi.getState().setDensity('nonsense' as never);
    expect(useUi.getState().density).toBe('comfortable');
  });
});

describe('paper', () => {
  beforeEach(() => useUi.setState({ paper: {} }));
  it('is white by default and remembered per graph', () => {
    expect(paperOf(useUi.getState().paper, 'g1')).toBe('white');
    useUi.getState().setPaper('g1', 'checker');
    useUi.getState().setPaper('g2', 'theme');
    expect(paperOf(useUi.getState().paper, 'g1')).toBe('checker');
    expect(paperOf(useUi.getState().paper, 'g2')).toBe('theme');
    expect(paperOf(useUi.getState().paper, 'g3')).toBe('white');
    const stored = JSON.parse(window.localStorage.getItem('dioptra-ui') ?? '{}');
    expect(stored.state.paper).toEqual({ g1: 'checker', g2: 'theme' });
  });
  it('rejects unknown papers', () => {
    useUi.getState().setPaper('g1', 'purple' as never);
    expect(paperOf(useUi.getState().paper, 'g1')).toBe('white');
    expect(paperOf({ g1: 'purple' as never }, 'g1')).toBe('white');
  });
});

describe('human-readable timestamps', () => {
  const now = new Date(2026, 9, 11, 23, 40);
  const at = (y: number, mo: number, d: number, h: number, mi: number) => new Date(y, mo, d, h, mi).toISOString();
  it('today, yesterday, and a short date otherwise', () => {
    expect(formatWhen(at(2026, 9, 11, 23, 18), now)).toBe('today 23:18');
    expect(formatWhen(at(2026, 9, 10, 9, 5), now)).toBe('yesterday 09:05');
    expect(formatWhen(at(2026, 9, 1, 9, 12), now)).toBe('1 Oct 2026, 09:12');
    expect(formatWhen(at(2025, 11, 31, 0, 0), now)).toBe('31 Dec 2025, 00:00');
  });
  it('yesterday across a month boundary', () => {
    expect(formatWhen(at(2026, 8, 30, 8, 0), new Date(2026, 9, 1, 7, 0))).toBe('yesterday 08:00');
  });
  it('leaves what is not a date alone', () => {
    expect(formatWhen('not a date', now)).toBe('not a date');
  });
  it('rewrites instants inside provenance sentences', () => {
    const iso = at(2026, 9, 11, 23, 18);
    expect(humanizeTimestamps(`Pasted from the clipboard at ${iso}.`, now)).toBe('Pasted from the clipboard today at 23:18.');
    expect(humanizeTimestamps(`Typed at ${at(2026, 9, 1, 9, 12)}`, now)).toBe('Typed on 1 Oct 2026 at 09:12');
    expect(humanizeTimestamps('Imported from data.csv, column B', now)).toBe('Imported from data.csv, column B');
  });
});

describe('the zoom model', () => {
  it('names the presets Fit · 50 · 100 · 200', () => {
    expect(presetOf('fit')).toBe('fit');
    expect(presetOf(0.5)).toBe(0.5);
    expect(presetOf(1)).toBe(1);
    expect(presetOf(2)).toBe(2);
    expect(presetOf(1.25)).toBeNull();
  });
  it('steps in and out between the limits', () => {
    expect(zoomStep(1, 1)).toBe(1.25);
    expect(zoomStep(1, -1)).toBe(0.75);
    expect(zoomStep(0.83, 1)).toBe(1);
    expect(zoomStep(4, 1)).toBe(ZOOM_MAX);
    expect(zoomStep(0.5, -1)).toBe(0.33);
    expect(zoomStep(0.25, -1)).toBe(ZOOM_MIN);
    expect(zoomLabel(0.667)).toBe('67%');
  });
  it('rulers tick every 5 mm with labels every 10 mm, tracking zoom', () => {
    const t = rulerTicks(85, 1);
    expect(t.map((x) => x.mm)).toEqual([0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75, 80, 85]);
    expect(t.filter((x) => x.major).map((x) => x.mm)).toEqual([0, 10, 20, 30, 40, 50, 60, 70, 80]);
    expect(t[2]!.px).toBeCloseTo(10 * pxPerMm(1), 9);
    expect(rulerTicks(85, 2)[2]!.px).toBeCloseTo(2 * t[2]!.px, 9);
    expect(pxPerMm(1)).toBeCloseTo(96 / 25.4, 12);
  });
});

describe('assumption card rule', () => {
  const s = (st: string[]) => ({ assumptions: st.map((status) => ({ name: '', detail: '', status: status as never })) });
  it('takes the worst status', () => {
    expect(worstStatus(s(['pass', 'not_checked']))).toBe('pass');
    expect(worstStatus(s(['pass', 'caution']))).toBe('caution');
    expect(worstStatus(s(['caution', 'fail', 'pass']))).toBe('fail');
    expect(worstStatus(s(['not_checked']))).toBe('not_checked');
  });
});
