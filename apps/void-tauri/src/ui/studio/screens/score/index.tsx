// S24 — Score / notation workspace (figma 4:2308, UI-T34).
//
// The Figma S24 is a layout starting point only; this screen renders real
// engine notes through the internal derive pipeline:
//
//   parts        ← TRACK_LIST items (ScorePart rows)
//   clips        ← CLIP_LIST(track_id) per part via ClipEditor
//   notes        ← NOTE_RANGE per MIDI clip via NoteEditor (paged, merged
//                  into the bounded view cache); element id == engine
//                  note id — the SAME ids the piano roll selects by
//   layout       ← layoutScore: meter + display quantization are view
//                  settings only; raw performance ticks are never
//                  rewritten (UI-T34)
//   selection    ⇄ editorStore.noteSelection + studioStore.selection —
//                  the identical actions the piano roll calls, so a note
//                  picked here is selected in Compose and vice versa
//   edits        ← InsertNoteOp / SetNoteOp / RemoveNoteOp via
//                  NoteEditor (sendWithStaleRetry, one gesture = one
//                  transactionId) — not score ops; the score-op wire
//                  does not exist in protocol major.1
//   export       ← exportMusicXml emits the real ticks + engine ids —
//                  the file save itself is a browser download; the
//                  crate-side MUSICXML writer remains coordinator-only
//   gaps         ← ENGRAVING_AVAILABLE / SCORE_OPS_WIRE_AVAILABLE /
//                  playback — all shown honestly in the inspector

import * as React from 'react';
import { ActionButton, IconButton, StatusBadge, tokens } from 'void-ui';
import type { ReadItem } from 'void-client';
import {
  ClipEditor,
  DISPLAY_QUANTIZE_OPTIONS,
  ENGRAVING_AVAILABLE,
  exportMusicXml,
  MUSICXML_INTERCHANGE_AVAILABLE,
  NoteEditor,
  SCORE_METERS,
  SCORE_OPS_WIRE_AVAILABLE,
  TICKS_PER_QUARTER,
  editorStore,
  loadViewPage,
  makeViewKey,
  noteEditIntent,
  parseClipItem,
  quantizeGridTicks,
  studioStore,
  useStudio,
  type ClipView,
  type DisplayQuantize,
  type NoteSegment,
  type NoteView,
  type ScoreMeter,
  type SpellingPref,
  meterLabel,
} from 'void-studio';
import { getClient } from '../../../client';
import {
  useEditor,
  useProjectSummary,
  useStudioCompactContext,
} from '../../useStudioData';
import {
  CLEF_W,
  MEASURE_WIDTH,
  MEASURES_PER_SYSTEM,
  SCORE_GAPS,
  STAFF_HEIGHT,
  STAFF_STEP_PX,
  SYSTEM_HEIGHT,
  deriveLayout,
  deriveScoreNotes,
  exportFileName,
  isMidiClip,
  ledgerSteps,
  midiForStaffStep,
  noteHeadY,
  offsetX,
  partsFromTrackItems,
  passageInfo,
  staffLineY,
  type ScorePart,
} from './model';

// -- editors (lazy singletons — same pattern as the piano roll) --------------

let _clipEd: ClipEditor | null = null;
const clipEd = () => (_clipEd ??= new ClipEditor(getClient(), () => crypto.randomUUID()));
let _noteEd: NoteEditor | null = null;
const noteEd = () => (_noteEd ??= new NoteEditor(getClient(), () => crypto.randomUUID()));

const STEP_NAME: Record<string, string> = {
  c: 'C', d: 'D', e: 'E', f: 'F', g: 'G', a: 'A', b: 'B',
};

function pitchLabel(s: NoteSegment): string {
  const acc = s.pitch.alter === 1 ? '♯' : s.pitch.alter === -1 ? '♭' : s.pitch.alter !== 0 ? `${s.pitch.alter}` : '';
  return `${STEP_NAME[s.pitch.step]}${acc}${s.pitch.octave}`;
}

// -- parts rail ---------------------------------------------------------------

