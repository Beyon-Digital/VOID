// Bind a VoidClient to a StudioStore: engine lifecycle → engine view,
// receipts → revision, telemetry → transient frames, engine-lost → detach.
// Read-view pages are fetched explicitly (loadView); nothing here ever
// materializes a whole project into the store.

import type {
  ClockSnapshot,
  MeterFrame,
  ReadResponse,
  SaveResultEvent,
  UnlistenFn,
  ViewKindName,
  VoidClient,
} from 'void-client';
import { makeViewKey, StudioStore } from './store';

export function bindStudioClient(
  store: StudioStore,
  client: VoidClient,
): UnlistenFn {
  const unsubs: UnlistenFn[] = [];

  unsubs.push(
    client.onControl((ev) => {
      const st = store.getState();
      if (ev.kind === 'CommandReceipt') {
        if (ev.revision) st.actions.setRevision(ev.revision);
        if (
          ev.status === 'REJECTED' &&
          ev.error === 'STALE_EPOCH'
        ) {
          // epoch rolled — stale projections must not linger
          st.actions.clearProjections();
        }
      }
    }),
  );

  unsubs.push(
    client.onTelemetry((ev) => {
      if (ev.kind === 'TransportAck') return; // callers await the ack
      store
        .getState()
        .actions.applyTelemetry(
          ev as ClockSnapshot | MeterFrame | SaveResultEvent,
        );
    }),
  );

  unsubs.push(
    client.onEngineLost(() => {
      const st = store.getState();
      st.actions.setEngine({ attached: false });
      st.actions.clearProjections();
    }),
  );

  return () => unsubs.forEach((u) => u());
}

/**
 * Pull one view page into the bounded store cache. Call again with the
 * returned nextCursor until done; the store enforces the per-view item cap.
 */
export async function loadViewPage(
  store: StudioStore,
  client: VoidClient,
  view: ViewKindName,
  opts: {
    trackId?: string;
    startTicks?: string;
    endTicks?: string;
    cursor?: string;
    limit?: number;
  } = {},
): Promise<ReadResponse> {
  const page = await client.readView({
    view,
    track_id: opts.trackId,
    start_ticks: opts.startTicks,
    end_ticks: opts.endTicks,
    cursor: opts.cursor,
    limit: opts.limit,
  });
  store
    .getState()
    .actions.mergeReadPage(
      makeViewKey(view, opts.trackId, opts.startTicks, opts.endTicks),
      page,
    );
  return page;
}
