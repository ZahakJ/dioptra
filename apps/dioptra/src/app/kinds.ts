// The eight table kinds, explained in one sentence each with a three-line sketch (DESIGN §4.1,
// complaint P9).
import type { SubcolumnFormat, TableKind } from '../engine/types';

export interface KindInfo {
  kind: TableKind;
  name: string;
  sentence: string;
  /** Three rows of a tiny table: header row then two data rows. */
  sketch: [string[], string[], string[]];
  /** Whether the user chooses replicates / a subcolumn format. */
  formats: boolean;
  defaultDatasets: number;
  defaultFormat: SubcolumnFormat;
  datasetWord: string;
}

export const KINDS: KindInfo[] = [
  {
    kind: 'column',
    name: 'Column',
    sentence: 'Groups side by side, one column each, for t tests, one-way ANOVA and descriptive statistics.',
    sketch: [['Control', 'Treated'], ['4.1', '5.3'], ['3.8', '6.0']],
    formats: false,
    defaultDatasets: 3,
    defaultFormat: { kind: 'values', n: 1 },
    datasetWord: 'Groups',
  },
  {
    kind: 'xy',
    name: 'XY',
    sentence: 'An X column and one or more Y data sets, for curves, regression and dose-response fits.',
    sketch: [['X', 'Y1', 'Y2'], ['0.1', '2.3', '2.1'], ['1.0', '4.0', '3.8']],
    formats: true,
    defaultDatasets: 2,
    defaultFormat: { kind: 'values', n: 1 },
    datasetWord: 'Y data sets',
  },
  {
    kind: 'grouped',
    name: 'Grouped',
    sentence: 'Rows are levels of one factor and data sets are levels of another, for two-way ANOVA.',
    sketch: [['', 'Male', 'Female'], ['WT', '12', '14'], ['KO', '9', '8']],
    formats: true,
    defaultDatasets: 2,
    defaultFormat: { kind: 'values', n: 3 },
    datasetWord: 'Column groups',
  },
  {
    kind: 'contingency',
    name: 'Contingency',
    sentence: 'Counts of subjects in each combination of two categories, for chi-square and Fisher tests.',
    sketch: [['', 'Cured', 'Not'], ['Drug', '30', '10'], ['Placebo', '18', '22']],
    formats: false,
    defaultDatasets: 2,
    defaultFormat: { kind: 'single' },
    datasetWord: 'Outcomes',
  },
  {
    kind: 'survival',
    name: 'Survival',
    sentence: 'Time in X and, per group, 1 for an event or 0 for censored, for Kaplan-Meier curves.',
    sketch: [['Days', 'Group A', 'Group B'], ['12', '1', ''], ['30', '', '0']],
    formats: false,
    defaultDatasets: 2,
    defaultFormat: { kind: 'single' },
    datasetWord: 'Groups',
  },
  {
    kind: 'parts',
    name: 'Parts of whole',
    sentence: 'One data set of categories that add up to a whole, for fractions and proportions.',
    sketch: [['', 'Count'], ['Type I', '42'], ['Type II', '17']],
    formats: false,
    defaultDatasets: 1,
    defaultFormat: { kind: 'single' },
    datasetWord: 'Data sets',
  },
  {
    kind: 'variables',
    name: 'Multiple variables',
    sentence: 'One column per variable and one row per case, like a data frame; the safe default.',
    sketch: [['Age', 'Sex', 'BMI'], ['54', 'F', '23.1'], ['61', 'M', '27.4']],
    formats: false,
    defaultDatasets: 3,
    defaultFormat: { kind: 'single' },
    datasetWord: 'Variables',
  },
  {
    kind: 'nested',
    name: 'Nested',
    sentence: 'Groups with subgroups, such as animals within treatments, each holding repeated observations.',
    sketch: [['Ctrl:m1', 'Ctrl:m2', 'Drug:m1'], ['3.2', '2.9', '4.4'], ['3.5', '3.1', '4.0']],
    formats: true,
    defaultDatasets: 2,
    defaultFormat: { kind: 'values', n: 3 },
    datasetWord: 'Groups',
  },
];

export function kindInfo(kind: string | undefined): KindInfo | undefined {
  return KINDS.find((k) => k.kind === kind);
}
