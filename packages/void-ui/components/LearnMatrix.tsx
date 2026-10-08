import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';
import { Button } from './Button';

export interface MappingRow {
  mappingId: string;
  /** e.g. "CC 20 ch0" or "Touch Y". */
  controlLabel: string;
  /** e.g. "Filter.cutoff" or macro name. */
  targetLabel: string;
  /** true while this mapping's target is armed (only it responds). */
  armed: boolean;
  curve?: string;
  range?: string;
}

export interface LearnMatrixProps {
  rows: MappingRow[];
  /** Target currently waiting for a control (learn capture in progress). */
  learningTargetLabel?: string | null;
  onArm?: (mappingId: string) => void;
  onRemove?: (mappingId: string) => void;
  onBeginLearn?: () => void;
  onCancelLearn?: () => void;
  /** PANIC — releases held controls + disarms. Always enabled. */
  onPanic?: () => void;
  'aria-label'?: string;
}

/**
 * MIDI-learn / macro mapping table — presentational. The learn store
 * owns arming, capture, releaseAll and panic; this renders the truth:
 * which control drives which target, which one is armed, and a PANIC
 * button that is always reachable (T58: panic/release has priority).
 */
export const LearnMatrix: React.FC<LearnMatrixProps> = React.memo(
  ({
    rows,
    learningTargetLabel,
    onArm,
    onRemove,
    onBeginLearn,
    onCancelLearn,
    onPanic,
    'aria-label': ariaLabel = 'Control mappings',
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    return (
      <div
        role="region"
        aria-label={ariaLabel}
        style={{
          border: `1px solid ${tokens.border}`,
          borderRadius: tokens.radius,
          background: tokens.surface,
          padding: 8,
        }}
      >
        <div style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
          {learningTargetLabel ? (
            <>
              <span
                aria-live="assertive"
                style={{
                  fontFamily: tokens.mono,
                  fontSize: 11,
                  color: tokens.warn,
                  alignSelf: 'center',
                }}
              >
                Move a control to bind: {learningTargetLabel}
              </span>
              <Button variant="ghost" onClick={onCancelLearn} aria-label="Cancel learn">
                Cancel
              </Button>
            </>
          ) : (
            <Button variant="subtle" onClick={onBeginLearn} aria-label="Learn a new mapping">
              + Learn
            </Button>
          )}
          <span style={{ flex: 1 }} />
          <Button
            variant="danger"
            onClick={onPanic}
            aria-label="Panic: release all controls and disarm"
          >
            PANIC
          </Button>
        </div>
        {rows.length === 0 ? (
          <p
            style={{
              fontFamily: tokens.mono,
              fontSize: 11,
              color: tokens.textMuted,
              margin: '8px 0',
            }}
          >
            No mappings. Press + Learn, then move a control.
          </p>
        ) : (
          <table
            style={{
              width: '100%',
              borderCollapse: 'collapse',
              fontFamily: tokens.mono,
              fontSize: 11,
            }}
          >
            <thead>
              <tr>
                {['Control', 'Target', 'Curve', 'Range', '', ''].map((h) => (
                  <th
                    key={h}
                    scope="col"
                    style={{
                      textAlign: 'left',
                      color: tokens.textSecondary,
                      fontWeight: 500,
                      padding: '2px 6px',
                      borderBottom: `1px solid ${tokens.border}`,
                    }}
                  >
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.mappingId}>
                  <td style={{ padding: '4px 6px', color: tokens.text }}>{row.controlLabel}</td>
                  <td style={{ padding: '4px 6px', color: tokens.text }}>{row.targetLabel}</td>
                  <td style={{ padding: '4px 6px', color: tokens.textSecondary }}>
                    {row.curve ?? 'linear'}
                  </td>
                  <td style={{ padding: '4px 6px', color: tokens.textSecondary }}>
                    {row.range ?? '0..1'}
                  </td>
                  <td style={{ padding: '4px 6px' }}>
                    <Button
                      variant={row.armed ? 'primary' : 'ghost'}
                      onClick={() => onArm?.(row.mappingId)}
                      aria-pressed={row.armed}
                      aria-label={`${row.armed ? 'Disarm' : 'Arm'} ${row.targetLabel}`}
                      style={{ padding: '2px 8px', fontSize: 10 }}
                    >
                      {row.armed ? 'ARMED' : 'Arm'}
                    </Button>
                  </td>
                  <td style={{ padding: '4px 6px' }}>
                    <Button
                      variant="ghost"
                      onClick={() => onRemove?.(row.mappingId)}
                      aria-label={`Remove mapping ${row.controlLabel} to ${row.targetLabel}`}
                      style={{ padding: '2px 8px', fontSize: 10 }}
                    >
                      ×
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    );
  },
);
