// The options schema: for every analysis kind declared in crates/dioptra-core/src/analysis.rs,
// which fields of its spec the options panel shows, with a label and one plain sentence each.
// The engine supplies the values (`DefaultAnalysisSpec`); this file only says how to show
// them. Enum choices mirror the Rust enums (serde snake_case names).
import type { JsonObject, JsonValue, TableMeta } from '../engine/types';
import type { Spec } from '../engine/objects';

export type Primitive = string | number | boolean | null;

export interface Choice {
  value: Primitive;
  label: string;
  hint?: string;
}

interface Base {
  /** Dot path into the spec, e.g. `post_hoc` or `equation.name`. */
  path: string;
  label: string;
  /** One plain sentence: what the choice means. */
  help: string;
  /** Shown only when this returns true. */
  when?: (spec: Spec) => boolean;
}

export type Field =
  | (Base & { type: 'bool' })
  | (Base & { type: 'enum'; choices: Choice[]; control?: 'seg' | 'select' })
  | (Base & {
      type: 'number';
      min?: number;
      max?: number;
      step?: number;
      /** Shown as a percentage (0.95 as 95). */
      percent?: boolean;
      integer?: boolean;
      /** The field may be empty (null); the placeholder says what empty means. */
      nullable?: string;
    })
  | (Base & { type: 'text'; nullable?: string; suggestions?: string[]; mono?: boolean })
  | (Base & { type: 'numberList'; min?: number; max?: number; placeholder?: string })
  | (Base & {
      type: 'columns';
      /**
       * How picked data sets are stored: `first` = the flat column of each data set's first
       * subcolumn, `all` = every flat column of the data set, `dataset` = data set indices.
       */
      unit: 'first' | 'all' | 'dataset';
      /** Number of data sets allowed, given the rest of the spec. */
      count?: (spec: Spec) => [number, number];
      /** An empty list means "every data set". */
      emptyAll?: boolean;
    })
  | (Base & { type: 'multiEnum'; choices: Choice[] })
  | (Base & { type: 'postHoc'; nullable: boolean; methods: Choice[] })
  | (Base & {
      type: 'variant';
      /** Tag key inside the object, e.g. `method`. */
      tag: string;
      variants: { value: string; label: string; hint: string; defaults: JsonObject; fields: Field[] }[];
    });

// ---- shared choices ------------------------------------------------------------------------

const ALTERNATIVE: Field = {
  type: 'enum',
  path: 'alternative',
  label: 'Tail',
  help: 'Two-sided tests ask whether the groups differ in either direction. Use one-sided only if you chose the direction before seeing the data.',
  control: 'seg',
  choices: [
    { value: 'two_sided', label: 'Two-sided', hint: 'Recommended.' },
    { value: 'greater', label: 'Greater', hint: 'The first is larger.' },
    { value: 'less', label: 'Less', hint: 'The first is smaller.' },
  ],
};

const CI: Field = {
  type: 'number',
  path: 'ci_level',
  label: 'Confidence level',
  help: 'Width of the confidence intervals; 95% is the convention.',
  percent: true,
  min: 50,
  max: 99.99,
  step: 1,
};

const MC_METHODS: Choice[] = [
  { value: 'tukey', label: 'Tukey', hint: 'All pairs, Tukey–Kramer for unequal n. The standard choice.' },
  { value: 'dunnett', label: 'Dunnett', hint: 'Each group against one control. More power than Tukey for that question.' },
  { value: 'holm', label: 'Holm', hint: 'Step-down Bonferroni; always at least as powerful as Bonferroni.' },
  { value: 'sidak', label: 'Šidák', hint: 'Slightly more power than Bonferroni for independent comparisons.' },
  { value: 'bonferroni', label: 'Bonferroni', hint: 'Simple and conservative.' },
  { value: 'benjamini_hochberg', label: 'Benjamini–Hochberg', hint: 'Controls the false discovery rate, not the family-wise error.' },
];

function postHoc(nullable: boolean, extra: Choice[] = []): Field {
  return {
    type: 'postHoc',
    path: 'post_hoc',
    label: 'Multiple comparisons',
    help: 'Which group means to compare after the overall test, and how to correct for making several comparisons.',
    nullable,
    methods: [...MC_METHODS, ...extra],
  };
}

