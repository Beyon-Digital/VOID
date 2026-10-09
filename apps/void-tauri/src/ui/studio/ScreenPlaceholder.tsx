import * as React from 'react';
import { tokens } from 'void-ui';

/**
 * Honest unavailability: a routed screen id with no implementation yet shows
 * the id plainly. Never renders fake content or interactive affordances.
 */
export const ScreenPlaceholder: React.FC<{ screenId: string }> = ({ screenId }) => (
  <div
    role="status"
    aria-label={`Screen ${screenId} is not built yet`}
    style={{
      flex: 1,
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      gap: tokens.space8,
      color: tokens.subtle,
      fontFamily: tokens.sans,
      padding: tokens.space24,
    }}
  >
    <span className="void-type-title" style={{ color: tokens.muted }}>
      {screenId}
    </span>
    <span className="void-type-body">This screen is not built yet.</span>
  </div>
);
