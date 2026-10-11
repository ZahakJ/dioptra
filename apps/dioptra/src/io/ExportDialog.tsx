import { useState } from 'react';
import type { TableFormat } from '../engine/types';
import { DialogHeader } from '../app/dialogs';
import { Button, RadioList } from '../ui';

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
      <div className="field">
        <span className="field-label">Format</span>
        <RadioList label="Format" value={format} onChange={setFormat} choices={FORMATS.map((f) => ({ value: f.value, label: f.label, hint: f.hint, testId: `export-format-${f.value}` }))} />
      </div>
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary">
          {desktop ? 'Choose location' : 'Download'}
        </Button>
      </footer>
    </form>
  );
}
