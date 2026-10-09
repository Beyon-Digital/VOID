import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface LoopItem {
  templateId: string;
  name: string;
  /** Musical bpm the template was authored at (display only — ticks are
   *  tempo-agnostic; previewLoop computes real duration at project tempo). */
  bpm?: number;
  meter?: string;
  tags?: string[];
}

export interface LoopBrowserProps {
  items: LoopItem[];
  /** Current filter text — the caller runs searchLoops. */
  query: string;
  selectedId?: string | null;
  onQuery?: (query: string) => void;
  onSelect?: (templateId: string) => void;
  onPreview?: (templateId: string) => void;
  'aria-label'?: string;
}

/**
 * Loop-template browser — listbox with a real search field. Every item
 * is keyboard selectable (arrows via listbox semantics + Enter applies).
 * Presentational: deterministic template lookup/preview lives in
 * void-studio patterns/loops (PAT-01).
 */
export const LoopBrowser: React.FC<LoopBrowserProps> = React.memo(
  ({
    items,
    query,
    selectedId,
    onQuery,
    onSelect,
    onPreview,
    'aria-label': ariaLabel = 'Loop browser',
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    return (
      <div
        style={{
          border: `1px solid ${tokens.border}`,
          borderRadius: tokens.radius,
          background: tokens.surface,
          padding: 8,
          maxWidth: 320,
        }}
      >
        <input
          type="search"
          value={query}
          onChange={(e) => onQuery?.(e.target.value)}
          placeholder="Search loops…"
          aria-label="Search loop templates"
          className={focusClass}
          style={{
            width: '100%',
            boxSizing: 'border-box',
            fontFamily: tokens.mono,
            fontSize: 12,
            padding: '4px 8px',
            marginBottom: 6,
            background: tokens.surfaceRaised,
            color: tokens.text,
            border: `1px solid ${tokens.border}`,
            borderRadius: tokens.radius,
          }}
        />
        <div role="listbox" aria-label={ariaLabel} aria-activedescendant={selectedId ?? undefined}>
          {items.length === 0 && (
            <p style={{ fontFamily: tokens.mono, fontSize: 11, color: tokens.textMuted }}>
              No templates match.
            </p>
          )}
          {items.map((item) => {
            const selected = item.templateId === selectedId;
            return (
              <button
                key={item.templateId}
                id={item.templateId}
                role="option"
                aria-selected={selected}
                onClick={() => onSelect?.(item.templateId)}
                onDoubleClick={() => onPreview?.(item.templateId)}
                className={`${focusClass} ${animatedClass}`}
                style={{
                  display: 'flex',
                  width: '100%',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  padding: '6px 8px',
                  marginBottom: 2,
                  border: `1px solid ${selected ? tokens.accent : 'transparent'}`,
                  borderRadius: tokens.radius,
                  background: selected ? tokens.accentSoft : 'transparent',
                  color: tokens.text,
                  fontFamily: tokens.mono,
                  fontSize: 11,
                  cursor: 'pointer',
                  textAlign: 'left',
                }}
              >
                <span>
                  {item.name}
                  {item.meter ? (
                    <span style={{ color: tokens.textMuted }}> · {item.meter}</span>
                  ) : null}
                </span>
                <span style={{ color: tokens.textSecondary, fontSize: 10 }}>
                  {item.bpm ? `${item.bpm}bpm` : ''}
                </span>
              </button>
            );
          })}
        </div>
      </div>
    );
  },
);
