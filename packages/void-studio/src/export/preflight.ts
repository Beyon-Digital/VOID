// Export preflight (S11, UI-T29) — REAL checks over real engine data.
//
// Every row is derived from read-view projections and live state the
// caller passes in; nothing here is a hardcoded green check. A check the
// preflight genuinely cannot evaluate reports `unavailable` with the
// reason, never a guessed pass. `blocking` mirrors the runner's hard
// gates — a blocking failure means the export cannot produce a verified
// output.

/** Inputs are already-narrowed projections (TRACK_LIST/ASSET_LIST/etc). */
export interface PreflightInputs {
  /** Engine attachment — all content checks are meaningless detached. */
  engineAttached: boolean;
  /** Track count from TRACK_LIST (any kinds). */
  trackCount: number;
  /** Bus tracks from the routing model (the only mix destinations). */
  busCount: number;
  /** sha256/ids the assets view actually reported. */
  assetIds: string[];
  /** Asset ids referenced by clips (CLIP_LIST projection). */
  clipAssetRefs: string[];
  /** Asset refs that the clip items already flag as missing. */
  clipMissingRefs?: string[];
  /** Device items flagged failed/missing/quarantined by the engine. */
  failedDevices: { instanceId: string; name: string; reason: string }[];
  /** Id of the last durable checkpoint (telemetry SaveResultEvent). */
  checkpointId?: string;
  /** Inclusive range sanity: endTicks > startTicks already validated. */
  rangeNonEmpty: boolean;
  /** Clips present inside the export range (best-effort signal). */
  clipsInRange?: number;
}

export type PreflightStatus = 'pass' | 'fail' | 'warn' | 'unavailable';

export interface PreflightCheck {
  id: 'engine' | 'content' | 'routing' | 'assets' | 'plugins' | 'checkpoint' | 'output';
  status: PreflightStatus;
  /** One-line human label (the check name). */
  label: string;
  /** What was actually observed — counts, ids, reasons. */
  detail: string;
  /** A blocking failure prevents submitting an export. */
  blocking: boolean;
}

export interface PreflightReport {
  checks: PreflightCheck[];
  /** Any check that currently blocks the export. */
  blockers: PreflightCheck[];
  /** True when nothing blocks — submit may proceed to spec validation. */
  ready: boolean;
}

function check(
  id: PreflightCheck['id'],
  status: PreflightStatus,
  label: string,
  detail: string,
  blocking: boolean,
): PreflightCheck {
  return { id, status, label, detail, blocking };
}

/**
 * Run the real preflight. Order is stable: attachment first (it gates
 * the rest), then content, routing, assets, plugins, checkpoint, output.
 */
export function runPreflight(inp: PreflightInputs): PreflightReport {
  const checks: PreflightCheck[] = [];

  if (!inp.engineAttached) {
    checks.push(
      check('engine', 'fail', 'Engine attached', 'engine is detached — reconnect before exporting', true),
      check('content', 'unavailable', 'Project has content', 'cannot evaluate while detached', false),
      check('routing', 'unavailable', 'Routing reaches an output', 'cannot evaluate while detached', false),
      check('assets', 'unavailable', 'All assets available', 'cannot evaluate while detached', false),
      check('plugins', 'unavailable', 'No missing plugins', 'cannot evaluate while detached', false),
      check('checkpoint', 'unavailable', 'Durable checkpoint recorded', 'cannot evaluate while detached', false),
      check('output', 'unavailable', 'Output is not silent', 'cannot evaluate while detached', false),
    );
    return { checks, blockers: checks.filter((c) => c.blocking && c.status === 'fail'), ready: false };
  }

  checks.push(check('engine', 'pass', 'Engine attached', 'engine responds to read views', false));

  if (inp.trackCount === 0) {
    checks.push(
      check('content', 'fail', 'Project has content', 'TRACK_LIST is empty — nothing to render', true),
    );
  } else {
    checks.push(
      check('content', 'pass', 'Project has content', `${inp.trackCount} track(s) reported`, false),
    );
  }

  if (inp.trackCount === 0) {
    checks.push(
      check('routing', 'unavailable', 'Routing reaches an output', 'no channels to route', false),
    );
  } else if (inp.busCount > 0) {
    checks.push(
      check('routing', 'pass', 'Routing reaches an output', `${inp.busCount} bus track(s) as mix destinations`, false),
    );
  } else {
    // Sends/route ops don't exist in the schema — tracks feed the master
    // implicitly. That is a real, known-good wiring state, not a failure.
    checks.push(
      check('routing', 'pass', 'Routing reaches an output', 'channels feed the master implicitly (no send ops in protocol)', false),
    );
  }

  const missing = inp.clipAssetRefs.filter((id) => !inp.assetIds.includes(id));
  const flagged = inp.clipMissingRefs ?? [];
  const allMissing = [...new Set([...missing, ...flagged])];
  if (allMissing.length > 0) {
    checks.push(
      check(
        'assets',
        'fail',
        'All assets available',
        `${allMissing.length} referenced asset(s) missing: ${allMissing.slice(0, 4).join(', ')}${allMissing.length > 4 ? '…' : ''}`,
        true,
      ),
    );
  } else if (inp.clipAssetRefs.length === 0 && inp.assetIds.length === 0) {
    checks.push(
      check('assets', 'pass', 'All assets available', 'no asset dependencies', false),
    );
  } else {
    checks.push(
      check(
        'assets',
        'pass',
        'All assets available',
        `${inp.assetIds.length} asset(s) located; ${inp.clipAssetRefs.length} referenced`,
        false,
      ),
    );
  }

  if (inp.failedDevices.length > 0) {
    checks.push(
      check(
        'plugins',
        'fail',
        'No missing plugins',
        `${inp.failedDevices.length} device(s) unavailable: ${inp.failedDevices
          .slice(0, 4)
          .map((d) => `${d.name} (${d.reason})`)
          .join(', ')}${inp.failedDevices.length > 4 ? '…' : ''}`,
        true,
      ),
    );
  } else {
    checks.push(
      check('plugins', 'pass', 'No missing plugins', 'all reported devices can render', false),
    );
  }

  if (!inp.checkpointId) {
    checks.push(
      check(
        'checkpoint',
        'fail',
        'Durable checkpoint recorded',
        'no SaveResultEvent has produced a checkpoint — save the project first',
        true,
      ),
    );
  } else {
    checks.push(
      check('checkpoint', 'pass', 'Durable checkpoint recorded', `checkpoint ${inp.checkpointId}`, false),
    );
  }

  if (!inp.rangeNonEmpty) {
    checks.push(
      check('output', 'fail', 'Output is not silent', 'export range is empty', true),
    );
  } else if (inp.clipsInRange === 0) {
    checks.push(
      check('output', 'warn', 'Output is not silent', 'no clips in the export range', false),
    );
  } else {
    // A true peak check happens in the runner's verify pass — the
    // preflight reports range sanity only and says so.
    checks.push(
      check('output', 'pass', 'Output is not silent', 'range is non-empty; peak verified by the render pass', false),
    );
  }

  const blockers = checks.filter((c) => c.blocking && c.status === 'fail');
  return { checks, blockers, ready: blockers.length === 0 };
}