function PartsRail(props: {
  parts: ScorePart[];
  tracksLoaded: boolean;
  activeTrackId: string | null;
  onPick: (trackId: string) => void;
  meter: ScoreMeter;
  onMeter: (m: ScoreMeter) => void;
  quantize: DisplayQuantize;
  onQuantize: (q: DisplayQuantize) => void;
  spelling: SpellingPref;
  onSpelling: (s: SpellingPref) => void;
  midiClipCount: (trackId: string) => number | null;
}) {
  const label: React.CSSProperties = {
    fontSize: 10,
    letterSpacing: 1.2,
    color: tokens.subtle,
    fontFamily: tokens.sans,
    textTransform: 'uppercase',
  };
  const rowLabel: React.CSSProperties = { fontSize: 11, color: tokens.muted, fontFamily: tokens.sans };
  const rowValue: React.CSSProperties = { fontSize: 11, color: tokens.text, fontFamily: tokens.sans };
  const selectStyle: React.CSSProperties = {
    background: tokens.raised,
    color: tokens.text,
    border: `1px solid ${tokens.line}`,
    borderRadius: tokens.radius4,
    fontSize: 11,
    fontFamily: tokens.sans,
    padding: '2px 6px',
    maxWidth: 92,
  };
  return (
    <div
      style={{
        width: 216,
        flexShrink: 0,
        display: 'flex',
        flexDirection: 'column',
        borderRight: `1px solid ${tokens.line}`,
        background: tokens.surface,
        padding: '12px 10px',
        gap: 10,
        overflowY: 'auto',
      }}
      aria-label="Score tools"
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <StatusBadge status="processing" label="Specialist · F5" />
      </div>
      <div style={{ ...label }}>Score tools</div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        <ActionButton variant="primary" size="sm" aria-pressed="true">
          Notation
        </ActionButton>
        <ActionButton
          variant="ghost"
          size="sm"
          onClick={() => {
            window.location.hash = '#/compose';
          }}
        >
          Piano roll
        </ActionButton>
      </div>
      <div style={{ ...label, marginTop: 6 }}>Parts</div>
      <div role="listbox" aria-label="Parts" style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
        {!props.tracksLoaded ? (
          <div style={rowLabel}>Loading tracks…</div>
        ) : props.parts.length === 0 ? (
          <div style={rowLabel}>No tracks in this project.</div>
        ) : (
          props.parts.map((p) => {
            const n = props.midiClipCount(p.trackId);
            const active = p.trackId === props.activeTrackId;
            return (
              <button
                key={p.trackId}
                role="option"
                aria-selected={active}
                onClick={() => props.onPick(p.trackId)}
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  gap: 6,
                  padding: '5px 8px',
                  borderRadius: tokens.radius4,
                  border: 'none',
                  background: active ? tokens.accentSoft : 'transparent',
                  color: active ? tokens.accent : tokens.text,
                  fontFamily: tokens.sans,
                  fontSize: 12,
                  cursor: 'pointer',
                  textAlign: 'left',
                }}
              >
                <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {p.name}
                </span>
                <span style={{ fontFamily: tokens.mono, fontSize: 10, color: tokens.subtle }}>
                  {n === null ? '…' : `${n} clip${n === 1 ? '' : 's'}`}
                </span>
              </button>
            );
          })
        )}
      </div>
      <div style={{ ...label, marginTop: 6 }}>Display</div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span style={rowLabel}>Key</span>
        <span style={rowValue} title="The wire carries no key signature yet">—</span>
      </div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span style={rowLabel}>Meter</span>
        <select
          aria-label="Display meter"
          value={meterLabel(props.meter)}
          onChange={(e) => {
            const m = SCORE_METERS.find((mm) => meterLabel(mm) === e.target.value);
            if (m) props.onMeter(m);
          }}
          style={selectStyle}
        >
          {SCORE_METERS.map((m) => (
            <option key={meterLabel(m)} value={meterLabel(m)}>
              {meterLabel(m)}
            </option>
          ))}
        </select>
      </div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span style={rowLabel}>Quantize view</span>
        <select
          aria-label="Display quantization"
          value={props.quantize}
          onChange={(e) => props.onQuantize(e.target.value as DisplayQuantize)}
          style={selectStyle}
        >
          {DISPLAY_QUANTIZE_OPTIONS.map((q) => (
            <option key={q} value={q}>
              {q === 'off' ? 'Off (as played)' : q}
            </option>
          ))}
        </select>
      </div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span style={rowLabel}>Spelling</span>
        <select
          aria-label="Accidental spelling"
          value={props.spelling}
          onChange={(e) => props.onSpelling(e.target.value as SpellingPref)}
          style={selectStyle}
        >
          <option value="sharps">Sharps</option>
          <option value="flats">Flats</option>
        </select>
      </div>
      <div style={{ flex: 1 }} />
      <p style={{ fontSize: 10, color: tokens.subtle, fontFamily: tokens.sans, margin: 0, lineHeight: 1.5 }}>
        Engraving is a view of editable music — these display settings never
        rewrite the performance.
      </p>
      <ActionButton
        variant="ghost"
        size="sm"
        onClick={() => {
          window.location.hash = '#/arrange';
        }}
      >
        Back to song
      </ActionButton>
    </div>
  );
}

// -- canvas ---------------------------------------------------------------------

const PAPER_PAD_X = 24;
const PAPER_PAD_TOP = 40;

interface SystemGeom {
  /** Absolute x of a measure within the page. */
  measureX: (mi: number) => number;
  /** y of the staff top for a measure. */
  measureY: (mi: number) => number;
}

