// One sentence per analysis kind for the Analyze list: what it tests and when to use it.
export const DESCRIPTIONS: Record<string, string> = {
  descriptive: 'Mean, SD, SEM, confidence interval, median and quartiles of each data set. The place to start.',
  t_test: 'Do two group means differ? For two groups of roughly Gaussian values, paired or unpaired.',
  mann_whitney: 'Do two unpaired groups differ in location? The rank-based alternative to the unpaired t test.',
  wilcoxon: 'Do paired values differ, or does one group’s median differ from a value? The rank-based paired test.',
  one_way_anova: 'Do three or more group means differ? With post-hoc comparisons between the groups.',
  repeated_measures_anova: 'Do three or more treatments differ when each subject (row) received all of them?',
  two_way_anova: 'How do two factors (rows and data sets) and their interaction affect the mean?',
  kruskal_wallis: 'Do three or more unpaired groups differ? The rank-based alternative to one-way ANOVA.',
  friedman: 'Do three or more matched treatments differ? The rank-based alternative to repeated-measures ANOVA.',
  chi_square: 'Are the rows and columns of a table of counts associated?',
  fisher_exact: 'Exact test of association in a 2 × 2 table of counts; right even when counts are small.',
  correlation: 'How strongly do two variables move together? Pearson, Spearman or Kendall coefficients.',
  linear_regression: 'The best straight line through XY data, with its slope, intercept and their uncertainty.',
  nonlinear_regression: 'Fit a curve (dose–response, decay, binding) to XY data and estimate its parameters.',
  normality: 'Could each data set have come from a Gaussian distribution?',
  outliers: 'Which values are unlikely under the rest of the data? ROUT or Grubbs.',
  survival: 'Kaplan–Meier curves with median survival, and a test of whether curves differ.',
  roc: 'How well does a measurement separate patients from controls? The area under the ROC curve.',
  bland_altman: 'Do two measurement methods agree? Bias and 95% limits of agreement.',
  pca: 'Summarize many correlated variables with a few principal components.',
};

export function describe(kind: string): string {
  return DESCRIPTIONS[kind] ?? '';
}
