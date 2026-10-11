// Build the bundled sample project, apps/dioptra/public/samples/cytokine-study.dioptra, with the
// real engine (the wasm build from scripts/build-wasm.sh) running in Node. The data are the
// Cytokine study the documentation and the captures use: a one-way design (plasma IL-6 in four
// dose groups), a dose-response experiment (three compounds in triplicate) and a two-arm
// survival study, each with its analysis and graph.
//
//   ./scripts/build-wasm.sh && node scripts/make-sample.mjs
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pkg = resolve(root, 'apps/dioptra/src/engine/wasm/pkg');
const mod = await import(resolve(pkg, 'dioptra_wasm.js'));
mod.initSync({ module: readFileSync(resolve(pkg, 'dioptra_wasm_bg.wasm')) });
const engine = new mod.Engine();

function d(cmd) {
  const r = JSON.parse(engine.dispatch(JSON.stringify(cmd)));
  if (r && r.type === 'error') throw new Error(`${r.message} in ${JSON.stringify(cmd).slice(0, 200)}`);
  return r;
}
const created = (r) => r.changed?.created ?? r.changed?.ids?.[0];

// ---- data -------------------------------------------------------------------------------------

const IL6 = {
  names: ['Vehicle', 'Low dose', 'Mid dose', 'High dose'],
  rows: [
    [12.4, 13.1, 19.6, 28.9],
    [10.8, 11.9, 17.4, 31.4],
    [14.1, 14.6, 21.9, 26.7],
    [11.9, 12.2, 20.8, 33.2],
    [13.2, 13.8, 18.6, 29.8],
    [9.7, 12.5, 22.5, 27.5],
    [12.8, 11.4, 16.9, 30.6],
    [11.5, 14.0, 20.2, 32.1],
  ],
};

// Deterministic noise, so the sample is the same on every build.
let seed = 20261011;
function rnd() {
  seed = (seed * 1103515245 + 12345) % 2147483648;
  return seed / 2147483648;
}
function gauss() {
  let u = 0;
  for (let i = 0; i < 6; i++) u += rnd();
  return (u - 3) / Math.sqrt(0.5);
}

const DOSES = [-10, -9.5, -9, -8.5, -8, -7.5, -7, -6.5, -6, -5.5];
const COMPOUNDS = [
  { name: 'Compound A', logec50: -8.3, hill: 1.1, top: 98, bottom: 2 },
  { name: 'Compound B', logec50: -7.6, hill: 0.9, top: 95, bottom: 4 },
  { name: 'Compound C', logec50: -6.9, hill: 1.3, top: 88, bottom: 3 },
];
const doseRows = () =>
  DOSES.map((x) => {
    const row = [x.toFixed(1)];
    for (const c of COMPOUNDS) {
      const y = c.bottom + (c.top - c.bottom) / (1 + 10 ** ((c.logec50 - x) * c.hill));
      for (let r = 0; r < 3; r++) row.push((y + 2.6 * gauss()).toFixed(1));
    }
    return row.join('\t');
  }).join('\n');

// Months to death (1) or censoring (0), two arms.
const SURV = {
  'Standard care': [
    [2.1, 1], [3.4, 1], [4.0, 1], [4.8, 0], [5.5, 1], [6.3, 1], [7.1, 1], [7.9, 0], [8.6, 1], [9.4, 1],
    [10.2, 1], [11.0, 0], [12.5, 1], [13.8, 1], [15.1, 0], [16.4, 1], [18.2, 1], [20.5, 0], [22.9, 1], [26.0, 0],
    [29.3, 1], [33.0, 0], [36.0, 0], [36.0, 0],
  ],
  Treatment: [
    [3.2, 1], [5.1, 0], [6.8, 1], [8.4, 1], [9.9, 0], [11.7, 1], [13.2, 1], [14.6, 0], [16.9, 1], [18.3, 0],
    [19.8, 1], [21.4, 0], [23.7, 1], [25.2, 0], [27.9, 1], [29.5, 0], [31.1, 0], [32.8, 1], [34.0, 0], [36.0, 0],
    [36.0, 0], [36.0, 0], [36.0, 0], [36.0, 0],
  ],
};
const survivalRows = () =>
  [...SURV['Standard care'].map(([t, e]) => `${t}\t${e}\t`), ...SURV.Treatment.map(([t, e]) => `${t}\t\t${e}`)].join('\n');

