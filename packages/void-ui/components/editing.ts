// VOID Signal Studio — shared value-editing behavior core.
//
// Framework-free state machines implementing the mandatory interaction
// contracts (docs/ui-handoff/VOID_UI_UX_Build_Prompt.md §behavior contracts):
//   - one gesture = one undo transaction (begin→preview→commit emits once)
//   - numeric entry + arrow keys + fine-adjust modifier + clamping + reset
//   - Escape cancels an active gesture/preview before other UI reacts
//   - keyboard/IME text input never triggers transport/global shortcuts
//
// Components (ParameterKnob, ChannelFader, TimelineClip, SceneCell…) drive
// these from React handlers; keeping the logic pure makes the contracts
// unit-testable without a DOM.

// ---------------------------------------------------------------------------
// value domain: clamping + stepped keyboard nudges
// ---------------------------------------------------------------------------

export interface ValueDomain {
  min: number;
  max: number;
  /** Arrow-key increment. */
  step: number;
  /** Fine-adjust increment applied when the fine modifier is held. */
  fineStep: number;
  /** Page Up/Down increment (defaults to 10× step). */
  pageStep?: number;
}

export function clampValue(value: number, min: number, max: number): number {
  if (Number.isNaN(value)) return min;
  return Math.min(max, Math.max(min, value));
}

export function clampToDomain(value: number, domain: ValueDomain): number {
  return clampValue(value, domain.min, domain.max);
}

export interface KeyModifiers {
  shiftKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  altKey?: boolean;
}

/**
 * Keyboard nudge contract for continuous controls:
 *   ArrowUp/ArrowRight +step · ArrowDown/ArrowLeft −step
 *   Shift + arrow = fineStep · PageUp/PageDown = pageStep (or ±10×step)
 *   Home = min · End = max · unhandled keys return null.
 * Fine modifier is Shift (coarse/fine convention across knob + fader).
 */
export function applyValueKey(
  value: number,
  key: string,
  mods: KeyModifiers,
  domain: ValueDomain,
): number | null {
  const step = mods.shiftKey ? domain.fineStep : domain.step;
  const page = domain.pageStep ?? domain.step * 10;
  let next: number;
  switch (key) {
    case 'ArrowUp':
    case 'ArrowRight':
      next = value + step;
      break;
    case 'ArrowDown':
    case 'ArrowLeft':
      next = value - step;
      break;
    case 'PageUp':
      next = value + page;
      break;
    case 'PageDown':
      next = value - page;
      break;
    case 'Home':
      next = domain.min;
      break;
    case 'End':
      next = domain.max;
      break;
    default:
      return null;
  }
  return clampToDomain(next, domain);
}

/**
 * Numeric entry parser — accepts plain numbers, optional leading sign,
 * surrounding whitespace and a trailing unit suffix the caller strips.
 * Returns null for unparseable input (caller keeps the previous value).
 */
export function parseNumericEntry(text: string): number | null {
  const trimmed = text.trim();
  if (trimmed === '') return null;
  const n = Number(trimmed);
  if (!Number.isFinite(n)) return null;
  return n;
}

/** Format a value for display / numeric entry seeding (trimmed decimals). */
export function formatNumericValue(value: number, digits = 2): string {
  const fixed = value.toFixed(digits);
  return fixed.replace(/\.?0+$/, '');
}

// ---------------------------------------------------------------------------
// gesture session: one gesture = one undo transaction
// ---------------------------------------------------------------------------

export interface GestureSession {
  readonly startValue: number;
  readonly currentValue: number;
  readonly active: boolean;
}

export function beginGesture(startValue: number): GestureSession {
  return { startValue, currentValue: startValue, active: true };
}

/** Update the preview value mid-gesture (already clamped by caller). */
export function updateGesture(session: GestureSession, value: number): GestureSession {
  if (!session.active) return session;
  return { ...session, currentValue: value };
}

/**
 * Complete a gesture. Returns the single undo transaction {from,to} when the
 * value actually changed, or null — a no-op gesture must never emit an undo
 * step or a commit callback.
 */
export function commitGesture(session: GestureSession): { from: number; to: number } | null {
  if (!session.active) return null;
  if (session.currentValue === session.startValue) return null;
  return { from: session.startValue, to: session.currentValue };
}

/** Cancel a gesture — the previewed value is discarded. */
export function cancelGesture(session: GestureSession): GestureSession {
  return { ...session, active: false };
}

// ---------------------------------------------------------------------------
// Escape ordering: active gesture/preview consumes Escape before chrome
// ---------------------------------------------------------------------------

type EscapeHandler = () => void;

/**
 * Ordered stack of Escape consumers. An active gesture pushes itself on top;
 * the shell's global Escape handler calls consumeEscape() first — when the
 * stack eats the event, panels/menus/drawers must stay open.
 */
export interface EscapeStack {
  push(handler: EscapeHandler): () => void;
  consumeEscape(): boolean;
  depth(): number;
}

export function makeEscapeStack(): EscapeStack {
  const stack: EscapeHandler[] = [];
  return {
    push(handler) {
      stack.push(handler);
      let popped = false;
      return () => {
        if (popped) return;
        popped = true;
        const i = stack.lastIndexOf(handler);
        if (i >= 0) stack.splice(i, 1);
      };
    },
    consumeEscape() {
      const top = stack[stack.length - 1];
      if (!top) return false;
      stack.pop();
      top();
      return true;
    },
    depth() {
      return stack.length;
    },
  };
}

/** Shared stack for the running app — components register gesture cancels. */
export const globalEscapeStack: EscapeStack = makeEscapeStack();

// ---------------------------------------------------------------------------
// text-input guard: keyboard/IME entry must never trigger transport/global keys
// ---------------------------------------------------------------------------

export interface EditableLike {
  tagName?: string;
  isContentEditable?: boolean;
  /** input[type=…] — range/checkbox are NOT text editable. */
  type?: string;
}

const TEXT_INPUT_TYPES = new Set([
  'text',
  'search',
  'number',
  'email',
  'password',
  'tel',
  'url',
]);

/** True when the event target accepts typed text or is mid-IME composition. */
export function isEditableTarget(target: EditableLike | null | undefined): boolean {
  if (!target) return false;
  if (target.isContentEditable) return true;
  const tag = (target.tagName ?? '').toLowerCase();
  if (tag === 'textarea') return true;
  if (tag === 'input') {
    const t = (target.type ?? 'text').toLowerCase();
    return TEXT_INPUT_TYPES.has(t);
  }
  if (tag === 'select') return true;
  return false;
}

export interface KeyEventLike {
  target?: EditableLike | null;
  isComposing?: boolean;
  /** DOM KeyboardEvent.isComposing equivalent. */
  nativeIsComposing?: boolean;
}

/**
 * True when a global shortcut (transport space, workspace digit, Escape for
 * chrome…) may fire for this event. Returns false for text/IME input so
 * typing in a field never triggers transport.
 */
export function mayUseGlobalShortcut(ev: KeyEventLike): boolean {
  if (ev.isComposing || ev.nativeIsComposing) return false;
  if (isEditableTarget(ev.target)) return false;
  return true;
}
