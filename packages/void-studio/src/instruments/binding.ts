// Param binding — one user gesture → ops on the wire with revision
// tracking (W10; mirrors editSession semantics).
//
// Every send goes through `sendWithStaleRetry`: a STALE_REVISION receipt
// re-reads the PLUGIN_LIST view once and retries against the receipt's
// revision; any other failure surfaces verbatim for the caller to
// display. The rack store only records values the coordinator accepted
// (or values sent on the optimistic first send — callers may revert).

import type { VoidClient } from 'void-client';
import {
  sendWithStaleRetry,
  type EditOutcome,
} from '../workspaces/editSession';
import {
  insertInstrumentOp,
  removeInstrumentOp,
  setInstrumentParamOp,
  defaultParamOps,
  clampParam,
  paramOf,
  type InstrumentDescriptor,
} from './descriptors';
import { resolvePreset } from './presets';
import type { InstrumentPreset, InstrumentBrowseStore } from './presets';

export interface InstrumentBinding {
  client: VoidClient;
  store: InstrumentBrowseStore;
  /** Re-read PLUGIN_LIST (the view instance params live in). */
  refreshPlugins: () => Promise<unknown> | unknown;
  /** Shared transaction id for one user gesture (insert+preset). */
  transactionId?: string;
}

/**
 * Insert an instrument and, when a preset is given, apply its complete
 * param state as one gesture (single transaction_id). The ops run
 * sequentially against the *same* transaction so undo removes the whole
 * "load instrument" action, not individual params.
 */
export async function loadInstrument(
  b: InstrumentBinding,
  trackId: string,
  descriptor: InstrumentDescriptor,
  instanceId: string,
  slotIndex: number,
  preset?: InstrumentPreset,
): Promise<EditOutcome[]> {
  const outcomes: EditOutcome[] = [];
  const insert = await sendWithStaleRetry(
    b.client,
    insertInstrumentOp(trackId, descriptor, instanceId, slotIndex),
    { transactionId: b.transactionId, refresh: b.refreshPlugins },
  );
  outcomes.push(insert);
  if (insert.receipt.status !== 'APPLIED' && insert.receipt.status !== 'DUPLICATE') {
    // Insert failed — do not fire param ops at a plugin that may not exist.
    return outcomes;
  }
  const params = preset
    ? resolvePreset(descriptor, preset)
    : descriptor.params.map((p) => ({ param: p, value: p.defaultValue }));
  for (const { param, value } of params) {
    const r = await sendWithStaleRetry(
      b.client,
      setInstrumentParamOp(descriptor, instanceId, param.id, value),
      { transactionId: b.transactionId, refresh: b.refreshPlugins },
    );
    outcomes.push(r);
    if (r.receipt.status === 'APPLIED' || r.receipt.status === 'DUPLICATE') {
      b.store.getState().actions.noteParamSent(instanceId, param.id, value);
    }
  }
  b.store.getState().actions.upsertSlot({
    slotIndex,
    instanceId,
    pluginUid: descriptor.pluginUid,
    lastKnown: Object.fromEntries(params.map(({ param, value }) => [param.id, value])),
  });
  return outcomes;
}

/**
 * One control change → SetPluginParamOp. Value is clamped to the
 * descriptor range before it reaches the wire; a rejected send leaves
 * the rack's lastKnown untouched (the UI reverts to the last accepted
 * value — honest, per receiptFailure convention).
 */
export async function setInstrumentParam(
  b: InstrumentBinding,
  descriptor: InstrumentDescriptor,
  instanceId: string,
  paramId: string,
  value: number,
): Promise<EditOutcome> {
  const p = paramOf(descriptor, paramId);
  if (!p) throw new Error(`param ${paramId} not exposed by ${descriptor.name}`);
  const sent = clampParam(p, value); // what actually goes on the wire
  const outcome = await sendWithStaleRetry(
    b.client,
    setInstrumentParamOp(descriptor, instanceId, paramId, sent),
    { transactionId: b.transactionId, refresh: b.refreshPlugins },
  );
  if (outcome.receipt.status === 'APPLIED' || outcome.receipt.status === 'DUPLICATE') {
    b.store.getState().actions.noteParamSent(instanceId, paramId, sent);
  }
  return outcome;
}

/** Remove an instrument instance. */
export async function removeInstrument(
  b: InstrumentBinding,
  instanceId: string,
): Promise<EditOutcome> {
  const outcome = await sendWithStaleRetry(b.client, removeInstrumentOp(instanceId), {
    transactionId: b.transactionId,
    refresh: b.refreshPlugins,
  });
  if (outcome.receipt.status === 'APPLIED' || outcome.receipt.status === 'DUPLICATE') {
    b.store.getState().actions.removeSlot(instanceId);
  }
  return outcome;
}

/** Reset an instrument to descriptor defaults as one gesture. */
export async function resetInstrumentParams(
  b: InstrumentBinding,
  descriptor: InstrumentDescriptor,
  instanceId: string,
): Promise<EditOutcome[]> {
  const outcomes: EditOutcome[] = [];
  for (const op of defaultParamOps(descriptor, instanceId)) {
    const r = await sendWithStaleRetry(b.client, op, {
      transactionId: b.transactionId,
      refresh: b.refreshPlugins,
    });
    outcomes.push(r);
    if (r.receipt.status === 'APPLIED' || r.receipt.status === 'DUPLICATE') {
      const f = (op as { SetPluginParamOp: { param_id: string; value: number } })
        .SetPluginParamOp;
      b.store.getState().actions.noteParamSent(instanceId, f.param_id, f.value);
    }
  }
  return outcomes;
}
