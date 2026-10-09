// S11/S12 app-level export state — the dialog store singleton plus the
// last validated spec. Shared by the preflight screen (export) and the
// completed screen (export-done) so a submitted job's spec survives the
// route change.

import { createExportDialogStore } from 'void-studio';
import type { ExportSpecDto } from 'void-studio';

export const exportView = createExportDialogStore();

/** The most recent spec that passed validation — display only. */
export const lastSubmit: {
  spec: ExportSpecDto | null;
  status:
    | 'idle'
    | 'ready'
    | 'submitted'
    | 'unavailable'
    | 'rejected'
    | 'error';
  message?: string;
} = { spec: null, status: 'idle' };
