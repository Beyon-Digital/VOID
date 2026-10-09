// Back-compat adapter — `ClipBlock` was the pre-Signal name; the canonical
// family is TimelineClip (audio/midi/ghost/selected as variants of one
// component). Same props; `kind` picks the variant.
import React from 'react';
import { TimelineClip, type TimelineClipProps } from './TimelineClip';

export interface ClipBlockProps extends Omit<TimelineClipProps, 'variant'> {
  /** Legacy callers pass kind text; variant derives from it. */
  kind?: string;
}

export const ClipBlock: React.FC<ClipBlockProps> = ({ kind, ...rest }) => (
  <TimelineClip variant={kind === 'midi' ? 'midi' : 'audio'} kind={kind} {...rest} />
);
