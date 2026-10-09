// Pointer-capture gesture wiring for the continuous controls.
// Contract (VOID_UI_UX_Build_Prompt §behavior contracts):
//   - setPointerCapture on down; moves stream previews
//   - pointerup commits ONCE (one gesture = one undo transaction)
//   - pointercancel / lostpointercapture end the gesture gracefully —
//     treated as commit so a lost pointer can never strand a preview
//   - Escape during a gesture cancels it and swallows the event before
//     any other UI (menus, drawers, transport) reacts.

import * as React from 'react';
import {
  beginGesture,
  cancelGesture,
  commitGesture,
  globalEscapeStack,
  updateGesture,
  type GestureSession,
} from './editing';

export interface PointerGestureHandlers {
  onBegin?: (startValue: number) => void;
  /** Preview updates during the gesture (already-clamped value). */
  onPreview?: (value: number) => void;
  /** Single commit at gesture end; `from`/`to` is the undo transaction. */
  onCommit?: (commit: { from: number; to: number }) => void;
  /** Gesture cancelled via Escape — preview value is discarded. */
  onCancel?: () => void;
}

export interface UsePointerGestureOptions extends PointerGestureHandlers {
  /** Current committed value — the gesture's starting point. */
  value: number;
  /** Map a pointer-move event to the next clamped value. */
  delta: (event: React.PointerEvent, session: GestureSession) => number;
  disabled?: boolean;
}

export interface UsePointerGestureResult {
  active: boolean;
  onPointerDown: (e: React.PointerEvent<HTMLElement>) => void;
  onPointerMove: (e: React.PointerEvent<HTMLElement>) => void;
  onPointerUp: (e: React.PointerEvent<HTMLElement>) => void;
  onPointerCancel: (e: React.PointerEvent<HTMLElement>) => void;
  onLostPointerCapture: (e: React.PointerEvent<HTMLElement>) => void;
  onKeyDown: (e: React.KeyboardEvent<HTMLElement>) => void;
}

export function usePointerGesture(opts: UsePointerGestureOptions): UsePointerGestureResult {
  const { value, delta, disabled, onBegin, onPreview, onCommit, onCancel } = opts;
  const [session, setSession] = React.useState<GestureSession | null>(null);
  const sessionRef = React.useRef<GestureSession | null>(null);
  const popEscapeRef = React.useRef<(() => void) | null>(null);

  const end = React.useCallback(
    (commit: boolean) => {
      const s = sessionRef.current;
      if (!s) return;
      sessionRef.current = null;
      setSession(null);
      popEscapeRef.current?.();
      popEscapeRef.current = null;
      if (commit) {
        const tx = commitGesture(s);
        if (tx) onCommit?.(tx);
      } else {
        onCancel?.();
      }
    },
    [onCommit, onCancel],
  );

  const onPointerDown = React.useCallback(
    (e: React.PointerEvent<HTMLElement>) => {
      if (disabled || sessionRef.current) return;
      if (e.button !== 0 && e.pointerType === 'mouse') return;
      e.currentTarget.setPointerCapture(e.pointerId);
      const s = beginGesture(value);
      sessionRef.current = s;
      setSession(s);
      onBegin?.(value);
      // Escape cancels this gesture before any other UI sees the key.
      popEscapeRef.current = globalEscapeStack.push(() => {
        end(false);
      });
    },
    [disabled, value, onBegin, end],
  );

  const onPointerMove = React.useCallback(
    (e: React.PointerEvent<HTMLElement>) => {
      const s = sessionRef.current;
      if (!s || !s.active) return;
      const next = delta(e, s);
      const s2 = updateGesture(s, next);
      sessionRef.current = s2;
      setSession(s2);
      onPreview?.(next);
    },
    [delta, onPreview],
  );

  const releaseAndCommit = React.useCallback(
    (e: React.PointerEvent<HTMLElement>) => {
      const s = sessionRef.current;
      if (!s) return;
      try {
        if (e.currentTarget.hasPointerCapture(e.pointerId)) {
          e.currentTarget.releasePointerCapture(e.pointerId);
        }
      } catch {
        /* capture already lost — commit path below still runs */
      }
      end(true);
    },
    [end],
  );

  const onKeyDown = React.useCallback(
    (e: React.KeyboardEvent<HTMLElement>) => {
      if (e.key === 'Escape' && sessionRef.current) {
        e.preventDefault();
        e.stopPropagation();
        end(false);
      }
    },
    [end],
  );

  return {
    active: Boolean(session?.active),
    onPointerDown,
    onPointerMove,
    onPointerUp: releaseAndCommit,
    // Pointer-up loss: cancel/lost-capture still commit once so a dropped
    // pointer never strands the preview or skips the undo transaction.
    onPointerCancel: releaseAndCommit,
    onLostPointerCapture: releaseAndCommit,
    onKeyDown,
  };
}