function systemGeom(pageMode: boolean): SystemGeom {
  if (!pageMode) {
    return {
      measureX: (mi) => PAPER_PAD_X + mi * MEASURE_WIDTH,
      measureY: () => PAPER_PAD_TOP,
    };
  }
  return {
    measureX: (mi) => PAPER_PAD_X + (mi % MEASURES_PER_SYSTEM) * MEASURE_WIDTH,
    measureY: (mi) =>
      PAPER_PAD_TOP + Math.floor(mi / MEASURES_PER_SYSTEM) * SYSTEM_HEIGHT,
  };
}

function ScoreCanvas(props: {
  partName: string;
  projectName: string | null;
  layout: ReturnType<typeof deriveLayout>;
  loading: boolean;
  attached: boolean;
  dropped: number;
  quantize: DisplayQuantize;
  pageMode: boolean;
  onPageMode: (v: boolean) => void;
  selectedIds: ReadonlySet<string>;
  onPickNote: (seg: NoteSegment, additive: boolean) => void;
  onInsertAt: (measureIndex: number, xInMeasure: number, staffStep: number) => void;
  onExport: () => void;
  exportDisabledReason: string | null;
  canvasRef: React.RefObject<HTMLDivElement | null>;
  onKeyDown: (e: React.KeyboardEvent) => void;
}) {
  const { layout, pageMode } = props;
  const geom = systemGeom(pageMode);
  const measureCount = layout.measures.length;
  const systemCount = pageMode
    ? Math.max(1, Math.ceil(measureCount / MEASURES_PER_SYSTEM))
    : 1;
  const width = pageMode
    ? PAPER_PAD_X * 2 + MEASURES_PER_SYSTEM * MEASURE_WIDTH
    : PAPER_PAD_X * 2 + Math.max(1, measureCount) * MEASURE_WIDTH;
  const height =
    PAPER_PAD_TOP + systemCount * SYSTEM_HEIGHT + 24;

  const segX = (s: NoteSegment) =>
    geom.measureX(s.measureIndex) + offsetX(s.segOffsetTicks, layout.measureLenTicks);
  const segY = (s: NoteSegment) =>
    geom.measureY(s.measureIndex) + noteHeadY(s.staffStep);

  // Tie arcs: join each tieStart segment to its continuation.
  const ties: { x1: number; y1: number; x2: number; y2: number; key: string }[] = [];
  for (const m of layout.measures) {
    for (const s of m.segments) {
      if (!s.tieStart) continue;
      const next = layout.measures[s.measureIndex + 1]?.segments.find(
        (o) => o.noteId === s.noteId && o.tieStop,
      );
      if (!next) continue;
      ties.push({
        x1: segX(s) + 6,
        y1: segY(s) + (s.staffStep < 4 ? 4 : -4),
        x2: segX(next) - 6,
        y2: segY(next) + (next.staffStep < 4 ? 4 : -4),
        key: `${s.noteId}:${s.measureIndex}`,
      });
    }
  }

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          padding: '8px 14px',
          borderBottom: `1px solid ${tokens.line}`,
        }}
      >
        <span style={{ fontFamily: tokens.fontDisplay, fontSize: 15, color: tokens.text }}>
          Score
        </span>
        <span style={{ fontSize: 12, color: tokens.muted, fontFamily: tokens.sans }}>
          {props.partName}
        </span>
        <div style={{ flex: 1 }} />
        <ActionButton
          variant={pageMode ? 'secondary' : 'ghost'}
          size="sm"
          aria-pressed={pageMode}
          onClick={() => props.onPageMode(true)}
        >
          Page view
        </ActionButton>
        <ActionButton
          variant={pageMode ? 'ghost' : 'secondary'}
          size="sm"
          aria-pressed={!pageMode}
          onClick={() => props.onPageMode(false)}
        >
          Scroll
        </ActionButton>
        <ActionButton
          variant="secondary"
          size="sm"
          onClick={props.onExport}
          disabled={props.exportDisabledReason !== null}
          title={props.exportDisabledReason ?? 'Download MusicXML for this part'}
        >
          Export score
        </ActionButton>
      </div>
      <div
        ref={props.canvasRef}
        role="region"
        aria-label="Score canvas"
        tabIndex={0}
        onKeyDown={props.onKeyDown}
        style={{ flex: 1, overflow: 'auto', outline: 'none', background: tokens.bg }}
      >
        <svg
          width={width}
          height={height}
          style={{ display: 'block', margin: '16px auto' }}
          role="img"
          aria-label={`Derived score for ${props.partName}`}
        >
          {/* paper */}
          <rect
            x={0}
            y={0}
            width={width}
            height={height}
            rx={4}
            fill={tokens.surfaceRaised}
            stroke={tokens.line}
          />
          <text
            x={PAPER_PAD_X}
            y={22}
            fill={tokens.text}
            fontSize={13}
            fontFamily="var(--void-font-display, 'Space Grotesk', 'Inter', system-ui, sans-serif)"
          >
            {props.projectName ?? 'Untitled project'} — {props.partName}
          </text>
          <text
            x={width - PAPER_PAD_X}
            y={22}
            textAnchor="end"
            fill={tokens.subtle}
            fontSize={9}
            fontFamily="var(--void-font-numeric, 'IBM Plex Mono', ui-monospace, monospace)"
          >
            internal layout · view {props.quantize === 'off' ? 'as played' : props.quantize}
          </text>
          {props.loading || measureCount === 0 ? (
            <text
              x={width / 2}
              y={PAPER_PAD_TOP + SYSTEM_HEIGHT / 2}
              textAnchor="middle"
              fill={tokens.muted}
              fontSize={12}
              fontFamily="var(--void-font-body, Inter, system-ui, sans-serif)"
            >
              {!props.attached
                ? 'Engine not attached — connect to a session to read notes.'
                : props.loading
                  ? 'Reading part…'
                  : 'No notes in this part yet — double-click inside a clip’s bars to write one.'}
            </text>
          ) : null}
          {layout.measures.map((m) => {
            const mx = geom.measureX(m.index);
            const my = geom.measureY(m.index);
            return (
              <g key={m.index}>
                {/* measure number */}
                <text
                  x={mx + 2}
                  y={my - 6}
                  fill={tokens.subtle}
                  fontSize={9}
                  fontFamily="var(--void-font-numeric, 'IBM Plex Mono', ui-monospace, monospace)"
                >
                  {m.index + 1}
                </text>
                {/* clef + time signature at system head */}
                {pageMode ? (
                  m.index % MEASURES_PER_SYSTEM === 0 ? (
                    <text x={mx + 4} y={my + STAFF_HEIGHT - 2} fontSize={30} fill={tokens.text}
                      aria-hidden="true" style={{ userSelect: 'none' }}>
                      {'\u{1D11E}'}
                    </text>
                  ) : null
                ) : m.index === 0 ? (
                  <text x={mx + 4} y={my + STAFF_HEIGHT - 2} fontSize={30} fill={tokens.text}
                    aria-hidden="true" style={{ userSelect: 'none' }}>
                    {'\u{1D11E}'}
                  </text>
                ) : null}
                {/* staff lines */}
                {[0, 1, 2, 3, 4].map((li) => (
                  <line
                    key={li}
                    x1={mx}
                    y1={my + staffLineY(li)}
                    x2={mx + MEASURE_WIDTH}
                    y2={my + staffLineY(li)}
                    stroke={tokens.line}
                    strokeWidth={1}
                  />
                ))}
                {/* barline + insert target */}
                <line
                  x1={mx + MEASURE_WIDTH}
                  y1={my}
                  x2={mx + MEASURE_WIDTH}
                  y2={my + STAFF_HEIGHT}
                  stroke={tokens.line}
                  strokeWidth={1}
                />
                <rect
                  x={mx}
                  y={my - 8}
                  width={MEASURE_WIDTH}
                  height={STAFF_HEIGHT + 24}
                  fill="transparent"
                  onDoubleClick={(e) => {
                    const rect = (e.currentTarget.ownerSVGElement as SVGSVGElement).getBoundingClientRect();
                    props.onInsertAt(m.index, e.clientX - rect.left - mx, Math.round((my + STAFF_HEIGHT - (e.clientY - rect.top)) / STAFF_STEP_PX));
                  }}
                />
                {m.empty ? (
                  <rect
                    x={mx + MEASURE_WIDTH / 2 - 6}
                    y={my + staffLineY(3) + 1}
                    width={12}
                    height={STAFF_STEP_PX}
                    rx={1.5}
                    fill={tokens.subtle}
                    aria-hidden="true"
                  />
                ) : null}
              </g>
            );
          })}
          {/* noteheads + stems + ledger lines */}
          {layout.measures.map((m) =>
            m.segments.map((s) => {
              const x = segX(s) + s.xShift * 7;
              const y = segY(s);
              const stemUp = s.staffStep < 4;
              const stemX = stemUp ? x + 5.5 : x - 5.5;
              const stemEndY = stemUp ? y - 36 : y + 36;
              const selected = props.selectedIds.has(s.noteId);
              const headW = 6;
              return (
                <g
                  key={`${s.noteId}:${s.measureIndex}`}
                  role="button"
                  tabIndex={-1}
                  aria-label={`${pitchLabel(s)} bar ${s.measureIndex + 1}${selected ? ' selected' : ''}`}
                  aria-pressed={selected}
                  style={{ cursor: 'pointer' }}
                  onClick={(e) => {
                    e.stopPropagation();
                    props.onPickNote(s, e.shiftKey || e.ctrlKey || e.metaKey);
                  }}
                >
                  {/* enlarged hit area */}
                  <rect x={x - 9} y={y - 7} width={18} height={14} fill="transparent" />
                  {ledgerSteps(s.staffStep).map((ls) => (
                    <line
                      key={ls}
                      x1={x - 9}
                      y1={segY({ ...s, staffStep: ls })}
                      x2={x + 9}
                      y2={segY({ ...s, staffStep: ls })}
                      stroke={tokens.line}
                      strokeWidth={1}
                    />
                  ))}
                  <line
                    x1={stemX}
                    y1={y}
                    x2={stemX}
                    y2={stemEndY}
                    stroke={selected ? tokens.accent : tokens.text}
                    strokeWidth={1.2}
                  />
                  <ellipse
                    cx={x}
                    cy={y}
                    rx={headW}
                    ry={4.2}
                    transform={`rotate(-14 ${x} ${y})`}
                    fill={selected ? tokens.accent : tokens.text}
                  />
                  {/* duration hint: filled vs hollow for >= half */}
                  {s.segDurationTicks >= layout.measureLenTicks / 2n ? (
                    <ellipse
                      cx={x}
                      cy={y}
                      rx={3.4}
                      ry={2.2}
                      transform={`rotate(-14 ${x} ${y})`}
                      fill={tokens.surfaceRaised}
                    />
                  ) : null}
                </g>
              );
            }),
          )}
          {/* ties */}
          {ties.map((t) => (
            <path
              key={t.key}
              d={`M ${t.x1} ${t.y1} Q ${(t.x1 + t.x2) / 2} ${t.y1 + 8} ${t.x2} ${t.y2}`}
              fill="none"
              stroke={tokens.textSecondary}
              strokeWidth={1}
            />
          ))}
        </svg>
        {props.dropped > 0 ? (
          <p style={{ fontSize: 10, color: tokens.warn, fontFamily: tokens.sans, margin: '0 16px 12px' }}>
            {props.dropped} read item{props.dropped === 1 ? '' : 's'} could not be parsed — skipped.
          </p>
        ) : null}
      </div>
    </div>
  );
}

