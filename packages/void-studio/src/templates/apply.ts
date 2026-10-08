// Apply a project template (W11 item 2, DOC-01).
//
// One template application == one user gesture == one transaction id:
// CreateProjectOp -> SetTimeSignatureOp (only when non-4/4) -> AddTrackOp
// per spec -> InsertPluginOp per instrument spec. Every step is a real
// persistent command; the result lists each receipt and stops at the
// first rejection — a partially applied template is reported as such,
// never silently continued or faked as complete.

import type { CommandReceipt, PersistentOpName, VoidClient } from 'void-client';
import { receiptOk } from 'void-client';
import type { Translator } from '../i18n/catalog';
import { descriptorByUid, insertInstrumentOp } from '../instruments/descriptors';
import type { ProjectTemplateDescriptor } from './templates';

export interface TemplateApplyStep {
  /** Human/log label, e.g. "create", "track:Drums", "instrument:Sampler". */
  label: string;
  opName: PersistentOpName;
  receipt: CommandReceipt;
}

export interface TemplateApplyResult {
  ok: boolean;
  projectId: string;
  containerDir: string;
  templateId: string;
  /** Steps that produced receipts, in order. */
  steps: TemplateApplyStep[];
  /** The step whose receipt was not APPLIED/DUPLICATE (undefined when ok). */
  failedStep?: TemplateApplyStep;
}

export interface ApplyTemplateOptions {
  /** Deterministic ids for tests; defaults to crypto.randomUUID. */
  ids?: () => string;
  /** Track-name resolution — pass the shell's `t`. */
  t: Translator;
  /** One gesture → one transaction id; defaults to a fresh ids(). */
  transactionId?: string;
}

function defaultIds(): string {
  return crypto.randomUUID();
}

/**
 * Create `projectId`/`containerDir` from `template` via real ops.
 * `client.setProject` is called up-front so the commands address the new
 * project; callers should still gate on `engine.attached` before calling.
 */
export async function applyProjectTemplate(
  client: VoidClient,
  template: ProjectTemplateDescriptor,
  fields: { projectId: string; name: string; containerDir: string },
  opts: ApplyTemplateOptions,
): Promise<TemplateApplyResult> {
  const ids = opts.ids ?? defaultIds;
  const transactionId = opts.transactionId ?? ids();
  const steps: TemplateApplyStep[] = [];
  const result: TemplateApplyResult = {
    ok: false,
    projectId: fields.projectId,
    containerDir: fields.containerDir,
    templateId: template.id,
    steps,
  };

  const step = async (
    label: string,
    opName: PersistentOpName,
    send: () => Promise<CommandReceipt>,
  ): Promise<boolean> => {
    const receipt = await send();
    const s = { label, opName, receipt };
    steps.push(s);
    if (!receiptOk(receipt)) {
      result.failedStep = s;
      return false;
    }
    return true;
  };

  client.setProject(fields.projectId);
  if (
    !(await step('create', 'CreateProjectOp', () =>
      client.sendCommand(
        {
          CreateProjectOp: {
            name: fields.name,
            container_dir: fields.containerDir,
            sample_rate: template.sampleRate,
            initial_bpm: template.initialBpm,
          },
        },
        { transactionId },
      ),
    ))
  ) {
    return result;
  }

  const sig = template.timeSignature;
  if (sig && !(sig.numerator === 4 && sig.denominator === 4)) {
    if (
      !(await step('time-signature', 'SetTimeSignatureOp', () =>
        client.sendCommand(
          {
            SetTimeSignatureOp: {
              at_ticks: '0',
              numerator: sig.numerator,
              denominator: sig.denominator,
            },
          },
          { transactionId },
        ),
      ))
    ) {
      return result;
    }
  }

  for (const spec of template.tracks) {
    const trackId = ids();
    const trackName = opts.t(spec.nameKey);
    if (
      !(await step(`track:${trackName}`, 'AddTrackOp', () =>
        client.sendCommand(
          { AddTrackOp: { track_id: trackId, kind: spec.kind, name: trackName } },
          { transactionId },
        ),
      ))
    ) {
      return result;
    }
    if (spec.instrumentUid) {
      const descriptor = descriptorByUid(spec.instrumentUid);
      if (!descriptor) {
        // Descriptor bug — a template naming an unknown instrument is a
        // defect we surface, not an engine rejection we fake.
        throw new Error(`template ${template.id}: unknown instrumentUid ${spec.instrumentUid}`);
      }
      const instanceId = ids();
      if (
        !(await step(`instrument:${descriptor.name}`, 'InsertPluginOp', () =>
          client.sendCommand(insertInstrumentOp(trackId, descriptor, instanceId), {
            transactionId,
          }),
        ))
      ) {
        return result;
      }
    }
  }

  result.ok = true;
  return result;
}
