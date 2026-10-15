// File → Batch analysis… and Watch a folder… (Dioptra Lab): a template project, the table that
// receives each file, the files (or the folder), the outputs; then progress and the summary.
import { useEffect, useMemo, useState } from 'react';
import { DialogHeader, openDialog } from '../app/dialogs';
import { useLab } from '../store/lab';
import { notifyError } from '../store/notices';
import { useProject } from '../store/project';
import { useWatch } from '../store/watch';
import { Button, Field, RadioList, Segmented, Select, TextArea } from '../ui';
import {
  currentProjectTemplate,
  listFolder,
  pickDataPaths,
  pickFiles,
  pickFolder,
  readTemplatePath,
  runInEngine,
  runOnDisk,
  templateInfo,
  writeSummary,
  zipResults,
  type BatchOptions,
  type FigureFormat,
  type Template,
  type TemplateInfo,
} from './batch';
import { BATCH_EXTENSIONS, fileName, formatSummaryCell, pColumns, summarise, type BatchItem } from './batchSummary';
import { desktopOpenPath, downloadBytes, fileToBase64, isDesktop, pickFile, PROJECT_FILTER } from './files';
import './batch.css';

type Mode = 'batch' | 'watch';
type Input = { name: string; path?: string; file?: File };

export function openBatchDialog(mode: Mode): Promise<void> {
  return openDialog<void>((close) => <BatchDialog mode={mode} close={close} />, undefined);
}

function LabOff({ mode, close }: { mode: Mode; close: () => void }) {
  return (
    <div className="modal-body modal-narrow" data-testid="batch-lab-off">
      <DialogHeader title={mode === 'watch' ? 'Watch a folder' : 'Batch analysis'} onClose={close} />
      <p className="modal-text">
        Batch analysis is part of Dioptra Lab. It applies a project, with its analyses and graphs, to many data files or to each new file in a
        folder, and collects the results in one summary table.
      </p>
      <p className="modal-text muted">Importing a single file, plate-reader exports included, and replacing a table’s data by hand are free.</p>
      <footer className="modal-foot">
        <Button onClick={close}>Close</Button>
        <Button
          variant="primary"
          testId="batch-open-lab"
          onClick={() => {
            close();
            useLab.getState().setDialogOpen(true);
          }}
        >
          Dioptra Lab…
        </Button>
      </footer>
    </div>
  );
}

function BatchDialog({ mode, close }: { mode: Mode; close: () => void }) {
  const enabled = useLab((s) => s.view.lab_enabled);
  if (!enabled) return <LabOff mode={mode} close={close} />;
  return <BatchForm mode={mode} close={close} />;
}

