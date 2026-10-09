// Project lifecycle helpers shared by the projects + new-project screens
// (S13/S14). These run the same coordinator ops the launcher does — create →
// attach → open through void-client, template application in one transaction,
// recents updated only after an APPLIED/DUPLICATE receipt. Nothing here is a
// parallel path: it calls the same adapters as ui/support.tsx's LauncherPanel.

import {
  applyProjectTemplate,
  createTranslator,
  en,
  studioStore,
  type ProjectTemplateDescriptor,
} from 'void-studio';
import { receiptOk } from 'void-client';
import { getClient } from '../../../client';
import { recents } from '../../../support';

const t = createTranslator(en);

export interface LifecycleResult {
  ok: boolean;
  /** Human-readable failure — rejected receipt or thrown error. */
  error?: string;
}

export function slugifyName(name: string): string {
  const s = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return s || 'untitled';
}

/** create → attach → open via applyProjectTemplate (single transactionId). */
export async function createProjectFromTemplate(args: {
  template: ProjectTemplateDescriptor;
  name: string;
  containerDir: string;
}): Promise<LifecycleResult> {
  const { template, name, containerDir } = args;
  const projectId = crypto.randomUUID(); // project ids must be UUIDs (is_valid_id)
  try {
    const res = await applyProjectTemplate(getClient(), template, { projectId, name, containerDir }, { t });
    const last = res.steps[res.steps.length - 1];
    if (res.ok && last) {
      studioStore.getState().actions.setProject(projectId, last.receipt.revision ?? '0');
      recents().getState().actions.recordOpen({ containerDir, projectId, name, kind: 'created' });
      return { ok: true };
    }
    if (res.failedStep) {
      const r = res.failedStep.receipt;
      return { ok: false, error: `${res.failedStep.opName} rejected (${r.error})${r.message ? ` — ${r.message}` : ''}` };
    }
    return { ok: false, error: 'template produced no steps' };
  } catch (e) {
    return { ok: false, error: String(e instanceof Error ? e.message : e) };
  }
}

/** Open an existing project by container dir (attach → OpenProjectOp). */
export async function openProjectByDir(args: {
  containerDir: string;
  projectId?: string;
  displayName?: string;
}): Promise<LifecycleResult> {
  const { containerDir, projectId, displayName } = args;
  try {
    const c = getClient();
    if (projectId) c.setProject(projectId);
    const receipt = await c.sendCommand({ OpenProjectOp: { container_dir: containerDir } });
    if (receiptOk(receipt)) {
      studioStore.getState().actions.setProject(projectId || containerDir, receipt.revision ?? '0');
      recents().getState().actions.recordOpen({
        containerDir,
        projectId: projectId || containerDir,
        name: displayName ?? containerDir.split('/').filter(Boolean).pop() ?? containerDir,
        kind: 'opened',
      });
      return { ok: true };
    }
    return { ok: false, error: `OpenProjectOp rejected (${receipt.error})${receipt.message ? ` — ${receipt.message}` : ''}` };
  } catch (e) {
    return { ok: false, error: String(e instanceof Error ? e.message : e) };
  }
}
