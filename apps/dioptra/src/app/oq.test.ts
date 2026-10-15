// Records → Run OQ...: number display and the desktop-only menu item.
import { describe, expect, it } from 'vitest';
import { numText, OQ_COMMANDS } from './OqDialog';

describe('Run OQ', () => {
  it('shows numbers with up to 10 significant digits and keeps non-finite text', () => {
    expect(numText(13)).toBe('13');
    expect(numText(0.1234567890123)).toBe('0.123456789');
    expect(numText(1.5e-300)).toBe('1.5e-300');
    expect(numText('Infinity')).toBe('Infinity');
  });
  it('has no menu item outside the desktop app', () => {
    expect(OQ_COMMANDS).toEqual([]);
  });
});
