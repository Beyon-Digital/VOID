// Mix screen — honest placeholder (same content the legacy shell showed):
// channel strips/meters land with the dedicated mix lane. The developer
// surface carries live gain/pan/meters today.

import * as React from 'react';
import { tokens } from 'void-ui';

export default function MixScreen() {
  return (
    <div
      role="status"
      style={{
        flex: 1,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: tokens.space24,
      }}
    >
      <p
        className="void-type-body"
        style={{ color: tokens.subtle, maxWidth: 420, textAlign: 'center' }}
      >
        mix workspace — channel strips and meters land with the dedicated mix
        lane; the developer surface (header toggle) carries live gain, pan and
        meters today
      </p>
    </div>
  );
}
