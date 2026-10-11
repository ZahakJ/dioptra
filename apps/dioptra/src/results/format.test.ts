import { describe, expect, it } from 'vitest';
import { fixed, formatP, formatSig, pPhrase, roundSig, stars } from './format';

describe('fixed (Rust {:.d}, exact value, ties to even)', () => {
  it('rounds exact ties to even like Rust', () => {
    expect(fixed(0.0625, 3)).toBe('0.062');
    expect(fixed(0.03125, 4)).toBe('0.0312');
    expect(fixed(2.5, 0)).toBe('2');
    expect(fixed(3.5, 0)).toBe('4');
  });
  it('rounds non-ties by the exact binary value', () => {
    expect(fixed(0.0015, 3)).toBe('0.002');
    expect(fixed(0.00125, 4)).toBe('0.0013');
    expect(fixed(0.12345, 3)).toBe('0.123');
    expect(fixed(0.9995, 3)).toBe('1.000');
    expect(fixed(0.0005, 3)).toBe('0.001');
    expect(fixed(-1.25, 1)).toBe('-1.2');
    expect(fixed(123.456, 0)).toBe('123');
  });
});

describe('formatP matches results.rs', () => {
  it('reproduces the Rust unit test', () => {
    expect(formatP(0.0049, 'apa')).toBe('.005');
    expect(pPhrase(0.0049, 'apa')).toBe('p = .005');
    expect(pPhrase(0.0004, 'apa')).toBe('p < .001');
    expect(formatP(0.0049, 'nejm')).toBe('0.005');
    expect(pPhrase(0.0004, 'nejm')).toBe('P<0.001');
    expect(pPhrase(0.0049, 'nejm')).toBe('P=0.005');
    expect(formatP(0.0049, 'gp')).toBe('0.0049 (**)');
    expect(formatP(0.00001, 'gp')).toBe('<0.0001 (****)');
    expect(formatP(0.05000001, 'gp')).toBe('0.0500 (ns)');
    expect(formatP(0.0004, 'gp')).toBe('0.0004 (***)');
    expect(formatP(0.049, 'gp')).toBe('0.0490 (*)');
    expect(formatP(0.004921, 'exact')).toBe('0.004921');
    expect(formatP(0.123456, 'exact')).toBe('0.1235');
    expect(formatP(0.00005, 'exact')).toBe('< 0.0001');
    expect(pPhrase(0.00005, 'exact')).toBe('p < 0.0001');
    expect(stars(0.05)).toBe('ns');
  });

  it('APA thresholds', () => {
    expect(formatP(0.05, 'apa')).toBe('.050');
    expect(formatP(0.01, 'apa')).toBe('.010');
    expect(formatP(0.001, 'apa')).toBe('.001');
    expect(formatP(0.000999, 'apa')).toBe('< .001');
    expect(formatP(0.0001, 'apa')).toBe('< .001');
    expect(formatP(0.0625, 'apa')).toBe('.062');
  });

  it('NEJM thresholds', () => {
    expect(formatP(0.05, 'nejm')).toBe('0.050');
    expect(formatP(0.01, 'nejm')).toBe('0.010');
    expect(formatP(0.001, 'nejm')).toBe('0.001');
    expect(formatP(0.0009, 'nejm')).toBe('<0.001');
    expect(formatP(0.0001, 'nejm')).toBe('<0.001');
  });

  it('GP asterisks at each threshold, decided on the stored p', () => {
    expect(formatP(0.05, 'gp')).toBe('0.0500 (ns)');
    expect(formatP(0.0499, 'gp')).toBe('0.0499 (*)');
    expect(formatP(0.01, 'gp')).toBe('0.0100 (*)');
    expect(formatP(0.0099, 'gp')).toBe('0.0099 (**)');
    expect(formatP(0.001, 'gp')).toBe('0.0010 (**)');
    expect(formatP(0.00099, 'gp')).toBe('0.0010 (***)');
    expect(formatP(0.0001, 'gp')).toBe('0.0001 (***)');
    expect(formatP(0.00009, 'gp')).toBe('<0.0001 (****)');
    expect(stars(0.0001)).toBe('***');
    expect(stars(0.00009999)).toBe('****');
  });

  it('exact style thresholds', () => {
    expect(formatP(0.05, 'exact')).toBe('0.05');
    expect(formatP(0.01, 'exact')).toBe('0.01');
    expect(formatP(0.001, 'exact')).toBe('0.001');
    expect(formatP(0.0001, 'exact')).toBe('0.0001');
    expect(formatP(0.00009999, 'exact')).toBe('< 0.0001');
    expect(formatP(0.03125, 'exact')).toBe('0.03125');
  });

  it('p at or above 0.9995', () => {
    expect(formatP(0.9995, 'apa')).toBe('> .999');
    expect(formatP(0.999, 'apa')).toBe('.999');
    expect(formatP(0.99949, 'apa')).toBe('> .999');
    expect(formatP(0.9994, 'nejm')).toBe('>0.999');
    expect(formatP(0.9989, 'nejm')).toBe('0.999');
    expect(formatP(0.9995, 'nejm')).toBe('>0.999');
    expect(formatP(0.9995, 'gp')).toBe('0.9995 (ns)');
    expect(formatP(0.99995, 'gp')).toBe('>0.9999 (ns)');
    expect(formatP(0.9995, 'exact')).toBe('0.9995');
    expect(formatP(0.99996, 'exact')).toBe('1');
    expect(formatP(1, 'apa')).toBe('> .999');
    expect(pPhrase(1, 'apa')).toBe('p > .999');
    expect(pPhrase(0.99995, 'gp')).toBe('P >0.9999');
  });

  it('missing p is empty', () => {
    expect(formatP(Number.NaN, 'apa')).toBe('');
    expect(formatP(null, 'gp')).toBe('');
  });
});

describe('formatSig matches util::format_sig', () => {
  it('four significant digits by default', () => {
    expect(formatSig(0.5773502691896258, undefined)).toBe('0.5774');
    expect(formatSig(1234.5678, undefined)).toBe('1235');
    expect(formatSig(50, undefined)).toBe('50');
    expect(formatSig(-0.48413771175032894, undefined)).toBe('-0.4841');
    expect(formatSig(8.166666666666666, undefined)).toBe('8.167');
    expect(formatSig(0, undefined)).toBe('0');
    expect(formatSig(99996, undefined)).toBe('100000');
  });
  it('switches to scientific outside [1e-4, 1e15)', () => {
    expect(formatSig(0.000012345, undefined)).toBe('1.234E-5');
    expect(formatSig(1e20, undefined)).toBe('1E20');
    expect(formatSig(2.5e15, undefined)).toBe('2.5E15');
  });
  it('fixed decimals', () => {
    expect(formatSig(3, 0)).toBe('3');
    expect(formatSig(-0.00001, 2)).toBe('0.00');
    expect(formatSig(2.5, 0)).toBe('2');
  });
  it('empty for missing values', () => {
    expect(formatSig(null, undefined)).toBe('');
    expect(formatSig(Number.NaN, 2)).toBe('');
  });
  it('roundSig keeps significant digits half to even', () => {
    expect(roundSig(0.12345, 4)).toBe(0.1235);
    expect(roundSig(0.00048828125, 4)).toBe(0.0004883);
    expect(roundSig(-2.5, 1)).toBe(-2);
  });
});