// -- inspector ------------------------------------------------------------------

function InspectorRow({ k, v }: { k: string; v: string }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', padding: '3px 0' }}>
      <span style={{ fontSize: 11, color: tokens.muted, fontFamily: tokens.sans }}>{k}</span>
      <span style={{ fontSize: 11, color: tokens.text, fontFamily: tokens.sans }}>{v}</span>
    </div>
  );
}

function InspectorPane(props: {
  partName: string;
  passage: { bars: string | null; count: number };
  quantize: DisplayQuantize;
  onQuantize: (q: DisplayQuantize) => void;
  selectedDetail: { pitch: string; onsetTicks: string; durTicks: string; clipId: string } | null;
  editError: string | null;
}) {
  const label: React.CSSProperties = {
    fontSize: 10,
    letterSpacing: 1.2,
    color: tokens.subtle,
    fontFamily: tokens.sans,
    textTransform: 'uppercase',
  };
  return (
    <div
      style={{
        width: 232,
        flexShrink: 0,
        borderLeft: `1px solid ${tokens.line}`,
        background: tokens.surface,
        padding: '12px 12px',
        display: 'flex',
        flexDirection: 'column',
        gap: 10,
        overflowY: 'auto',
      }}
      aria-label="Notation inspector"
    >
      <div style={label}>Notation</div>
      <div style={{ fontSize: 12, color: tokens.text, fontFamily: tokens.sans }}>
        Selected passage
      </div>
      <div>
        <InspectorRow k="Part" v={props.partName} />
        <InspectorRow k="Bars" v={props.passage.bars ?? '—'} />
        <InspectorRow k="Notes" v={props.passage.count > 0 ? `${props.passage.count}` : '—'} />
        <InspectorRow k="Voice" v="1" />
      </div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span style={{ fontSize: 11, color: tokens.muted, fontFamily: tokens.sans }}>
          Quantize view
        </span>
        <select
          aria-label="Display quantization (inspector)"
          value={props.quantize}
          onChange={(e) => props.onQuantize(e.target.value as DisplayQuantize)}
          style={{
            background: tokens.raised,
            color: tokens.text,
            border: `1px solid ${tokens.line}`,
            borderRadius: tokens.radius4,
            fontSize: 11,
            fontFamily: tokens.sans,
            padding: '2px 6px',
          }}
        >
          {DISPLAY_QUANTIZE_OPTIONS.map((q) => (
            <option key={q} value={q}>
              {q === 'off' ? 'Off (as played)' : q}
            </option>
          ))}
        </select>
      </div>
      {props.selectedDetail ? (
        <div style={{ borderTop: `1px solid ${tokens.line}`, paddingTop: 8 }}>
          <div style={label}>Note</div>
          <InspectorRow k="Pitch" v={props.selectedDetail.pitch} />
          <InspectorRow k="Onset" v={props.selectedDetail.onsetTicks} />
          <InspectorRow k="Length" v={props.selectedDetail.durTicks} />
          <InspectorRow k="Clip" v={props.selectedDetail.clipId} />
        </div>
      ) : (
        <p style={{ fontSize: 11, color: tokens.subtle, fontFamily: tokens.sans, margin: 0 }}>
          Click a note to select it — the same selection shows in the piano roll.
        </p>
      )}
      <ActionButton
        variant="secondary"
        size="sm"
        onClick={() => {
          window.location.hash = '#/compose';
        }}
      >
        Edit notes
      </ActionButton>
      <div style={{ borderTop: `1px solid ${tokens.line}`, paddingTop: 8 }}>
        <div style={label}>Wire gaps</div>
        {!ENGRAVING_AVAILABLE ? (
          <p style={{ fontSize: 10, color: tokens.subtle, fontFamily: tokens.sans, margin: '4px 0' }}>
            {SCORE_GAPS.engraving}
          </p>
        ) : null}
        {!SCORE_OPS_WIRE_AVAILABLE ? (
          <p style={{ fontSize: 10, color: tokens.subtle, fontFamily: tokens.sans, margin: '4px 0' }}>
            {SCORE_GAPS.scoreOps}
          </p>
        ) : null}
        <p style={{ fontSize: 10, color: tokens.subtle, fontFamily: tokens.sans, margin: '4px 0' }}>
          {MUSICXML_INTERCHANGE_AVAILABLE ? SCORE_GAPS.exportView : 'MusicXML export unavailable.'}
        </p>
        <p style={{ fontSize: 10, color: tokens.subtle, fontFamily: tokens.sans, margin: '4px 0' }}>
          {SCORE_GAPS.playback}
        </p>
      </div>
      {props.editError ? (
        <p role="alert" style={{ fontSize: 11, color: tokens.danger, fontFamily: tokens.sans, margin: 0 }}>
          {props.editError}
        </p>
      ) : null}
    </div>
  );
}

