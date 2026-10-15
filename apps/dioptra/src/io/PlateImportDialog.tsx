// The plate-map step of Import file… for plate-reader exports.
import { useState } from 'react';
import { DialogHeader } from '../app/dialogs';
import type { PlateSummary } from '../engine/types';
import { Button, Field, TextArea } from '../ui';

export interface PlateImportSpec {
  plateMap: string;
}

const EXAMPLE = '\t1\t2\t3\nA\tBlank\tDrug @ 0.1\tDrug @ 1\nB\tBlank\tDrug @ 0.1\tDrug @ 1';

export function PlateImportDialog({ name, plate, close }: { name: string; plate: PlateSummary; close: (spec: PlateImportSpec | null) => void }) {
  const [map, setMap] = useState('');
  const reads = plate.reads.length;
  const shape = plate.kinetic
    ? 'An XY table: time in seconds down the rows, one data set per well, or per group of the plate map with its wells as replicates.'
    : 'Without a plate map, a grouped table: plate rows down, one data set per read with the plate columns side by side. With groups, a column table; with doses, an XY table of response against dose.';
  return (
    <form
      className="modal-body modal-wide"
      data-testid="plate-import-dialog"
      onSubmit={(e) => {
        e.preventDefault();
        close({ plateMap: map });
      }}
    >
      <DialogHeader
        title="Import a plate"
        sub={
          <>
            <span className="mono">{name}</span> is a {plate.vendor_label} export: {plate.rows * plate.cols}-well plate, {plate.kinetic ? `kinetic with ${plate.time_points} time points` : 'endpoint'},{' '}
            {reads} {reads === 1 ? 'read' : 'reads'} ({plate.reads.join(', ')}).
          </>
        }
      />
      <p className="modal-text">{shape}</p>
      <Field
        label="Plate map (optional)"
        hint="Paste a grid as copied from a spreadsheet (row letters and column numbers optional) or one well, label line per well. A label is a group (Control), a dose (10), or both (Drug A @ 10). Empty cells leave a well out."
      >
        {(lid) => <TextArea mono rows={6} aria-labelledby={lid} value={map} placeholder={EXAMPLE} onChange={(e) => setMap(e.target.value)} testId="plate-map" />}
      </Field>
      {plate.notes.map((n) => (
        <p key={n} className="field-hint">
          {n}
        </p>
      ))}
      <footer className="modal-foot">
        <Button onClick={() => close(null)}>Cancel</Button>
        <Button type="submit" variant="primary" testId="plate-import-create">
          Create table
        </Button>
      </footer>
    </form>
  );
}
