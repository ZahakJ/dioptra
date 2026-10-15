import { useMemo, useState } from 'react';
import type { DecimalMark, PasteOptions, Separator, TableKind } from '../engine/types';
import { DialogHeader } from '../app/dialogs';
import { Button, Field, RadioRow, Segmented, TextArea, TextInput } from '../ui';
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
          <Field label="Text">
            {(lid) => (
              <TextArea
                mono
                aria-labelledby={lid}
                className="import-text"
                value={text}
                placeholder="Paste here (Ctrl+V)"
                onChange={(e) => setText(e.target.value)}
                testId="paste-import-text"
              />
            )}
          </Field>
          <div className="import-opts">
            <Field label="Separator">
              {(lid) => (
                <Segmented
                  labelledBy={lid}
                  value={separator}
                  onChange={setSeparator}
                  choices={[
                    { value: 'auto' as Separator, label: 'Detect', hint: `Detected: ${sepName}` },
                    { value: 'tab' as Separator, label: 'Tab' },
                    { value: 'comma' as Separator, label: 'Comma' },
                    { value: 'semicolon' as Separator, label: 'Semicolon' },
                    { value: 'whitespace' as Separator, label: 'Spaces' },
                  ]}
                />
              )}
            </Field>
            <Field label="Decimal mark">
              {(lid) => (
                <Segmented
                  labelledBy={lid}
                  value={decimal}
                  onChange={setDecimal}
                  choices={[
                    { value: 'auto' as DecimalMark, label: 'Detect', hint: `Detected: ${preview.decimal === 'comma' ? 'comma' : 'point'}` },
                    { value: 'point' as DecimalMark, label: <span className="mono">1.5</span>, aria: 'Point' },
                    { value: 'comma' as DecimalMark, label: <span className="mono">1,5</span>, aria: 'Comma' },
                  ]}
                />
              )}
            </Field>
            <Field label="First row">
              {(lid) => (
                <Segmented
                  labelledBy={lid}
                  value={header}
                  onChange={setHeader}
                  choices={[
                    { value: 'auto' as const, label: 'Detect', hint: `Detected: ${preview.header ? 'header' : 'data'}` },
                    { value: 'yes' as const, label: 'Header' },
                    { value: 'no' as const, label: 'Data' },
                  ]}
                />
              )}
            </Field>
          </div>
          <Field label="Title">{(lid) => <TextInput aria-labelledby={lid} value={title} placeholder="Automatic" onChange={(e) => setTitle(e.target.value)} />}</Field>
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
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" disabled={!text.trim()} testId="paste-import-create">
          Create table
        </Button>
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
        sub="CSV, TSV and text files, Excel (xlsx, xls), OpenDocument (ods) and Prism (pzfx, prism). Workbooks and Prism files keep their own structure."
      />
      <div className="field-label">Table kind for delimited text</div>
      <RadioRow checked={kind === 'detect'} onSelect={() => setKind('detect')} hint="A multiple-variables table unless the file says otherwise.">
        Decide from the file
      </RadioRow>
      <KindPicker value={kind === 'detect' ? ('' as TableKind) : kind} onChange={setKind} compact />
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary">
          Choose file
        </Button>
      </footer>
    </form>
  );
}
