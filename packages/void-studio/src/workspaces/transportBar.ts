// Shared transport-bar model — one bar across all workspaces.
//
// Everything display-side derives from ClockSnapshot (the only clock the
// WebView may trust); everything command-side goes through the real
// client: PLAY/STOP/SEEK/PANIC via send_transport, and loop persistence
// via SetLoopRangeOp (a document edit) — SET_CYCLE on the transport
// channel is the non-persistent live equivalent.

import type { ClockSnapshot, CommandReceipt, VoidClient } from 'void-client';
import { parseI64 } from 'void-client';
import { sendWithStaleRetry } from './editSession';
import type { EditIssue, EditOutcome } from './editSession';

export interface TransportBarState {
  /** From the last ClockSnapshot; '—' when no snapshot has arrived. */
  transport: 'STOPPED' | 'PLAYING' | 'RECORDING' | 'PAUSED' | '—';
  /** Position on the musical timeline in engine samples (honest — the wire ships samples, not ticks). */
  timelineSample: string | null;
  /** Cycle/loop indicator driven by ClockSnapshot loop bounds. */
  cycle: { active: boolean; startTicks: string; endTicks: string };
  /** tempo_map_revision — invalidates the snap grid when it moves. */
  tempoMapRevision: string | null;
  /** Monotonic counters for diagnostics (never used as a UI clock). */
  sequence: string | null;
  hostClockNs: string | null;
  sampleRate: number | null;
}

export function transportBarState(
  clock: ClockSnapshot | undefined,
  attached: boolean,
): TransportBarState {
  if (!clock) {
    return {
      transport: '—',
      timelineSample: null,
      cycle: { active: false, startTicks: '0', endTicks: '0' },
      tempoMapRevision: null,
      sequence: null,
      hostClockNs: null,
      sampleRate: null,
    };
  }
  const start = parseI64(clock.loop_start_ticks);
  const end = parseI64(clock.loop_end_ticks);
  return {
    transport: attached ? clock.transport : '—',
    timelineSample: clock.timeline_sample,
    cycle: {
      active: end > start,
      startTicks: clock.loop_start_ticks,
      endTicks: clock.loop_end_ticks,
    },
    tempoMapRevision: clock.tempo_map_revision,
    sequence: clock.sequence,
    hostClockNs: clock.host_clock_ns,
    sampleRate: clock.sample_rate,
  };
}

/**
 * Persistent loop range edit — SetLoopRangeOp through send_command with
 * the shared stale-revision retry. end_ticks <= start_ticks with
 * enabled=true is rejected client-side (malformed range never leaves JS).
 */
export async function setLoopRange(
  client: VoidClient,
  fields: { startTicks: string; endTicks: string; enabled: boolean },
  issue: EditIssue = {},
): Promise<EditOutcome> {
  const start = parseI64(fields.startTicks);
  const end = parseI64(fields.endTicks);
  if (fields.enabled && end <= start) {
    throw new Error(
      `loop range requires end > start (${fields.startTicks}..${fields.endTicks})`,
    );
  }
  return sendWithStaleRetry(
    client,
    {
      SetLoopRangeOp: {
        start_ticks: fields.startTicks,
        end_ticks: fields.endTicks,
        enabled: fields.enabled,
      },
    },
    issue,
  );
}

/**
 * Live (non-persistent) cycle via the transport channel — the immediate
 * loop marker without a document edit. Distinct from SetLoopRangeOp:
 * use setLoopRange for the committed range, setCycle for preview/live.
 */
export function setLiveCycle(
  client: VoidClient,
  startTicks: string,
  endTicks: string,
): Promise<unknown> {
  const start = parseI64(startTicks);
  const end = parseI64(endTicks);
  if (end <= start) {
    return Promise.reject(
      new Error(`cycle requires end > start (${startTicks}..${endTicks})`),
    );
  }
  return client.setCycle(startTicks, endTicks);
}