const COLUMNS_ALL = (label = 'Data sets'): Field => ({
  type: 'columns',
  path: 'columns',
  label,
  help: 'Which data sets to include. None picked means all of them.',
  unit: 'first',
  emptyAll: true,
});

const TWO_COLUMNS = (label: string, help: string): Field => ({
  type: 'columns',
  path: 'columns',
  label,
  help,
  unit: 'first',
  count: () => [2, 2],
});

// ---- per kind ------------------------------------------------------------------------------

export const SCHEMA: Record<string, Field[]> = {
  descriptive: [
    { ...COLUMNS_ALL(), unit: 'all' } as Field,
    CI,
    {
      type: 'numberList',
      path: 'percentiles',
      label: 'Extra percentiles',
      help: 'Percentiles to add beside the quartiles, e.g. 5, 95.',
      min: 0,
      max: 100,
      placeholder: 'e.g. 5, 95',
    },
  ],
  t_test: [
    {
      type: 'enum',
      path: 'variant',
      label: 'Test',
      help: 'Welch’s correction does not assume equal variances and is recommended for unpaired data.',
      control: 'select',
      choices: [
        { value: 'welch', label: 'Unpaired, Welch', hint: 'Does not assume equal SDs. Recommended.' },
        { value: 'unpaired', label: 'Unpaired, Student', hint: 'Assumes both groups have the same SD.' },
        { value: 'paired', label: 'Paired', hint: 'Each value in one group pairs with one in the other (same row).' },
        { value: 'ratio_paired', label: 'Ratio paired', hint: 'Paired test on logarithms: for ratios rather than differences.' },
        { value: 'one_sample', label: 'One sample', hint: 'Compares one group with a hypothetical mean.' },
      ],
    },
    {
      type: 'columns',
      path: 'columns',
      label: 'Data sets',
      help: 'The two groups to compare (one for a one-sample test).',
      unit: 'first',
      count: (s) => (s.variant === 'one_sample' ? [1, 1] : [2, 2]),
    },
    {
      type: 'number',
      path: 'mu',
      label: 'Hypothetical value',
      help: 'The mean (one sample) or mean difference that the null hypothesis assumes; usually 0.',
      step: 0.1,
    },
    ALTERNATIVE,
    CI,
  ],
  mann_whitney: [
    TWO_COLUMNS('Data sets', 'The two unpaired groups to compare.'),
    ALTERNATIVE,
    {
      type: 'enum',
      path: 'exact',
      label: 'P value',
      help: 'Exact p values are computed when they are affordable; the normal approximation suits large samples.',
      control: 'seg',
      choices: [
        { value: null, label: 'Automatic' },
        { value: true, label: 'Exact' },
        { value: false, label: 'Approximate' },
      ],
    },
    { ...CI, help: 'Width of the Hodges–Lehmann interval for the difference in location.' } as Field,
  ],
  wilcoxon: [
    {
      type: 'columns',
      path: 'columns',
      label: 'Data sets',
      help: 'Two paired data sets (same rows), or one data set tested against a hypothetical median.',
      unit: 'first',
      count: () => [1, 2],
    },
    {
      type: 'number',
      path: 'mu',
      label: 'Hypothetical median',
      help: 'The median (or median difference) that the null hypothesis assumes; usually 0.',
      step: 0.1,
    },
    ALTERNATIVE,
    CI,
  ],
  one_way_anova: [
    COLUMNS_ALL(),
    {
      type: 'bool',
      path: 'welch',
      label: 'Welch’s ANOVA',
      help: 'Does not assume equal SDs. Choose it when the groups’ spreads differ.',
    },
    {
      type: 'bool',
      path: 'brown_forsythe',
      label: 'Brown–Forsythe ANOVA',
      help: 'Another test robust to unequal SDs, reported beside the ordinary F test.',
    },
    postHoc(true),
  ],
  repeated_measures_anova: [
    COLUMNS_ALL('Treatments'),
    {
      type: 'bool',
      path: 'gg_correction',
      label: 'Geisser–Greenhouse',
      help: 'Corrects for unequal variances of the differences (sphericity). Recommended.',
    },
    postHoc(true),
  ],
  two_way_anova: [
    {
      type: 'enum',
      path: 'ss_type',
      label: 'Sums of squares',
      help: 'Type III (as Prism) tests each factor adjusted for the others; it matters only for unbalanced designs.',
      control: 'seg',
      choices: [
        { value: 'iii', label: 'Type III', hint: 'Recommended.' },
        { value: 'ii', label: 'Type II' },
        { value: 'i', label: 'Type I', hint: 'Sequential.' },
      ],
    },
    {
      type: 'enum',
      path: 'repeated',
      label: 'Repeated measures',
      help: 'Whether one factor was measured repeatedly on the same subjects.',
      control: 'seg',
      choices: [
        { value: null, label: 'None' },
        { value: 'rows', label: 'Rows matched' },
        { value: 'columns', label: 'Columns matched' },
      ],
    },
    postHoc(true),
  ],
  kruskal_wallis: [
    COLUMNS_ALL(),
    { type: 'bool', path: 'dunn', label: 'Dunn’s comparisons', help: 'Compare every pair of groups by mean ranks after the overall test.' },
  ],
  friedman: [
    COLUMNS_ALL('Treatments'),
    { type: 'bool', path: 'dunn', label: 'Dunn’s comparisons', help: 'Compare every pair of treatments by rank sums after the overall test.' },
  ],
  chi_square: [
    {
      type: 'bool',
      path: 'yates',
      label: 'Yates correction',
      help: 'A continuity correction for 2 × 2 tables; conservative, and Fisher’s exact test is usually better.',
    },
  ],
  fisher_exact: [ALTERNATIVE],
  correlation: [
    { ...COLUMNS_ALL('Variables'), unit: 'first' } as Field,
    {
      type: 'enum',
      path: 'method',
      label: 'Coefficient',
      help: 'Pearson measures linear association; Spearman and Kendall use ranks and suit monotonic or skewed data.',
      control: 'seg',
      choices: [
        { value: 'pearson', label: 'Pearson' },
        { value: 'spearman', label: 'Spearman' },
        { value: 'kendall', label: 'Kendall' },
      ],
    },
    CI,
  ],
  linear_regression: [
    { type: 'columns', path: 'datasets', label: 'Data sets', help: 'Which Y data sets to fit. None picked means all of them.', unit: 'dataset', emptyAll: true },
    { type: 'bool', path: 'through_origin', label: 'Through the origin', help: 'Forces the line through (0, 0). Only when theory demands it.' },
    {
      type: 'enum',
      path: 'bands',
      label: 'Bands',
      help: 'The confidence band shows where the true line lies; the prediction band shows where new points will fall.',
      control: 'seg',
      choices: [
        { value: 'none', label: 'None' },
        { value: 'confidence', label: 'Confidence' },
        { value: 'prediction', label: 'Prediction' },
        { value: 'both', label: 'Both' },
      ],
    },
    CI,
  ],
  nonlinear_regression: [
    { type: 'columns', path: 'datasets', label: 'Data sets', help: 'Which Y data sets to fit. None picked means all of them.', unit: 'dataset', emptyAll: true },
    {
      type: 'text',
      path: 'equation.name',
      label: 'Model',
      help: 'A built-in model by name, e.g. four_parameter_logistic (4PL), 3PL, 5PL, line or MM.',
      suggestions: ['four_parameter_logistic', '3PL', '5PL', 'line', 'MM'],
      mono: true,
    },
    {
      type: 'text',
      path: 'equation.formula',
      label: 'Own model',
      help: 'Or write Y = … of X and named parameters in the formula language; it replaces the model above.',
      nullable: 'None',
      mono: true,
    },
    {
      type: 'enum',
      path: 'weighting',
      label: 'Weighting',
      help: 'Weights give less influence to points with more scatter; equal weights suit constant scatter.',
      control: 'select',
      choices: [
        { value: 'none', label: 'Equal weights', hint: 'Recommended unless the scatter grows with Y.' },
        { value: 'relative_y2', label: '1/Y²', hint: 'Scatter proportional to Y.' },
        { value: 'poisson_y', label: '1/Y', hint: 'Poisson (counting) scatter.' },
        { value: 'inverse_sd2', label: '1/SD²', hint: 'From the replicates.' },
      ],
    },
    { type: 'bool', path: 'robust', label: 'Robust fit', help: 'Down-weights outliers instead of letting them pull the curve.' },
  ],
  normality: [
    COLUMNS_ALL(),
    {
      type: 'multiEnum',
      path: 'tests',
      label: 'Tests',
      help: 'Shapiro–Wilk is the most powerful for small samples; D’Agostino–Pearson looks at skew and kurtosis.',
      choices: [
        { value: 'shapiro_wilk', label: 'Shapiro–Wilk' },
        { value: 'dagostino_pearson', label: 'D’Agostino–Pearson' },
        { value: 'anderson_darling', label: 'Anderson–Darling' },
        { value: 'kolmogorov_smirnov', label: 'Kolmogorov–Smirnov' },
      ],
    },
  ],
  outliers: [
    COLUMNS_ALL(),
    {
      type: 'variant',
      path: 'method',
      tag: 'method',
      label: 'Method',
      help: 'ROUT finds any number of outliers with a false discovery rate Q; Grubbs finds one at a time.',
      variants: [
        {
          value: 'rout',
          label: 'ROUT',
          hint: 'Recommended; Q = 1% is the usual choice.',
          defaults: { q: 1 },
          fields: [{ type: 'number', path: 'q', label: 'Q (%)', help: 'The maximum false discovery rate in percent.', min: 0.0001, max: 10, step: 0.5 }],
        },
        {
          value: 'grubbs',
          label: 'Grubbs',
          hint: 'For a single outlier in Gaussian data.',
          defaults: { alpha: 0.05 },
          fields: [{ type: 'number', path: 'alpha', label: 'Alpha', help: 'Significance level of each Grubbs test.', min: 0.0001, max: 0.5, step: 0.01 }],
        },
      ],
    },
  ],
  survival: [
    {
      type: 'enum',
      path: 'compare',
      label: 'Compare curves',
      help: 'The log-rank test weighs all times equally; Gehan–Breslow–Wilcoxon weighs early deaths more.',
      control: 'seg',
      choices: [
        { value: 'log_rank', label: 'Log-rank', hint: 'Recommended.' },
        { value: 'gehan_breslow_wilcoxon', label: 'Gehan–Breslow–Wilcoxon' },
      ],
    },
    CI,
  ],
  roc: [
    TWO_COLUMNS('Controls, patients', 'The first data set holds the controls, the second the patients.'),
    { type: 'text', path: 'positive_label', label: 'Positive label', help: 'In a variables table, the category that marks a patient.', nullable: 'Not used' },
    { ...CI, help: 'Width of the interval around the area under the curve.' } as Field,
  ],
  bland_altman: [
    TWO_COLUMNS('Methods A, B', 'The two methods measured on the same subjects, one per row.'),
    {
      type: 'enum',
      path: 'method',
      label: 'Difference',
      help: 'Plot A − B, the ratio A / B, or the difference as a percentage of the mean.',
      control: 'seg',
      choices: [
        { value: 'difference', label: 'A − B' },
        { value: 'ratio', label: 'A / B' },
        { value: 'percent_difference', label: '% difference' },
      ],
    },
    { ...CI, help: 'Width of the intervals around the bias and the limits of agreement.' } as Field,
  ],
  pca: [
    COLUMNS_ALL('Variables'),
    { type: 'bool', path: 'standardize', label: 'Standardize', help: 'Scales every variable to unit variance (correlation matrix). Recommended when units differ.' },
    {
      type: 'number',
      path: 'components',
      label: 'Components',
      help: 'How many components to keep; empty chooses by parallel analysis.',
      integer: true,
      min: 1,
      max: 50,
      nullable: 'By parallel analysis',
    },
  ],
};