// -- screen -----------------------------------------------------------------------

export default function ScoreScreen() {
  const compact = useStudioCompactContext();
  const summary = useProjectSummary();
  const views = useStudio((s) => s.views);
  const attached = useStudio((s) => s.engine.attached);
  const noteSel = useEditor((s) => s.noteSelection);
  const snap = useEditor((s) => s.snap);
  const editError = useEditor((s) => s.lastEditError);

  // display-only settings — never written to the engine
  const [meter, setMeter] = React.useState<ScoreMeter>({ beatsPerBar: 4, beatUnit: 4 });
  const [quantize, setQuantize] = React.useState<DisplayQuantize>('1/16');
  const [spelling, setSpelling] = React.useState<SpellingPref>('sharps');
  const [pageMode, setPageMode] = React.useState(true);
  const [leftOpen, setLeftOpen] = React.useState(false);
  const [rightOpen, setRightOpen] = React.useState(false);

  const trackEntry = views[makeViewKey('TRACK_LIST')];
  const parts = React.useMemo(
    () => (trackEntry ? partsFromTrackItems(trackEntry.items as ReadItem[]) : []),
    [trackEntry],
  );

  const [activeTrackId, setActiveTrackId] = React.useState<string | null>(null);
  const resolvedTrackId = React.useMemo(() => {
    if (activeTrackId && parts.some((p) => p.trackId === activeTrackId)) return activeTrackId;
    return parts[0]?.trackId ?? null;
  }, [activeTrackId, parts]);
  const activePart = parts.find((p) => p.trackId === resolvedTrackId) ?? null;

  // clips + notes for the active part (loaded through the same editors the
  // piano roll uses — pages also merge into the bounded view cache)
  const [clips, setClips] = React.useState<ClipView[]>([]);
  const [notes, setNotes] = React.useState<NoteView[]>([]);
  const [dropped, setDropped] = React.useState(0);
  const [loading, setLoading] = React.useState(false);
  const [clipCounts, setClipCounts] = React.useState<Map<string, number>>(new Map());

  React.useEffect(() => {
    void loadViewPage(studioStore, getClient(), 'TRACK_LIST').catch(() => undefined);
  }, []);

  const loadPart = React.useCallback(async (trackId: string) => {
    setLoading(true);
    try {
      const { clips: cls } = await clipEd().loadTrackClips(studioStore, trackId);
      setClipCounts((m) => new Map(m).set(trackId, cls.filter(isMidiClip).length));
      setClips(cls);
      const midi = cls.filter(isMidiClip);
      const all: NoteView[] = [];
      let drop = 0;
      for (const c of midi) {
        const r = await noteEd().loadClipNotes(studioStore, c);
        all.push(...r.notes);
        drop += r.dropped;
      }
      setNotes(all);
      setDropped(drop);
    } catch (e) {
      editorStore.getState().actions.setEditError(
        e instanceof Error ? e.message : 'Failed to read part',
      );
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    setClips([]);
    setNotes([]);
    setDropped(0);
    if (resolvedTrackId) void loadPart(resolvedTrackId);
  }, [resolvedTrackId, loadPart]);

  const scoreNotes = React.useMemo(
    () => deriveScoreNotes(notes, clips),
    [notes, clips],
  );
  const layout = React.useMemo(
    () => deriveLayout(scoreNotes.notes, meter, quantize, spelling),
    [scoreNotes, meter, quantize, spelling],
  );

  const selectedIds = React.useMemo(
    () => new Set(noteSel.noteIds),
    [noteSel.noteIds],
  );

  const refreshClip = React.useCallback(
    (clipId: string) => {
      const c = clips.find((x) => x.clipId === clipId);
      if (!c) return;
      return noteEd().loadClipNotes(studioStore, c).then((r) => {
        setNotes((prev) => [...prev.filter((n) => n.clipId !== clipId), ...r.notes]);
        setDropped((d) => d + r.dropped);
      });
    },
    [clips],
  );

  const pickNote = React.useCallback(
    (seg: NoteSegment, additive: boolean) => {
      const a = editorStore.getState().actions;
      if (additive) a.toggleNote(seg.clipId, seg.noteId);
      else a.selectNotes(seg.clipId, [seg.noteId]);
      if (resolvedTrackId) {
        studioStore.getState().actions.selectNote(resolvedTrackId, seg.clipId, seg.noteId);
      }
    },
    [resolvedTrackId],
  );

  // -- editing (real ops through the note ops wire) -------------------------

  const canvasRef = React.useRef<HTMLDivElement | null>(null);

  const applyKey = React.useCallback(
    (e: React.KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (
        t instanceof HTMLInputElement ||
        t instanceof HTMLTextAreaElement ||
        t instanceof HTMLSelectElement ||
        (e.nativeEvent as KeyboardEvent).isComposing
      ) {
        return;
      }
      if (e.key === 'Escape') {
        editorStore.getState().actions.clearNoteSelection();
        if (resolvedTrackId && noteSel.clipId) {
          studioStore.getState().actions.selectNote(resolvedTrackId, noteSel.clipId, null);
        }
        e.stopPropagation();
        return;
      }
      const intent = noteEditIntent(e.key, { shift: e.shiftKey, alt: e.altKey }, snap);
      if (intent.type === 'noop') return;
      if (!noteSel.clipId || noteSel.noteIds.length === 0) return;
      e.preventDefault();
      const clipId = noteSel.clipId;
      const sel = new Set(noteSel.noteIds);
      const target = notes.filter((n) => n.clipId === clipId && sel.has(n.noteId));
      if (target.length === 0) return;
      const tx = crypto.randomUUID();
      void noteEd()
        .applyIntent(clipId, target, intent, tx, () => refreshClip(clipId))
        .then((results) => {
          const fail = results.find((r) => r.errorText);
          editorStore.getState().actions.setEditError(fail?.errorText ?? null);
        });
    },
    [snap, noteSel, notes, resolvedTrackId, refreshClip],
  );

  const insertAt = React.useCallback(
    (measureIndex: number, xInMeasure: number, staffStep: number) => {
      if (!resolvedTrackId) return;
      const mLen = layout.measureLenTicks;
      const frac = Math.min(1, Math.max(0, (xInMeasure - CLEF_W) / (MEASURE_WIDTH - CLEF_W - 8)));
      const absTicks = BigInt(measureIndex) * mLen + BigInt(Math.round(frac * Number(mLen)));
      const grid = quantizeGridTicks(quantize);
      const snapped = grid
        ? BigInt(Math.round(Number(absTicks) / Number(grid))) * grid
        : absTicks;
      const clip = clips.find(
        (c) =>
          isMidiClip(c) &&
          snapped >= BigInt(c.startTicks) &&
          snapped < BigInt(c.startTicks) + BigInt(c.lengthTicks),
      );
      if (!clip) {
        editorStore.getState().actions.setEditError(
          'Notes live inside clips — extend or create a MIDI clip in Arrange first.',
        );
        return;
      }
      const midi = Math.min(127, Math.max(0, midiForStaffStep(staffStep)));
      const len = quantizeGridTicks(quantize) ?? TICKS_PER_QUARTER;
      const tx = crypto.randomUUID();
      void noteEd()
        .insertNote(clip.clipId, midi, 100, snapped.toString(10), len.toString(10), tx, () => refreshClip(clip.clipId))
        .then((r) => {
          editorStore.getState().actions.setEditError(r.errorText ?? null);
        });
    },
    [resolvedTrackId, layout.measureLenTicks, quantize, clips, refreshClip],
  );

  // -- export ---------------------------------------------------------------

  const exportScore = React.useCallback(() => {
    if (!activePart || scoreNotes.notes.length === 0) return;
    const xml = exportMusicXml({
      name: activePart.name,
      meter,
      notes: scoreNotes.notes,
      spelling,
    });
    const blob = new Blob([xml], { type: 'application/vnd.recordare.musicxml+xml' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = exportFileName(activePart.name);
    a.click();
    URL.revokeObjectURL(url);
  }, [activePart, scoreNotes, meter, spelling]);

  const exportDisabledReason = !MUSICXML_INTERCHANGE_AVAILABLE
    ? 'MusicXML writer is not available in this build'
    : scoreNotes.notes.length === 0
      ? 'No notes in this part to export'
      : null;

  // -- selection summary for the inspector -----------------------------------

  const passage = React.useMemo(
    () => passageInfo(noteSel.noteIds, layout),
    [noteSel.noteIds, layout],
  );

  const selectedDetail = React.useMemo(() => {
    if (noteSel.noteIds.length === 0) return null;
    const id = noteSel.noteIds[noteSel.noteIds.length - 1];
    const seg = layout.measures
      .flatMap((m) => m.segments)
      .find((s) => s.noteId === id && !s.tieStop);
    if (!seg) return null;
    return {
      pitch: pitchLabel(seg),
      onsetTicks: seg.rawStartTicks.toString(10),
      durTicks: seg.rawDurationTicks.toString(10),
      clipId: seg.clipId,
    };
  }, [noteSel.noteIds, layout]);

  const midiClipCountFor = React.useCallback(
    (trackId: string) => clipCounts.get(trackId) ?? null,
    [clipCounts],
  );

  // Lazily resolve clip counts for rail labels of other parts.
  React.useEffect(() => {
    for (const p of parts) {
      if (clipCounts.has(p.trackId)) continue;
      const key = makeViewKey('CLIP_LIST', p.trackId, undefined, undefined);
      const entry = views[key];
      if (!entry) continue;
      const n = (entry.items as ReadItem[])
        .map(parseClipItem)
        .filter((c): c is ClipView => c !== null && isMidiClip(c)).length;
      setClipCounts((m) => (m.has(p.trackId) ? m : new Map(m).set(p.trackId, n)));
    }
  }, [parts, views, clipCounts]);

  const rail = (
    <PartsRail
      parts={parts}
      tracksLoaded={!!trackEntry}
      activeTrackId={resolvedTrackId}
      onPick={setActiveTrackId}
      meter={meter}
      onMeter={setMeter}
      quantize={quantize}
      onQuantize={setQuantize}
      spelling={spelling}
      onSpelling={setSpelling}
      midiClipCount={midiClipCountFor}
    />
  );
  const inspector = (
    <InspectorPane
      partName={activePart?.name ?? '—'}
      passage={passage}
      quantize={quantize}
      onQuantize={setQuantize}
      selectedDetail={selectedDetail}
      editError={editError}
    />
  );
  const canvas = (
    <ScoreCanvas
      partName={activePart?.name ?? '—'}
      projectName={summary.name}
      layout={layout}
      loading={loading}
      attached={attached}
      dropped={dropped + scoreNotes.dropped}
      quantize={quantize}
      pageMode={pageMode}
      onPageMode={setPageMode}
      selectedIds={selectedIds}
      onPickNote={pickNote}
      onInsertAt={insertAt}
      onExport={exportScore}
      exportDisabledReason={exportDisabledReason}
      canvasRef={canvasRef}
      onKeyDown={applyKey}
    />
  );

  if (!compact) {
    return (
      <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0 }}>
        {rail}
        {canvas}
        {inspector}
      </div>
    );
  }

  const drawerBase: React.CSSProperties = {
    position: 'absolute',
    top: 0,
    bottom: 0,
    zIndex: 30,
    display: 'flex',
    background: tokens.surface,
    boxShadow: `4px 0 16px ${tokens.ink}`,
  };
  return (
    <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0, position: 'relative' }}>
      <div style={{ position: 'absolute', top: 8, left: 8, zIndex: 20, display: 'flex', gap: 6 }}>
        <IconButton
          icon="☰"
          aria-label="Open parts drawer"
          variant="solid"
          pressed={leftOpen}
          onClick={() => setLeftOpen((v) => !v)}
        />
        <IconButton
          icon="ⓘ"
          aria-label="Open inspector drawer"
          variant="solid"
          pressed={rightOpen}
          onClick={() => setRightOpen((v) => !v)}
        />
      </div>
      {canvas}
      {leftOpen ? (
        <div
          role="dialog"
          aria-label="Score tools"
          onKeyDown={(e) => {
            if (e.key === 'Escape') {
              e.stopPropagation();
              setLeftOpen(false);
            }
          }}
          style={{ ...drawerBase, left: 0, borderRight: `1px solid ${tokens.line}` }}
        >
          {rail}
        </div>
      ) : null}
      {rightOpen ? (
        <div
          role="dialog"
          aria-label="Notation inspector"
          onKeyDown={(e) => {
            if (e.key === 'Escape') {
              e.stopPropagation();
              setRightOpen(false);
            }
          }}
          style={{ ...drawerBase, right: 0, left: 'auto', borderLeft: `1px solid ${tokens.line}`, boxShadow: `-4px 0 16px ${tokens.ink}` }}
        >
          {inspector}
        </div>
      ) : null}
    </div>
  );
}
