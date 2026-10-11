import { describe, expect, it } from 'vitest';
import { boxToPx, clampZoom, fitZoom, inToMm, mmToIn, mmToPt, pngPixels, ptToMm, ptToPx, pxPerPt, pxToPt, trim } from './units';
import { axisMapFrom, moveBy, parseTickText, valueAt } from './svgAxes';

describe('pt / px / mm conversion', () => {
  it('points and pixels at a zoom', () => {
    expect(pxPerPt(1)).toBeCloseTo(4 / 3, 12);
    expect(ptToPx(72, 1)).toBeCloseTo(96, 12);
    expect(ptToPx(72, 2)).toBeCloseTo(192, 12);
    expect(pxToPt(96, 1)).toBeCloseTo(72, 12);
    expect(pxToPt(ptToPx(13.7, 0.5), 0.5)).toBeCloseTo(13.7, 12);
  });
  it('millimetres', () => {
    expect(mmToPt(25.4)).toBeCloseTo(72, 12);
    expect(mmToPt(85)).toBeCloseTo(240.94, 2); // the SVG viewBox width of an 85 mm graph
    expect(ptToMm(mmToPt(70))).toBeCloseTo(70, 12);
    expect(mmToIn(25.4)).toBe(1);
    expect(inToMm(3.5)).toBeCloseTo(88.9, 12);
  });
  it('target boxes to screen boxes', () => {
    expect(boxToPx({ x: 72, y: 36, width: 7.2, height: 3.6 }, 1)).toEqual({ x: 96, y: 48, width: 9.6, height: 4.8 });
  });
  it('fit zoom and its limits', () => {
    // An 85 mm figure is 321.26 px at 96 dpi; a 700 px wide, tall view leaves 628 px.
    expect(fitZoom(85, 70, 700, 2000)).toBeCloseTo((700 - 72) / ((85 / 25.4) * 96), 6);
    expect(fitZoom(85, 70, 10000, 10000)).toBe(2);
    expect(fitZoom(500, 500, 100, 100)).toBe(0.5);
    expect(clampZoom(Number.NaN)).toBe(1);
  });
  it('PNG pixels as the engine computes them', () => {
    expect(pngPixels(85, 300)).toBe(1004);
    expect(pngPixels(70, 300)).toBe(827);
    expect(trim(1.23456, 2)).toBe('1.23');
  });
});

describe('reading the data scale from tick labels', () => {
  it('parses tick labels', () => {
    expect(parseTickText('−2.5')).toBe(-2.5);
    expect(parseTickText('2×10³')).toBe(2000);
    expect(parseTickText('10⁻²')).toBeCloseTo(0.01, 12);
    expect(parseTickText('10^3')).toBe(1000);
    expect(parseTickText('1,000')).toBe(1000);
    expect(parseTickText('Control')).toBeNull();
  });
  it('maps linear and log axes', () => {
    // y axis: 0 at 184.09 pt, 8 at 79.01 pt (from a rendered SVG)
    const y = axisMapFrom([{ pt: 184.09, v: 0 }, { pt: 79.01, v: 8 }], 'linear')!;
    expect(moveBy(y, 5, -(184.09 - 79.01) / 8)).toBeCloseTo(6, 9);
    const x = axisMapFrom([{ pt: 64.12, v: 1 }, { pt: 198.54, v: 3 }], 'categorical')!;
    expect(valueAt(x, 131.33)).toBeCloseTo(2, 3);
    const log = axisMapFrom([{ pt: 0, v: 1 }, { pt: 100, v: 1000 }], 'log10')!;
    expect(moveBy(log, 10, 100 / 3)).toBeCloseTo(100, 9);
    expect(axisMapFrom([{ pt: 1, v: 1 }], 'linear')).toBeNull();
  });
});