export function fieldsFor(kind: string): Field[] {
  return SCHEMA[kind] ?? [];
}

// ---- spec access ---------------------------------------------------------------------------

export function getAt(obj: JsonValue | undefined, path: string): JsonValue | undefined {
  let cur: JsonValue | undefined = obj;
  for (const k of path.split('.')) {
    if (cur === null || typeof cur !== 'object' || Array.isArray(cur)) return undefined;
    cur = (cur as JsonObject)[k];
  }
  return cur;
}

/** A copy of `obj` with `value` at `path` (creating objects on the way). */
export function setAt<T extends JsonObject>(obj: T, path: string, value: JsonValue): T {
  const [head, ...rest] = path.split('.');
  const key = head!;
  if (!rest.length) return { ...obj, [key]: value };
  const child = obj[key];
  const base = child && typeof child === 'object' && !Array.isArray(child) ? (child as JsonObject) : {};
  return { ...obj, [key]: setAt(base, rest.join('.'), value) };
}

export interface FormRow {
  field: Field;
  value: JsonValue | undefined;
}

/** The rows the options panel shows for a spec, with their current values. */
export function formModel(spec: Spec): FormRow[] {
  return fieldsFor(spec.kind)
    .filter((f) => !f.when || f.when(spec))
    .map((field) => ({ field, value: getAt(spec, field.path) }));
}

