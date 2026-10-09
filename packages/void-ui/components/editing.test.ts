import { describe, expect, it } from 'vitest';
import {
  applyValueKey,
  beginGesture,
  cancelGesture,
  clampToDomain,
  clampValue,
  commitGesture,
  formatNumericValue,
  isEditableTarget,
  makeEscapeStack,
  mayUseGlobalShortcut,
  parseNumericEntry,
  updateGesture,
  type ValueDomain,
} from './editing';

const domain: ValueDomain = { min: -60, max: 6, step: 1, fineStep: 0.1 };

describe('clampValue / clampToDomain', () => {
  it('clamps into the domain and maps NaN to min', () => {
    expect(clampValue(7, -60, 6)).toBe(6);
    expect(clampValue(-70, -60, 6)).toBe(-60);
    expect(clampValue(0, -60, 6)).toBe(0);
    expect(clampValue(NaN, -60, 6)).toBe(-60);
    expect(clampToDomain(99, domain)).toBe(6);
  });
});

describe('applyValueKey', () => {
  it('arrows step by the coarse step', () => {
    expect(applyValueKey(0, 'ArrowUp', {}, domain)).toBe(1);
    expect(applyValueKey(0, 'ArrowRight', {}, domain)).toBe(1);
    expect(applyValueKey(0, 'ArrowDown', {}, domain)).toBe(-1);
    expect(applyValueKey(0, 'ArrowLeft', {}, domain)).toBe(-1);
  });

  it('shift-held arrows use the fine step', () => {
    expect(applyValueKey(0, 'ArrowUp', { shiftKey: true }, domain)).toBeCloseTo(0.1);
    expect(applyValueKey(0, 'ArrowDown', { shiftKey: true }, domain)).toBeCloseTo(-0.1);
  });

  it('clamps at domain edges', () => {
    expect(applyValueKey(5.5, 'ArrowUp', {}, domain)).toBe(6);
    expect(applyValueKey(-59.5, 'ArrowDown', {}, domain)).toBe(-60);
  });

  it('home/end jump to domain bounds, page keys use 10x step', () => {
    expect(applyValueKey(0, 'Home', {}, domain)).toBe(-60);
    expect(applyValueKey(0, 'End', {}, domain)).toBe(6);
    expect(applyValueKey(-30, 'PageUp', {}, domain)).toBe(-20);
    expect(applyValueKey(-30, 'PageDown', {}, domain)).toBe(-40);
    expect(applyValueKey(0, 'PageUp', {}, domain)).toBe(6);
  });

  it('returns null for unhandled keys', () => {
    expect(applyValueKey(0, 'a', {}, domain)).toBeNull();
    expect(applyValueKey(0, 'Enter', {}, domain)).toBeNull();
  });
});

describe('parseNumericEntry / formatNumericValue', () => {
  it('parses plain numbers with whitespace and signs', () => {
    expect(parseNumericEntry('  -3.5 ')).toBe(-3.5);
    expect(parseNumericEntry('+2')).toBe(2);
    expect(parseNumericEntry('0')).toBe(0);
  });
  it('rejects empty and non-numeric input', () => {
    expect(parseNumericEntry('')).toBeNull();
    expect(parseNumericEntry('   ')).toBeNull();
    expect(parseNumericEntry('abc')).toBeNull();
    expect(parseNumericEntry('-')).toBeNull();
  });
  it('formats values trimming trailing zeros', () => {
    expect(formatNumericValue(-6.25)).toBe('-6.25');
    expect(formatNumericValue(6)).toBe('6');
    expect(formatNumericValue(0.5, 1)).toBe('0.5');
  });
});

describe('gesture session — one gesture = one undo transaction', () => {
  it('commit returns a single {from,to} after updates', () => {
    let g = beginGesture(0);
    g = updateGesture(g, 1);
    g = updateGesture(g, 2);
    g = updateGesture(g, -4);
    expect(commitGesture(g)).toEqual({ from: 0, to: -4 });
  });

  it('a no-move gesture commits nothing', () => {
    const g = beginGesture(3);
    expect(commitGesture(g)).toBeNull();
  });

  it('committing an inactive/cancelled gesture is a no-op', () => {
    let g = beginGesture(1);
    g = updateGesture(g, 5);
    g = cancelGesture(g);
    expect(commitGesture(g)).toBeNull();
  });

  it('updates after cancel are ignored', () => {
    let g = beginGesture(1);
    g = cancelGesture(g);
    g = updateGesture(g, 9);
    expect(g.currentValue).toBe(1);
    expect(commitGesture(g)).toBeNull();
  });
});

describe('escape stack — gesture consumes Escape before other UI', () => {
  it('runs the most recent consumer first and reports consumption', () => {
    const stack = makeEscapeStack();
    const order: string[] = [];
    stack.push(() => order.push('panel'));
    stack.push(() => order.push('gesture'));
    expect(stack.consumeEscape()).toBe(true);
    expect(order).toEqual(['gesture']);
    expect(stack.consumeEscape()).toBe(true);
    expect(order).toEqual(['gesture', 'panel']);
    expect(stack.consumeEscape()).toBe(false);
  });

  it('pop unsubscribe removes the handler', () => {
    const stack = makeEscapeStack();
    const calls: string[] = [];
    const pop = stack.push(() => calls.push('g'));
    pop();
    expect(stack.consumeEscape()).toBe(false);
    expect(calls).toEqual([]);
  });
});

describe('text/IME guard — typing never triggers global shortcuts', () => {
  it('editable targets swallow global keys', () => {
    expect(isEditableTarget({ tagName: 'input', type: 'text' })).toBe(true);
    expect(isEditableTarget({ tagName: 'input', type: 'number' })).toBe(true);
    expect(isEditableTarget({ tagName: 'textarea' })).toBe(true);
    expect(isEditableTarget({ tagName: 'div', isContentEditable: true })).toBe(true);
    expect(isEditableTarget({ tagName: 'select' })).toBe(true);
  });
  it('non-text controls and plain elements are not editable', () => {
    expect(isEditableTarget({ tagName: 'input', type: 'range' })).toBe(false);
    expect(isEditableTarget({ tagName: 'input', type: 'checkbox' })).toBe(false);
    expect(isEditableTarget({ tagName: 'div' })).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
  });
  it('IME composition blocks global shortcuts even on non-editable targets', () => {
    expect(mayUseGlobalShortcut({ isComposing: true })).toBe(false);
    expect(mayUseGlobalShortcut({ nativeIsComposing: true })).toBe(false);
    expect(mayUseGlobalShortcut({ target: { tagName: 'input', type: 'text' } })).toBe(false);
    expect(mayUseGlobalShortcut({ target: { tagName: 'div' } })).toBe(true);
  });
});