function BatchForm({ mode, close }: { mode: Mode; close: () => void }) {
  const desktop = isDesktop();
  const hasTables = useProject((s) => !!s.view?.items.some((i) => i.kind === 'table'));
  const [source, setSource] = useState<'project' | 'file'>(hasTables ? 'project' : 'file');
  const [template, setTemplate] = useState<Template | null>(null);
  const [info, setInfo] = useState<TemplateInfo | null>(null);
  const [table, setTable] = useState('');
  const [inputs, setInputs] = useState<Input[]>([]);
  const [outDir, setOutDir] = useState<string | null>(null);
  const [watchDir, setWatchDir] = useState<string | null>(null);
  const [format, setFormat] = useState<FigureFormat>('pdf');
  const [plateMap, setPlateMap] = useState('');
  const [progress, setProgress] = useState<{ done: number; total: number; current: string } | null>(null);
  const [items, setItems] = useState<BatchItem[] | null>(null);
  const [summaryPath, setSummaryPath] = useState<string | null>(null);
  const watching = useWatch((s) => s.dir);
  const watchItems = useWatch((s) => s.items);

  // The template of the open project is read when the dialog opens.
  useEffect(() => {
    if (source !== 'project') return;
    let live = true;
    currentProjectTemplate()
      .then((t) => live && setTemplate(t))
      .catch(notifyError);
    return () => {
      live = false;
    };
  }, [source]);

  useEffect(() => {
    setInfo(null);
    if (!template) return;
    let live = true;
    templateInfo(template.base64)
      .then((i) => {
        if (!live) return;
        setInfo(i);
        setTable((t) => (i.tables.includes(t) ? t : (i.tables[0] ?? '')));
      })
      .catch(notifyError);
    return () => {
      live = false;
    };
  }, [template]);

  const chooseTemplate = async () => {
    try {
      if (desktop) {
        const path = await desktopOpenPath([PROJECT_FILTER]);
        if (path) setTemplate({ label: fileName(path), base64: await readTemplatePath(path) });
      } else {
        const f = await pickFile('.dioptra');
        if (f) setTemplate({ label: f.name, base64: await fileToBase64(f) });
      }
    } catch (err) {
      notifyError(err);
    }
  };

  const addFiles = async () => {
    try {
      if (desktop) {
        const paths = await pickDataPaths();
        if (paths.length) setInputs((v) => [...v, ...paths.map((p) => ({ name: fileName(p), path: p }))]);
      } else {
        const files = await pickFiles(BATCH_EXTENSIONS.map((e) => `.${e}`).join(','));
        if (files.length) setInputs((v) => [...v, ...files.map((f) => ({ name: f.name, file: f }))]);
      }
    } catch (err) {
      notifyError(err);
    }
  };

  const addFolder = async () => {
    try {
      const dir = await pickFolder('Folder of data files');
      if (!dir) return;
      const paths = await listFolder(dir);
      setInputs((v) => [...v, ...paths.map((p) => ({ name: fileName(p), path: p }))]);
      setOutDir((o) => o ?? `${dir}${dir.includes('\\') ? '\\' : '/'}analysed`);
    } catch (err) {
      notifyError(err);
    }
  };

  const options: BatchOptions = { format, ...(plateMap.trim() ? { plate_map: plateMap } : {}) };
  const ready = !!template && !!table && (mode === 'watch' ? !!watchDir : inputs.length > 0 && (!desktop || !!outDir));

  const run = async () => {
    if (!template || !ready) return;
    const done: BatchItem[] = [];
    setItems(null);
    setSummaryPath(null);
    try {
      for (const [k, input] of inputs.entries()) {
        setProgress({ done: k, total: inputs.length, current: input.name });
        const item = desktop && input.path ? await runOnDisk(template, table, input.path, outDir!, options) : await runInEngine(template, table, input.file!, options);
        done.push(item);
        setItems([...done]);
      }
      setProgress({ done: inputs.length, total: inputs.length, current: '' });
      if (desktop) setSummaryPath(await writeSummary(outDir!, done));
      else {
        const z = await zipResults(done);
        downloadBytes(z.name, z.mime, z.base64);
      }
    } catch (err) {
      notifyError(err);
    } finally {
      setProgress(null);
    }
  };

  const startWatching = async () => {
    if (!template || !watchDir) return;
    try {
      await useWatch.getState().start(watchDir, template, table, options);
    } catch (err) {
      notifyError(err);
    }
  };

  const shown = mode === 'watch' ? watchItems : items;
  const running = progress !== null;

  return (
    <form
      className="modal-body modal-wide batch-dialog"
      data-testid="batch-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        if (mode === 'watch') void startWatching();
        else void run();
      }}
    >
      <DialogHeader
        title={mode === 'watch' ? 'Watch a folder' : 'Batch analysis'}
        sub={
          mode === 'watch'
            ? 'Each new data file in the folder is analysed with the template once it has finished writing. Results go to its analysed subfolder.'
            : 'Each data file goes into a copy of the template; every analysis, graph and layout is recomputed.'
        }
        onClose={close}
      />
      <div className="batch-grid">
        <div className="batch-col">
          <Field label="Template">
            {() => (
              <div className="batch-template">
                <RadioList
                  label="Template"
                  value={source}
                  onChange={(v) => {
                    setSource(v);
                    setTemplate(null);
                  }}
                  choices={[
                    ...(hasTables ? [{ value: 'project' as const, label: 'The open project', testId: 'batch-template-project' }] : []),
                    { value: 'file' as const, label: 'A project file', hint: source === 'file' && template ? template.label : undefined, testId: 'batch-template-file' },
                  ]}
                />
                {source === 'file' && (
                  <Button size="sm" onClick={() => void chooseTemplate()} testId="batch-choose-template">
                    Choose project…
                  </Button>
                )}
              </div>
            )}
          </Field>
          <Field label="Table that receives the data" hint={info ? `${info.analyses} analyses and ${info.figures} figures are recomputed for each file.` : undefined}>
            {(lid) => (
              <Select
                labelledBy={lid}
                size="md"
                value={table}
                disabled={!info || info.tables.length === 0}
                placeholder={template ? 'No data tables' : 'Choose a template first'}
                options={(info?.tables ?? []).map((t) => ({ value: t, label: t }))}
                onChange={setTable}
                testId="batch-table"
              />
            )}
          </Field>
          <Field label="Figures">
            {(lid) => (
              <Segmented
                labelledBy={lid}
                value={format}
                onChange={setFormat}
                choices={[
                  { value: 'pdf' as FigureFormat, label: 'PDF' },
                  { value: 'png' as FigureFormat, label: 'PNG', hint: '300 dpi' },
                  { value: 'svg' as FigureFormat, label: 'SVG' },
                ]}
              />
            )}
          </Field>
        </div>
        <div className="batch-col">
          {mode === 'batch' ? (
            <>
              <Field label="Data files" hint={inputs.length ? undefined : 'CSV, TSV, text, Excel, OpenDocument, Prism and plate-reader exports.'}>
                {() => (
                  <div className="batch-inputs">
                    <div className="batch-actions">
                      <Button size="sm" onClick={() => void addFiles()} testId="batch-add-files">
                        Add files…
                      </Button>
                      {desktop && (
                        <Button size="sm" onClick={() => void addFolder()} testId="batch-add-folder">
                          Add a folder…
                        </Button>
                      )}
                      {inputs.length > 0 && (
                        <Button size="sm" variant="quiet" onClick={() => setInputs([])}>
                          Clear
                        </Button>
                      )}
                    </div>
                    {inputs.length > 0 && (
                      <ul className="batch-list" data-testid="batch-inputs">
                        {inputs.map((i, k) => (
                          <li key={k} className="mono">
                            {i.name}
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                )}
              </Field>
              {desktop ? (
                <Field label="Output folder">
                  {() => (
                    <div className="batch-actions">
                      <Button
                        size="sm"
                        onClick={() =>
                          void pickFolder('Output folder')
                            .then((d) => d && setOutDir(d))
                            .catch(notifyError)
                        }
                        testId="batch-out"
                      >
                        Choose folder…
                      </Button>
                      <span className="batch-path mono">{outDir ?? 'None chosen'}</span>
                    </div>
                  )}
                </Field>
              ) : (
                <p className="field-hint">The projects, figures and summary.csv download as one zip.</p>
              )}
            </>
          ) : (
            <Field label="Folder to watch">
              {() => (
                <div className="batch-actions">
                  <Button
                    size="sm"
                    onClick={() =>
                      void pickFolder('Folder to watch')
                        .then((d) => d && setWatchDir(d))
                        .catch(notifyError)
                    }
                    testId="batch-watch-dir"
                  >
                    Choose folder…
                  </Button>
                  <span className="batch-path mono">{watchDir ?? 'None chosen'}</span>
                </div>
              )}
            </Field>
          )}
          <Field label="Plate map" hint="Optional, for plate-reader files. Paste an 8 × 12 grid or well, label lines; a label is a group, a dose, or both as Drug A @ 10.">
            {(lid) => (
              <TextArea mono rows={3} aria-labelledby={lid} value={plateMap} placeholder="A1, Blank" onChange={(e) => setPlateMap(e.target.value)} testId="batch-plate-map" />
            )}
          </Field>
        </div>
      </div>
      {mode === 'watch' && watching && (
        <p className="batch-status" data-testid="batch-watching">
          Watching <span className="mono">{watching}</span>. {watchItems.length} {watchItems.length === 1 ? 'file' : 'files'} analysed.
        </p>
      )}
      {progress && (
        <div className="batch-progress" role="status" data-testid="batch-progress">
          <div className="batch-bar" aria-hidden="true">
            <div style={{ width: `${(100 * progress.done) / Math.max(1, progress.total)}%` }} />
          </div>
          <span>
            {progress.done} of {progress.total}
            {progress.current ? `: ${progress.current}` : ''}
          </span>
        </div>
      )}
      {shown && shown.length > 0 && <Results items={shown} />}
      {summaryPath && (
        <p className="field-hint">
          Summary written to <span className="mono">{summaryPath}</span>.
        </p>
      )}
      <footer className="modal-foot">
        {mode === 'watch' && watching && (
          <Button variant="quiet" onClick={() => void useWatch.getState().stop()} testId="batch-stop">
            Stop watching
          </Button>
        )}
        <Button onClick={close}>Close</Button>
        {mode === 'batch' && !desktop && items && items.length > 0 && !running && (
          <Button
            onClick={() =>
              void zipResults(items)
                .then((z) => downloadBytes(z.name, z.mime, z.base64))
                .catch(notifyError)
            }
          >
            Download again
          </Button>
        )}
        <Button type="submit" variant="primary" disabled={!ready || running} testId="batch-run">
          {mode === 'watch' ? (watching ? 'Watch this folder instead' : 'Start watching') : running ? 'Running…' : `Run on ${inputs.length} ${inputs.length === 1 ? 'file' : 'files'}`}
        </Button>
      </footer>
    </form>
  );
}

function Results({ items }: { items: BatchItem[] }) {
  const s = useMemo(() => summarise(items), [items]);
  const ps = useMemo(() => pColumns(items), [items]);
  const failed = items.filter((i) => i.error);
  const errCol = s.columns.length - 1;
  return (
    <div className="batch-results">
      <div className="preview" aria-label="Summary">
        <div className="preview-caption">
          Summary: {items.length} {items.length === 1 ? 'file' : 'files'}
          {failed.length ? `, ${failed.length} with errors` : ''}.
        </div>
        <div className="preview-scroll">
          <table className="preview-table batch-summary" data-testid="batch-summary">
            <thead>
              <tr>
                {s.columns.slice(0, errCol).map((c) => (
                  <th key={c} title={c}>
                    {c}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {s.rows.map((r, i) => (
                <tr key={i} className={items[i]?.error ? 'is-error' : undefined}>
                  {r.slice(0, errCol).map((v, j) => (
                    <td key={j} className={j === 0 ? 'batch-file' : undefined}>
                      {j > 0 && ps.has(s.columns[j]!) && typeof v === 'number' && v < 0.0001 ? '< 0.0001' : formatSummaryCell(v)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
      {failed.length > 0 && (
        <ul className="batch-errors" data-testid="batch-errors">
          {failed.map((i) => (
            <li key={i.name}>
              <span className="mono">{i.name}</span>: {i.error}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
