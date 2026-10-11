import { describe, expect, it } from 'vitest';
import type { TableMeta } from '../engine/types';
import type { Spec } from '../engine/objects';
import { columnsOf, datasetsOf, fieldsFor, formModel, getAt, SCHEMA, setAt, validate, withSelection } from './schema';

// Every kind declared in crates/dioptra-core/src/analysis.rs (KINDS).
const KINDS = [
  'descriptive', 't_test', 'mann_whitney', 'wilcoxon', 'one_way_anova', 'repeated_measures_anova', 'two_way_anova',
  'kruskal_wallis', 'friedman', 'chi_square', 'fisher_exact', 'correlation', 'linear_regression', 'nonlinear_regression',
  'normality', 'outliers', 'survival', 'roc', 'bland_altman', 'pca',
];

// Default specs exactly as `DefaultAnalysisSpec` returns them for a three-data-set column table.
const DEFAULTS: Record<string, Spec> = {
  descriptive: { kind: 'descriptive', columns: [], ci_level: 0.95, percentiles: [] },
  t_test: { kind: 't_test', columns: [0, 1], variant: 'welch', mu: 0, alternative: 'two_sided', ci_level: 0.95 },
  one_way_anova: {
    kind: 'one_way_anova',
    columns: [],
    post_hoc: { method: { name: 'tukey' }, comparisons: { kind: 'all_pairs' } },
    welch: false,
    brown_forsythe: false,
  },
  outliers: { kind: 'outliers', columns: [], method: { method: 'rout', q: 1 } },
  mann_whitney: { kind: 'mann_whitney', columns: [0, 1], alternative: 'two_sided', exact: null, ci_level: 0.95 },
  two_way_anova: { kind: 'two_way_anova', ss_type: 'iii', post_hoc: null, repeated: null },
};

function meta(datasets: number, subcolumns = 1): TableMeta {
  const columns = [];
  const ds = [];
  for (let d = 0; d < datasets; d++) {
    ds.push({ name: `G${d + 1}`, letter: String.fromCharCode(65 + d), format: { kind: 'values' as const, n: subcolumns }, first_col: d * subcolumns, subcolumns });
    for (let s = 0; s < subcolumns; s++)
      columns.push({ index: d * subcolumns + s, role: 'y' as const, dataset: d, subcolumn: s, label: '', header: '', dataset_name: `G${d + 1}`, source: { kind: 'entered' as const } });
  }
  return { id: 't', title: 'T', kind: 'column', rows: 5, has_x: false, has_row_titles: false, row_titles: [], columns, datasets: ds, read_only: false };
}

describe('options schema', () => {
  it('covers every analysis kind of the engine', () => {
    for (const k of KINDS) expect(SCHEMA[k], k).toBeDefined();
    expect(Object.keys(SCHEMA).sort()).toEqual([...KINDS].sort());
  });

  it('labels and explains every field in one sentence', () => {
    for (const k of KINDS) {
      for (const f of fieldsFor(k)) {
        expect(f.label.length, `${k}.${f.path}`).toBeGreaterThan(1);
        expect(f.help, `${k}.${f.path}`).toMatch(/[.!?]$/);
      }
    }
  });

  it('maps a t test spec to form rows with the engine values', () => {
    const rows = formModel(DEFAULTS.t_test!);
    expect(rows.map((r) => r.field.path)).toEqual(['variant', 'columns', 'mu', 'alternative', 'ci_level']);
    const variant = rows[0]!;
    expect(variant.field.type).toBe('enum');
    expect(variant.value).toBe('welch');
    if (variant.field.type === 'enum') {
      expect(variant.field.choices.map((c) => c.value)).toEqual(['welch', 'unpaired', 'paired', 'ratio_paired', 'one_sample']);
      expect(variant.field.choices[0]!.hint).toMatch(/Recommended/);
    }
    expect(rows.find((r) => r.field.path === 'ci_level')!.value).toBe(0.95);
  });

  it('maps nested and nullable values (post hoc, outlier method, exact p)', () => {
    const anova = formModel(DEFAULTS.one_way_anova!);
    const ph = anova.find((r) => r.field.type === 'postHoc')!;
    expect(ph.value).toEqual({ method: { name: 'tukey' }, comparisons: { kind: 'all_pairs' } });
    const out = formModel(DEFAULTS.outliers!).find((r) => r.field.type === 'variant')!;
    expect(out.value).toEqual({ method: 'rout', q: 1 });
    const mw = formModel(DEFAULTS.mann_whitney!).find((r) => r.field.path === 'exact')!;
    expect(mw.value).toBeNull();
    if (mw.field.type === 'enum') expect(mw.field.choices.map((c) => c.value)).toEqual([null, true, false]);
    const two = formModel(DEFAULTS.two_way_anova!);
    expect(two.find((r) => r.field.path === 'repeated')!.value).toBeNull();
  });

  it('writes values back by path without touching the rest', () => {
    const s = setAt(DEFAULTS.t_test!, 'variant', 'paired');
    expect(s.variant).toBe('paired');
    expect(DEFAULTS.t_test!.variant).toBe('welch');
    const n = setAt({ kind: 'nonlinear_regression', equation: { name: 'four_parameter_logistic' } } as Spec, 'equation.formula', 'Y = a*X');
    expect(getAt(n, 'equation.name')).toBe('four_parameter_logistic');
    expect(getAt(n, 'equation.formula')).toBe('Y = a*X');
  });

  it('converts between data sets and flat columns', () => {
    const m = meta(3, 2);
    expect(columnsOf(m, [0, 2], 'first')).toEqual([0, 4]);
    expect(columnsOf(m, [1], 'all')).toEqual([2, 3]);
    expect(columnsOf(m, [1, 2], 'dataset')).toEqual([1, 2]);
    expect(datasetsOf(m, [0, 1, 4], 'all')).toEqual([0, 2]);
    expect(datasetsOf(m, [2, 0], 'dataset')).toEqual([2, 0]);
  });

  it('validates column counts and ranges', () => {
    const m = meta(3);
    expect(validate(DEFAULTS.t_test!, m)).toEqual([]);
    expect(validate(setAt(DEFAULTS.t_test!, 'columns', [0]), m)).toEqual(['Data sets: pick 2 data sets.']);
    expect(validate(setAt(setAt(DEFAULTS.t_test!, 'columns', [0]), 'variant', 'one_sample'), m)).toEqual([]);
    expect(validate(setAt(DEFAULTS.t_test!, 'ci_level', 1.2), m)[0]).toMatch(/Confidence level/);
    expect(validate(DEFAULTS.descriptive!, m)).toEqual([]);
  });

  it('narrows a default spec to the selected data sets', () => {
    const m = meta(3);
    expect(withSelection(DEFAULTS.descriptive!, m, [1, 2]).columns).toEqual([1, 2]);
    expect(withSelection(DEFAULTS.t_test!, m, [1, 2]).columns).toEqual([1, 2]);
    // One data set is too few for a two-group test: the engine default stays.
    expect(withSelection(DEFAULTS.t_test!, m, [2]).columns).toEqual([0, 1]);
    expect(withSelection(DEFAULTS.t_test!, m, []).columns).toEqual([0, 1]);
  });
});
