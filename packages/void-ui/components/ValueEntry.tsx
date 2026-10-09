import React from 'react';
import { formatNumericValue, parseNumericEntry, clampToDomain, type ValueDomain } from './editing';
import { tokens } from './styles';

export interface ValueEntryProps {
  initialValue: number;
  domain: ValueDomain;
  unit?: string;
  'aria-label': string;
  /** Commit a parsed+clamped entry — becomes one undo transaction upstream. */
  onCommit: (value: number) => void;
  /** Escape / unparseable-blur exit — keeps the previous value. */
  onCancel: () => void;
  width?: number;
}

/**
 * Inline numeric entry for the continuous controls. Enter commits (clamped),
 * Escape cancels, blur commits when changed and cancels when the text is
 * unparseable — never swallows a keystroke silently.
 */
export const ValueEntry: React.FC<ValueEntryProps> = ({
  initialValue,
  domain,
  unit,
  onCommit,
  onCancel,
  width = 56,
  ...rest
}) => {
  const [text, setText] = React.useState(formatNumericValue(initialValue));
  const inputRef = React.useRef<HTMLInputElement>(null);

  React.useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  const commit = () => {
    const n = parseNumericEntry(text);
    if (n === null) {
      onCancel();
      return;
    }
    onCommit(clampToDomain(n, domain));
  };

  return (
    <span style={{ display: 'inline-flex', alignItems: 'baseline', gap: 2 }}>
      <input
        ref={inputRef}
        aria-label={rest['aria-label']}
        inputMode="decimal"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === 'Enter') {
            e.preventDefault();
            commit();
          } else if (e.key === 'Escape') {
            e.preventDefault();
            onCancel();
          }
        }}
        style={{
          width,
          background: tokens.ink,
          color: tokens.text,
          border: `1px solid ${tokens.accent}`,
          borderRadius: tokens.radius4,
          fontFamily: tokens.fontNumeric,
          fontSize: 11,
          padding: '1px 4px',
          textAlign: 'right',
          outline: 'none',
        }}
      />
      {unit ? (
        <span aria-hidden style={{ fontSize: 9, color: tokens.subtle, fontFamily: tokens.fontNumeric }}>
          {unit}
        </span>
      ) : null}
    </span>
  );
};
