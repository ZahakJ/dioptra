// The Analyze dialog: the list of analyses for the table and selection (fitting ones first,
// each with one sentence; the others greyed with the reason), then the options panel.
import { useMemo, useRef, useState } from 'react';
import { Search } from 'lucide-react';
import { DialogHeader } from '../app/dialogs';
import { Button } from '../ui';
import type { AnalysisOption, TableMeta } from '../engine/types';
import type { Spec } from '../engine/objects';
import { describe } from './descriptions';
import { OptionsPanel } from './OptionsPanel';
import { validate } from './schema';

export type AnalyzeResult = { spec: Spec } | null;

export function AnalyzeDialog({
  options,
  meta,
  scope,
  loadSpec,
  close,
}: {
  options: AnalysisOption[];
  meta: TableMeta | undefined;
  /** "Table 1" or "Table 1, data sets A to C". */
  scope: string;
  loadSpec: (kind: string) => Promise<Spec | undefined>;
  close: (r: AnalyzeResult) => void;
}) {
  const [spec, setSpec] = useState<Spec | null>(null);
  const [filter, setFilter] = useState('');
  const [active, setActive] = useState(0);
  const [loading, setLoading] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);
  const sorted = useMemo(() => {
    const q = filter.trim().toLowerCase();
    const match = (o: AnalysisOption) => !q || o.name.toLowerCase().includes(q) || o.kind.includes(q) || describe(o.kind).toLowerCase().includes(q);
    return [...options.filter((o) => o.fits && match(o)), ...options.filter((o) => !o.fits && match(o))];
  }, [options, filter]);
  const fitting = sorted.filter((o) => o.fits);

  const choose = async (o: AnalysisOption | undefined) => {
    if (!o || !o.fits || loading) return;
    setLoading(true);
    const s = await loadSpec(o.kind);
    setLoading(false);
    if (s) setSpec(s);
  };

  if (spec) {
    return <OptionsStep spec={spec} meta={meta} scope={scope} onSpec={setSpec} back={() => setSpec(null)} close={close} confirm="Run" />;
  }

  return (
    <div className="modal-body analyze" data-testid="analyze-dialog">
      <DialogHeader title="Analyze" sub={scope} onClose={() => close(null)} />
      <label className="search-field">
        <Search size={16} strokeWidth={1.5} aria-hidden="true" />
          <input
          className="input"
          placeholder="Filter analyses"
          aria-label="Filter analyses"
          value={filter}
          data-autofocus
          data-testid="analyze-filter"
          onChange={(e) => {
            setFilter(e.target.value);
            setActive(0);
          }}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown') {
              e.preventDefault();
              setActive((a) => Math.min(a + 1, fitting.length - 1));
            } else if (e.key === 'ArrowUp') {
              e.preventDefault();
              setActive((a) => Math.max(a - 1, 0));
            } else if (e.key === 'Enter') {
              e.preventDefault();
              void choose(fitting[active]);
            }
          }}
        />
      </label>
      <div className="an-list scroll-area" role="listbox" aria-label="Analyses" ref={listRef}>
        {sorted.map((o) => {
          const i = fitting.indexOf(o);
          return (
            <button
              key={o.kind}
              type="button"
              role="option"
              aria-selected={o.fits && i === active}
              aria-disabled={!o.fits}
              className={'an-item' + (o.fits ? '' : ' is-unfit') + (o.fits && i === active ? ' is-active' : '')}
              data-testid={`analysis-${o.kind}`}
              onMouseEnter={() => o.fits && setActive(i)}
              onClick={() => void choose(o)}
            >
              <span className="an-name">
                {o.name}
                {o.fits && !o.implemented && <span className="an-tag">not computed in this build yet</span>}
              </span>
              <span className="an-desc">{o.fits ? describe(o.kind) : o.reason}</span>
            </button>
          );
        })}
        {sorted.length === 0 && <p className="muted">No analysis matches “{filter}”.</p>}
      </div>
      <footer className="modal-foot">
        <span className="foot-note">{loading ? 'Loading options…' : `${fitting.length} of ${options.length} fit this table`}</span>
        <Button onClick={() => close(null)}>Cancel</Button>
      </footer>
    </div>
  );
}

export function OptionsStep({
  spec,
  meta,
  scope,
  onSpec,
  back,
  close,
  confirm,
  name,
}: {
  spec: Spec;
  meta: TableMeta | undefined;
  scope: string;
  onSpec: (s: Spec) => void;
  back?: () => void;
  close: (r: AnalyzeResult) => void;
  confirm: string;
  name?: string;
}) {
  const errors = validate(spec, meta);
  return (
    <form
      className="modal-body analyze-options"
      data-testid="analysis-options"
      onSubmit={(e) => {
        e.preventDefault();
        if (!errors.length) close({ spec });
      }}
    >
      <DialogHeader title={name ?? kindTitle(spec.kind)} sub={`${describe(spec.kind)} ${scope}.`} onClose={() => close(null)} />
      <OptionsPanel spec={spec} meta={meta} onChange={onSpec} />
      {errors.length > 0 && (
        <ul className="opt-errors" data-testid="options-errors">
          {errors.map((e) => (
            <li key={e}>{e}</li>
          ))}
        </ul>
      )}
      <footer className="modal-foot">
        {back && (
          <Button variant="quiet" className="foot-left" onClick={back}>
            Back to the list
          </Button>
        )}
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={errors.length > 0} testId="analysis-run" data-autofocus>
          {confirm}
        </Button>
      </footer>
    </form>
  );
}

const TITLES: Record<string, string> = {
  descriptive: 'Descriptive statistics',
  t_test: 't test',
  mann_whitney: 'Mann–Whitney test',
  wilcoxon: 'Wilcoxon signed-rank test',
  one_way_anova: 'One-way ANOVA',
  repeated_measures_anova: 'Repeated-measures one-way ANOVA',
  two_way_anova: 'Two-way ANOVA',
  kruskal_wallis: 'Kruskal–Wallis test',
  friedman: 'Friedman test',
  chi_square: 'Chi-square test',
  fisher_exact: 'Fisher’s exact test',
  correlation: 'Correlation',
  linear_regression: 'Simple linear regression',
  nonlinear_regression: 'Nonlinear regression (curve fit)',
  normality: 'Normality tests',
  outliers: 'Identify outliers',
  survival: 'Survival curves (Kaplan–Meier)',
  roc: 'ROC curve',
  bland_altman: 'Bland–Altman',
  pca: 'Principal component analysis',
};

export function kindTitle(kind: string): string {
  return TITLES[kind] ?? kind.replace(/_/g, ' ');
}
