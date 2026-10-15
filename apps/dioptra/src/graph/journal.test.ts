import { describe, expect, it } from 'vitest';
import { widthOf, type JournalPreset } from './journal';

const plos: JournalPreset = {
  id: 'plos',
  name: 'PLOS',
  single_mm: 132,
  onehalf_mm: null,
  double_mm: 190.5,
  max_height_mm: 222.3,
  font: 'Arial',
  min_font_pt: 8,
  max_font_pt: 12,
  min_line_pt: 0.25,
  dpi_line_art: 300,
  dpi_halftone: 300,
  dpi_combination: 300,
  file_types: ['TIFF', 'EPS'],
  file_note: '',
  letters: 'upper',
  letter_pt: 10,
};

describe('journal presets', () => {
  it('gives the width of each column choice, or none', () => {
    expect(widthOf(plos, 'single')).toBe(132);
    expect(widthOf(plos, 'double')).toBe(190.5);
    expect(widthOf(plos, 'onehalf')).toBeNull();
  });
});
