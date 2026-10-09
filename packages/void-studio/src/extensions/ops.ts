// Extension ops (W28) — the coordinator intents for module lifecycle.
// PROTOCOL GAP (recorded in docs/wasm/NEEDS.md): protocol major.1 has
// no module/extension op members — these builders emit the honest
// intended shapes, validated here so the request is real today and
// byte-identical when the ops land.

const ID_RE = /^[A-Za-z0-9._-]{1,128}$/;
const SHA_RE = /^[0-9a-f]{64}$/;
const VER_RE = /^\d+\.\d+\.\d+(-[A-Za-z0-9.-]+)?$/;

function checkId(moduleId: string): void {
  if (!ID_RE.test(moduleId)) throw new Error(`bad module_id "${moduleId}"`);
}
function checkVersion(version: string): void {
  if (!VER_RE.test(version)) throw new Error(`bad version "${version}"`);
}
function checkSha(sha: string, what: string): void {
  if (!SHA_RE.test(sha)) throw new Error(`bad ${what} sha256`);
}

/** Install a declarative package: raw manifest text + wasm bytes ref
 * + the sha the manifest binds. NOT a native binary path — there is
 * no such thing as a native module in this system. */
export interface InstallModuleOp {
  op: 'install_module';
  manifest_json: string;
  wasm_ref: string;
  wasm_sha256: string;
}

export function installModuleOp(
  manifestJson: string,
  wasmRef: string,
  wasmSha256: string,
): InstallModuleOp {
  if (!manifestJson || manifestJson.length > 64 * 1024)
    throw new Error('manifest_json missing or oversized');
  if (!wasmRef) throw new Error('wasm_ref required');
  checkSha(wasmSha256, 'wasm');
  return { op: 'install_module', manifest_json: manifestJson, wasm_ref: wasmRef, wasm_sha256: wasmSha256 };
}

export interface LoadModuleOp {
  op: 'load_module';
  module_id: string;
  /** null = latest active. */
  version: string | null;
}

export function loadModuleOp(moduleId: string, version?: string): LoadModuleOp {
  checkId(moduleId);
  if (version !== undefined && version !== null) checkVersion(version);
  return { op: 'load_module', module_id: moduleId, version: version ?? null };
}

export interface UnloadModuleOp {
  op: 'unload_module';
  module_id: string;
}

export function unloadModuleOp(moduleId: string): UnloadModuleOp {
  checkId(moduleId);
  return { op: 'unload_module', module_id: moduleId };
}

/** Hot reload: carry state across, keep last-known-good on failure —
 * the coordinator semantics implemented in void-wasm ModuleRuntime. */
export interface ReloadModuleOp {
  op: 'reload_module';
  module_id: string;
  /** null = latest active. */
  to_version: string | null;
}

export function reloadModuleOp(moduleId: string, toVersion?: string): ReloadModuleOp {
  checkId(moduleId);
  if (toVersion !== undefined && toVersion !== null) checkVersion(toVersion);
  return { op: 'reload_module', module_id: moduleId, to_version: toVersion ?? null };
}

/** Revoke → unload. version null = revoke every version of the id. */
export interface RevokeModuleOp {
  op: 'revoke_module';
  module_id: string;
  version: string | null;
  reason: string;
}

export function revokeModuleOp(
  moduleId: string,
  reason: string,
  version?: string,
): RevokeModuleOp {
  checkId(moduleId);
  if (version !== undefined && version !== null) checkVersion(version);
  if (!reason || reason.length > 512) throw new Error('revoke needs a reason');
  return { op: 'revoke_module', module_id: moduleId, version: version ?? null, reason };
}

/** Cancel a module's in-flight guest call (epoch-checked, ~1 ms). */
export interface CancelModuleOp {
  op: 'cancel_module_call';
  module_id: string;
}

export function cancelModuleOp(moduleId: string): CancelModuleOp {
  checkId(moduleId);
  return { op: 'cancel_module_call', module_id: moduleId };
}
