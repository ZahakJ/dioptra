// File → Export to Prism (Dioptra Lab): the project's data tables as a .pzfx file.
import { lab, requireLab, saveLabFile, type LabFile } from '../graph/journal';
import { withEngine } from '../store/engine';
import { notifyError, useNotices } from '../store/notices';
import { useProject } from '../store/project';

export const PRISM_WHAT =
  'Data tables, with replicates, formats, row titles and exclusions, in a .pzfx file that Prism 5 and later opens.';

/** Whether the project has a table to export. */
export const hasTables = () => (useProject.getState().view?.items ?? []).some((i) => i.kind === 'table');

export async function exportPrism(): Promise<void> {
  if (!(await requireLab('Export to Prism', PRISM_WHAT))) return;
  try {
    const f = await withEngine((e) => lab<LabFile>(e, { type: 'lab_export_pzfx', tables: [] }, 'lab_file'));
    if (!(await saveLabFile(f))) return;
    if (f.notes.length > 0) {
      useNotices.getState().push({ kind: 'report', text: 'Written with approximations', detail: f.notes });
    }
  } catch (err) {
    notifyError(err);
  }
}
