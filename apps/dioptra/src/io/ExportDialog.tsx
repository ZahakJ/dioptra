import { useState } from 'react';
import type { TableFormat } from '../engine/types';
import { DialogHeader } from '../app/dialogs';

const FORMATS: { value: TableFormat; label: string; hint: string }[] = [
  { value: 'csv', label: 'CSV', hint: 'Comma separated, full precision, point decimals.' },
  { value: 'tsv', label: 'TSV', hint: 'Tab separated, full precision.' },
  { value: 'xlsx', label: 'Excel workbook (xlsx)', hint: 'Numbers stored as numbers, text as text.' },
];

export function ExportDialog({
  tableTitle,
  desktop,
  close,
}: {
  tableTitle: string;
  desktop: boolean;
  close: (format: TableFormat | null) => void;
}) {
  const [format, setFormat] = useState<TableFormat>('csv');
  return (
    <form
      className="modal-body modal-narrow"
      data-testid="export-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        close(format);
      }}
    >
      <DialogHeader title="Export table" sub={`"${tableTitle}": every value at full precision.`} />
      <fieldset className="choice-list">
        <legend className="field-label">Format</legend>
        {FORMATS.map((f) => (
          <label key={f.value} className="choice">
            <input type="radio" name="fmt" checked={format === f.value} onChange={() => setFormat(f.value)} />
            <span>
              <span className="choice-title">{f.label}</span>
              <span className="choice-hint">{f.hint}</span>
            </span>
          </label>
        ))}
      </fieldset>
      <footer className="modal-foot">
        <button type="button" className="btn" onClick={() => close(null)}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary">
          {desktop ? 'Choose location' : 'Download'}
        </button>
      </footer>
    </form>
  );
}
