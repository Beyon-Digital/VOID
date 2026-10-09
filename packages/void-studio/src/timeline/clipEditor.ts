// ClipEditor — the timeline's command lane.
//
// Reads: pages of CLIP_LIST scoped per track (never a whole-document
// clone — the store's bounded cache holds the projections). Writes: one
// PersistentOp per committed gesture via sendWithStaleRetry. On a failed
// receipt the caller reverts its optimistic preview and surfaces
// describeReceiptError.

import type {
  CommandReceipt,
  PersistentOp,
  ReadItem,
  VoidClient,
} from 'void-client';
import {
  sendWithStaleRetry,
  describeReceiptError,
  type EditOutcome,
} from '../workspaces/editSession';
import type { StudioStore } from '../store';
import { makeViewKey } from '../store';
import { parseClipItem, type ClipView } from './geometry';
import type { ClipDrag, DragPreviewState } from './drag';
import { commitClipDrag } from './drag';

export interface ClipEditResult extends EditOutcome {
  /** Human-readable error when the receipt failed ('' when applied). */
  errorText: string;
}

async function issue(
  client: VoidClient,
  op: PersistentOp,
  transactionId: string,
  refresh?: () => Promise<unknown> | unknown,
): Promise<ClipEditResult> {
  const out = await sendWithStaleRetry(client, op, {
    transactionId,
    refresh,
  });
  return { ...out, errorText: describeReceiptError(out.receipt) };
}

export class ClipEditor {
  constructor(
    private readonly client: VoidClient,
    private readonly newId: () => string,
  ) {}

  /**
   * Page through CLIP_LIST for one track (cursor pagination — the engine
   * bounds each page, we merge into the store's bounded cache).
   */
  async loadTrackClips(
    store: StudioStore,
    trackId: string,
    opts: { startTicks?: string; endTicks?: string; maxPages?: number } = {},
  ): Promise<{ clips: ClipView[]; dropped: number }> {
    const clips: ClipView[] = [];
    let dropped = 0;
    for await (const page of this.client.readViewPages({
      view: 'CLIP_LIST',
      track_id: trackId,
      start_ticks: opts.startTicks,
      end_ticks: opts.endTicks,
      maxPages: opts.maxPages,
    })) {
      store
        .getState()
        .actions.mergeReadPage(
          makeViewKey('CLIP_LIST', trackId, opts.startTicks, opts.endTicks),
          page,
        );
      for (const item of page.items as ReadItem[]) {
        const c = parseClipItem(item);
        if (c) clips.push(c);
        else dropped++;
      }
      if (page.error && page.error !== 'NONE') break;
      if (page.done) break;
    }
    return { clips, dropped };
  }

  /** Commit a finished drag gesture as one transaction. */
  async commitDrag(
    d: ClipDrag,
    preview: DragPreviewState,
    refresh?: () => Promise<unknown> | unknown,
  ): Promise<ClipEditResult> {
    const needsNewId = d.mode === 'split' || d.mode === 'duplicate';
    const op = commitClipDrag(d, preview, needsNewId ? this.newId() : undefined);
    return issue(this.client, op, d.transactionId, refresh);
  }

  async moveClip(
    clipId: string,
    trackId: string,
    startTicks: string,
    transactionId: string,
    refresh?: () => Promise<unknown> | unknown,
  ): Promise<ClipEditResult> {
    return issue(
      this.client,
      {
        MoveClipOp: { clip_id: clipId, track_id: trackId, start_ticks: startTicks },
      },
      transactionId,
      refresh,
    );
  }

  async removeClip(
    clipId: string,
    transactionId: string,
    refresh?: () => Promise<unknown> | unknown,
  ): Promise<ClipEditResult> {
    return issue(
      this.client,
      { RemoveClipOp: { clip_id: clipId } },
      transactionId,
      refresh,
    );
  }
}

export function clipReceiptOk(r: CommandReceipt): boolean {
  return r.status === 'APPLIED' || r.status === 'DUPLICATE';
}
