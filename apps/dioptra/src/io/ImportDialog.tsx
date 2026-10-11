import { useMemo, useState } from 'react';
import type { DecimalMark, PasteOptions, Separator, TableKind } from '../engine/types';
import { DialogHeader } from '../app/dialogs';
import { KindPicker } from '../app/NewTableDialog';
import { kindInfo } from '../app/kinds';
import { parsePreview, suggestKind } from './preview';

export interface PasteImportSpec {
  text: string;
  kind: TableKind;
  title: string | null;
  options: PasteOptions;
}

export function PasteImportDialog({
  initialText,
  close,
}: {
  initialText: string;
  close: (spec: PasteImportSpec | null) => void;
}) {
  const [text, setText] = useState(initialText);
  const [title, setTitle] = useState('');
  const [decimal, setDecimal] = useState<DecimalMark>('auto');
  const [separator, setSeparator] = useState<Separator>('auto');
  const [header, setHeader] = useState<'auto' | 'yes' | 'no'>('auto');
  const headerOpt = header === 'auto' ? null : header === 'yes';
  const preview = useMemo(() => parsePreview(text, { decimal, separator, header: headerOpt }, 10), [text, decimal, separator, headerOpt]);
  const suggestion = useMemo(() => suggestKind(preview), [preview]);
  const [kind, setKind] = useState<TableKind | null>(null);
  const chosen = kind ?? suggestion.kind;
  const body = preview.rows.slice(preview.header ? 1 : 0);
  const head = preview.header ? preview.rows[0] : undefined;
  const sepName = { tab: 'tab', comma: 'comma', semicolon: 'semicolon', whitespace: 'spaces' }[preview.separator];

  return (
    <form
      className="modal-body modal-wide"
      data-testid="paste-import-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        if (!text.trim()) return;
        close({ text, kind: chosen, title: title.trim() || null, options: { decimal, separator, header: headerOpt } });
      }}
    >
      <DialogHeader title="Paste as new table" sub="Paste raw text from a plate reader, a lab notebook or a spreadsheet. Text cells are kept as text." />
      <div className="import-split">
        <div className="import-left">
          <label className="field">
            <span className="field-label">Text</span>
            <textarea
              className="input mono import-text"
              value={text}
              spellCheck={false}
              placeholder="Paste here (Ctrl+V)"
              onChange={(e) => setText(e.target.value)}
              data-testid="paste-import-text"
            />
          </label>
          <div className="form-row">
            <label className="field">
              <span className="field-label">Separator</span>
              <select className="input" value={separator} onChange={(e) => setSeparator(e.target.value as Separator)}>
                <option value="auto">Detect ({sepName})</option>
                <option value="tab">Tab</option>
                <option value="comma">Comma</option>
                <option value="semicolon">Semicolon</option>
                <option value="whitespace">Spaces</option>
              </select>
            </label>
            <label className="field">
              <span className="field-label">Decimal mark</span>
              <select className="input" value={decimal} onChange={(e) => setDecimal(e.target.value as DecimalMark)}>
                <option value="auto">Detect ({preview.decimal === 'comma' ? 'comma' : 'point'})</option>
                <option value="point">Point (1.5)</option>
                <option value="comma">Comma (1,5)</option>
              </select>
            </label>
            <label className="field">
              <span className="field-label">First row</span>
              <select className="input" value={header} onChange={(e) => setHeader(e.target.value as 'auto' | 'yes' | 'no')}>
                <option value="auto">Detect ({preview.header ? 'header' : 'data'})</option>
                <option value="yes">Header</option>
                <option value="no">Data</option>
              </select>
            </label>
          </div>
          <label className="field">
            <span className="field-label">Title</span>
            <input className="input" value={title} placeholder="Automatic" onChange={(e) => setTitle(e.target.value)} />
          </label>
        </div>
        <div className="preview" aria-label="Preview of the first rows">
          <div className="preview-caption">
            Preview: {preview.totalRows} {preview.totalRows === 1 ? 'row' : 'rows'} &times; {preview.cols}{' '}
            {preview.cols === 1 ? 'column' : 'columns'}
            {preview.totalRows > 10 ? ', first 10 shown' : ''}. The engine makes the final parse.
          </div>
          <div className="preview-scroll">
            <table className="preview-table">
              {head && (
                <thead>
                  <tr>
                    {head.map((h, i) => (
                      <th key={i}>{h}</th>
                    ))}
                  </tr>
                </thead>
              )}
              <tbody>
                {body.map((r, i) => (
                  <tr key={i}>
                    {Array.from({ length: preview.cols }, (_, j) => (
                      <td key={j}>{r[j] ?? ''}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>
      <div className="import-kind">
        <div className="field-label">Table kind</div>
        <p className="hint-line" data-testid="kind-suggestion">
          Suggested: {kindInfo(suggestion.kind)?.name ?? suggestion.kind}. {suggestion.why}
        </p>
        <KindPicker value={chosen} onChange={setKind} suggested={suggestion.kind} compact />
      </div>
      <footer className="modal-foot">
        <button type="button" className="btn" onClick={() => close(null)}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary" disabled={!text.trim()} data-testid="paste-import-create">
          Create table
        </button>
      </footer>
    </form>
  );
}

export interface FileImportSpec {
  kind: TableKind | null;
}

export function FileImportDialog({ close }: { close: (spec: FileImportSpec | null) => void }) {
  const [kind, setKind] = useState<TableKind | 'detect'>('detect');
  return (
    <form
      className="modal-body modal-wide"
      onSubmit={(e) => {
        e.preventDefault();
        close({ kind: kind === 'detect' ? null : kind });
      }}
    >
      <DialogHeader
        title="Import a file"
        sub="CSV, TSV and text files, Excel (xlsx, xls), OpenDocument (ods) and Prism (pzfx). Workbooks and Prism files keep their own structure."
      />
      <div className="field-label">Table kind for delimited text</div>
      <label className="choice">
        <input type="radio" checked={kind === 'detect'} onChange={() => setKind('detect')} />
        <span>
          <span className="choice-title">Decide from the file</span>
          <span className="choice-hint">A multiple-variables table unless the file says otherwise.</span>
        </span>
      </label>
      <KindPicker value={kind === 'detect' ? ('' as TableKind) : kind} onChange={setKind} compact />
      <footer className="modal-foot">
        <button type="button" className="btn" onClick={() => close(null)}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary">
          Choose file
        </button>
      </footer>
    </form>
  );
}