function analysis(table, kind) {
  const spec = d({ type: 'default_analysis_spec', table, kind });
  delete spec.type;
  return created(d({ type: 'create_analysis', table, spec }));
}

// ---- build ------------------------------------------------------------------------------------

d({ type: 'new_project' });
d({ type: 'set_project_title', title: 'Cytokine study' });

const t1 = created(d({ type: 'create_table', kind: 'column', title: 'Plasma IL-6', datasets: 4, format: { kind: 'values', n: 1 }, rows: 8 }));
d({
  type: 'paste',
  table: t1,
  row0: 0,
  col0: 0,
  text: [IL6.names.join('\t'), ...IL6.rows.map((r) => r.map((v) => v.toFixed(1)).join('\t'))].join('\n'),
  options: { header: true },
});
for (let c = 0; c < 4; c++) d({ type: 'set_column_decimals', table: t1, col: c, decimals: 1 });
const a1 = analysis(t1, 'one_way_anova');
// Brackets in GraphPad asterisks, as most figures show them; the sheets stay in APA style.
d({ type: 'set_p_value_style', style: 'gp' });
const g1 = created(d({ type: 'create_graph', source: a1, family: 'auto', title: 'IL-6 by dose' }));
d({ type: 'set_p_value_style', style: 'apa' });
d({ type: 'patch_graph', id: g1, patch: { axes: { y: { title: 'IL-6 (pg/mL)' } } } });

const t2 = created(d({ type: 'create_table', kind: 'xy', title: 'Dose response', datasets: 3, format: { kind: 'values', n: 3 }, rows: DOSES.length }));
d({ type: 'paste', table: t2, row0: 0, col0: 0, text: doseRows(), options: { header: false } });
COMPOUNDS.forEach((c, i) => d({ type: 'set_dataset_name', table: t2, dataset: i, name: c.name }));
const a2 = analysis(t2, 'nonlinear_regression');
const g2 = created(d({ type: 'create_graph', source: a2, family: 'auto', title: 'Dose-response curves' }));
d({
  type: 'patch_graph',
  id: g2,
  patch: { axes: { x: { title: 'log[agonist] (M)' }, y: { title: 'Response (% of maximum)' } }, style: { size_mm: { width: 120, height: 80 } } },
});

const t3 = created(d({ type: 'create_table', kind: 'survival', title: 'Overall survival', datasets: 2, format: { kind: 'single' }, rows: 0 }));
d({ type: 'paste', table: t3, row0: 0, col0: 0, text: survivalRows(), options: { header: false } });
d({ type: 'set_dataset_name', table: t3, dataset: 0, name: 'Standard care' });
d({ type: 'set_dataset_name', table: t3, dataset: 1, name: 'Treatment' });
const a3 = analysis(t3, 'survival');
const g3 = created(d({ type: 'create_graph', source: a3, family: 'auto', title: 'Kaplan-Meier curves' }));
d({
  type: 'patch_graph',
  id: g3,
  patch: {
    axes: { x: { title: 'Months since randomisation', ticks: { major: 6 } } },
    style: { size_mm: { width: 120, height: 90 } },
    options: { ci_band: false },
  },
});

d({ type: 'recompute_all' });
for (const g of [g1, g2, g3]) d({ type: 'render_graph', id: g });
const bytes = d({ type: 'save_project_bytes' });
const out = resolve(root, 'apps/dioptra/public/samples/cytokine-study.dioptra');
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, Buffer.from(bytes.base64, 'base64'));
console.log(`wrote ${out.slice(root.length + 1)} (${Buffer.from(bytes.base64, 'base64').length} bytes)`);
