import { describe, expect, it } from 'vitest';
import { localToday, standingText } from './lab';
import type { License } from '../engine/types';

const license: License = {
  id: 'L-1',
  licensee: 'A. Lab',
  email: 'a@uni.edu',
  plan: 'academic',
  seats: 1,
  issued: '2026-01-01',
  expires: '2026-12-31',
};

describe('lab license', () => {
  it('formats the local date', () => {
    expect(localToday(new Date(2026, 0, 5))).toBe('2026-01-05');
  });
  it('describes each standing', () => {
    expect(standingText({ lab_enabled: false })).toMatch(/No license key/);
    expect(standingText({ lab_enabled: true, license, standing: { state: 'active', days_left: 200 } })).toBe(
      'Active until 2026-12-31.',
    );
    expect(standingText({ lab_enabled: true, license, standing: { state: 'active', days_left: 10 } })).toMatch(/10 days left/);
    expect(standingText({ lab_enabled: true, license, standing: { state: 'grace', days_left: 3 } })).toMatch(/3 more days/);
    expect(standingText({ lab_enabled: false, license, standing: { state: 'lapsed', days_since: 40 } })).toMatch(/paused/);
  });
});