// ---- data sets ↔ flat columns ---------------------------------------------------------------

/** Data set indices that a stored `columns`/`datasets` value refers to, in order. */
export function datasetsOf(meta: TableMeta | undefined, value: JsonValue | undefined, unit: 'first' | 'all' | 'dataset'): number[] {
  if (!Array.isArray(value)) return [];
  const nums = value.filter((v): v is number => typeof v === 'number');
  if (unit === 'dataset') return nums;
  const out: number[] = [];
  for (const c of nums) {
    const ds = meta?.columns[c]?.dataset;
    if (ds !== null && ds !== undefined && !out.includes(ds)) out.push(ds);
  }
  return out;
}

/** The stored value for picked data sets. */
export function columnsOf(meta: TableMeta | undefined, datasets: number[], unit: 'first' | 'all' | 'dataset'): number[] {
  if (unit === 'dataset' || !meta) return [...datasets];
  const out: number[] = [];
  for (const d of datasets) {
    const ds = meta.datasets[d];
    if (!ds) continue;
    const n = unit === 'all' ? ds.subcolumns : 1;
    for (let i = 0; i < n; i++) out.push(ds.first_col + i);
  }
  return out;
}

/** Problems that keep a spec from running, in plain sentences. */
export function validate(spec: Spec, meta: TableMeta | undefined): string[] {
  const errs: string[] = [];
  for (const { field, value } of formModel(spec)) {
    if (field.type === 'columns' && field.count) {
      const [lo, hi] = field.count(spec);
      const n = datasetsOf(meta, value, field.unit).length;
      if (n < lo || n > hi) {
        errs.push(
          lo === hi
            ? `${field.label}: pick ${lo === 1 ? 'one data set' : `${lo} data sets`}.`
            : `${field.label}: pick ${lo} to ${hi} data sets.`,
        );
      }
    }
    if (field.type === 'number' && value !== null && value !== undefined) {
      const v = typeof value === 'number' ? value : NaN;
      const shown = field.percent ? v * 100 : v;
      if (!Number.isFinite(v)) errs.push(`${field.label}: enter a number.`);
      else if ((field.min !== undefined && shown < field.min) || (field.max !== undefined && shown > field.max)) {
        errs.push(`${field.label}: between ${field.min ?? '−∞'} and ${field.max ?? '∞'}.`);
      }
    }
    if (field.type === 'multiEnum' && Array.isArray(value) && value.length === 0) {
      errs.push(`${field.label}: pick at least one.`);
    }
  }
  return errs;
}

/**
 * Narrow a fresh default spec to the grid selection: when whole data sets are selected,
 * `columns` (or `datasets`) lists them.
 */
export function withSelection(spec: Spec, meta: TableMeta | undefined, selected: number[]): Spec {
  if (!meta || selected.length === 0) return spec;
  let out = spec;
  for (const f of fieldsFor(spec.kind)) {
    if (f.type !== 'columns') continue;
    let pick = selected;
    if (f.count) {
      const [, hi] = f.count(spec);
      if (pick.length < f.count(spec)[0]) continue;
      pick = pick.slice(0, hi);
    }
    out = setAt(out, f.path, columnsOf(meta, pick, f.unit));
  }
  return out;
}
